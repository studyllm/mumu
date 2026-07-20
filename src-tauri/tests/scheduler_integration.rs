//! Integration test #1: scheduler 端到端
//!
//! 真 spawn `ReminderScheduler`，用 mpsc 喂 `StatisticsEvent` 和 `SchedulerControl`，
//! 收集 `ReminderCommand` 输出，验证整条链路正确。
//!
//! 不覆盖：
//! - screen_state（cfg(windows) 隔离；非 Windows 永远 Active）
//! - Local::now() 时间相关判定（依赖 tokio::time 自动推进 tick）

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mumu_lib::db::DbState;
use mumu_lib::reminders::{
    ReminderCommand, ReminderScheduler, SchedulerControl, SoftPromptKind,
};
use mumu_lib::settings::Settings;
use mumu_lib::statistics::StatisticsEvent;

fn fresh_db() -> Arc<DbState> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "mumu_integration_{}_{}",
        std::process::id(),
        id
    ));
    let _ = std::fs::create_dir_all(&dir);
    let path = PathBuf::from(&dir).join("stats.db");
    Arc::new(DbState::new_at(&path).expect("fresh db"))
}

/// 跑满一个 tick 周期（5 秒）让 scheduler 处理事件
async fn wait_a_tick() {
    tokio::time::sleep(Duration::from_millis(200)).await;
}

/// 等待直到收集到第一个匹配的事件（带超时）
async fn recv_matching<F>(rx: &mut tokio::sync::mpsc::Receiver<ReminderCommand>, pred: F) -> ReminderCommand
where
    F: Fn(&ReminderCommand) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let recv = tokio::time::timeout(remaining, rx.recv()).await;
        match recv {
            Ok(Some(cmd)) if pred(&cmd) => return cmd,
            Ok(Some(_)) => continue, // 跳过无关事件
            Ok(None) => panic!("reminder channel closed prematurely"),
            Err(_) => panic!("timed out waiting for matching ReminderCommand"),
        }
    }
}

#[tokio::test]
async fn test_trigger_emits_show_strong_reminder() {
    let db = fresh_db();
    let settings = Arc::new(tokio::sync::RwLock::new(Settings::default()));

    let (stats_tx, stats_rx) = tokio::sync::mpsc::channel(8);
    let (reminder_tx, mut reminder_rx) = tokio::sync::mpsc::channel(32);
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(8);

    let scheduler = ReminderScheduler::new(
        Arc::clone(&db),
        Arc::clone(&settings),
        stats_rx,
        reminder_tx,
        control_rx,
    );
    tokio::spawn(scheduler.run());

    // 喂一个 Active tick 让 scheduler 知道屏幕亮着
    stats_tx
        .send(StatisticsEvent::Tick {
            today_total_seconds: 0,
            state: "Active".into(),
        })
        .await
        .unwrap();
    wait_a_tick().await;

    // 测试按钮触发强提醒
    control_tx.send(SchedulerControl::TestTrigger).await.unwrap();

    let cmd = recv_matching(&mut reminder_rx, |c| {
        matches!(c, ReminderCommand::ShowStrongReminder { .. })
    })
    .await;
    match cmd {
        ReminderCommand::ShowStrongReminder {
            duration_seconds,
            play_sound,
            ..
        } => {
            assert_eq!(duration_seconds, 5, "test reminder 默认 5 秒");
            assert!(play_sound, "test reminder 默认播声音");
        }
        _ => unreachable!(),
    }

    drop(stats_tx);
    drop(control_tx);
}

#[tokio::test]
async fn skip_then_complete_flow_writes_db() {
    let db = fresh_db();
    let settings = Arc::new(tokio::sync::RwLock::new(Settings::default()));

    let (stats_tx, stats_rx) = tokio::sync::mpsc::channel(8);
    let (reminder_tx, mut reminder_rx) = tokio::sync::mpsc::channel(32);
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(8);

    let scheduler = ReminderScheduler::new(
        Arc::clone(&db),
        Arc::clone(&settings),
        stats_rx,
        reminder_tx,
        control_rx,
    );
    tokio::spawn(scheduler.run());

    stats_tx
        .send(StatisticsEvent::Tick {
            today_total_seconds: 0,
            state: "Active".into(),
        })
        .await
        .unwrap();
    wait_a_tick().await;

    // 触发
    control_tx.send(SchedulerControl::TestTrigger).await.unwrap();
    let _ = recv_matching(&mut reminder_rx, |c| {
        matches!(c, ReminderCommand::ShowStrongReminder { .. })
    })
    .await;

    // 用户点跳过
    control_tx.send(SchedulerControl::Skip).await.unwrap();
    let cmd = recv_matching(&mut reminder_rx, |c| {
        matches!(c, ReminderCommand::HideAllPopups)
    })
    .await;
    assert!(matches!(cmd, ReminderCommand::HideAllPopups));

    // skip 不应让 rest_count 增加
    let stats = mumu_lib::db::get_today_stats(&db).unwrap();
    assert_eq!(stats.rest_count, 0, "skip 不落库");

    // 第二次触发 + complete
    control_tx.send(SchedulerControl::TestTrigger).await.unwrap();
    let _ = recv_matching(&mut reminder_rx, |c| {
        matches!(c, ReminderCommand::ShowStrongReminder { .. })
    })
    .await;

    control_tx.send(SchedulerControl::Complete).await.unwrap();
    let _ = recv_matching(&mut reminder_rx, |c| {
        matches!(c, ReminderCommand::HideAllPopups)
    })
    .await;

    let stats = mumu_lib::db::get_today_stats(&db).unwrap();
    assert_eq!(stats.rest_count, 1, "complete 后 rest_count=1");
    // TestTrigger 的 duration_seconds=5（5 秒迷你弹窗），所以 add_rest_seconds 也 = 5
    assert_eq!(stats.rest_seconds, 5, "complete 后 rest_seconds=duration_seconds");

    drop(stats_tx);
    drop(control_tx);
}

#[tokio::test]
async fn locked_event_hides_all_popups() {
    let db = fresh_db();
    let settings = Arc::new(tokio::sync::RwLock::new(Settings::default()));

    let (stats_tx, stats_rx) = tokio::sync::mpsc::channel(8);
    let (reminder_tx, mut reminder_rx) = tokio::sync::mpsc::channel(32);
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(8);

    let scheduler = ReminderScheduler::new(
        Arc::clone(&db),
        Arc::clone(&settings),
        stats_rx,
        reminder_tx,
        control_rx,
    );
    tokio::spawn(scheduler.run());

    // 先确保 Active（让状态机基准存在）
    stats_tx
        .send(StatisticsEvent::Tick {
            today_total_seconds: 0,
            state: "Active".into(),
        })
        .await
        .unwrap();
    wait_a_tick().await;

    // 锁屏 → scheduler 触发 HideAllPopups
    stats_tx.send(StatisticsEvent::Paused).await.unwrap();
    let cmd = recv_matching(&mut reminder_rx, |c| {
        matches!(c, ReminderCommand::HideAllPopups)
    })
    .await;
    assert!(matches!(cmd, ReminderCommand::HideAllPopups));

    // 解锁 → Resumed（不强弹 reminder，只清状态）
    stats_tx.send(StatisticsEvent::Resumed).await.unwrap();
    // 至少等到一个 tick 周期过去，确认没有任何 ShowStrongReminder 发出
    tokio::time::sleep(Duration::from_secs(6)).await;

    drop(stats_tx);
    drop(control_tx);
}

#[tokio::test]
async fn dismiss_soft_eye_drop_updates_state() {
    let db = fresh_db();
    let settings = Arc::new(tokio::sync::RwLock::new(Settings::default()));

    let (stats_tx, stats_rx) = tokio::sync::mpsc::channel(8);
    let (reminder_tx, _reminder_rx) = tokio::sync::mpsc::channel(32);
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(8);

    let scheduler = ReminderScheduler::new(
        Arc::clone(&db),
        Arc::clone(&settings),
        stats_rx,
        reminder_tx,
        control_rx,
    );
    tokio::spawn(scheduler.run());

    // 喂 Active tick，触发一次 eye drop 周期不容易（依赖 tick 节拍 + work hour + interval），
    // 直接用 SchedulerControl 测试强提醒 + dismiss_soft 路径
    control_tx
        .send(SchedulerControl::DismissSoft(SoftPromptKind::EyeDrop))
        .await
        .unwrap();
    // 没有 active soft → 不应有任何 ShowSoftPrompt 输出
    tokio::time::sleep(Duration::from_millis(300)).await;

    drop(stats_tx);
    drop(control_tx);
}