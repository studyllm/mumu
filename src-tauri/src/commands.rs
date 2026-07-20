// Tauri 命令：前端 → Rust 桥
//
// T10：
// - reminder_skip      前端点"跳过"按钮
// - reminder_complete  倒计时归零自动完成
//
// T10.5：
// - softprompt_dismiss 软提示被点击 / 10s 自动消失 / 后端 HideAllPopups 触发
//
// 所有命令都通过 SchedulerControl channel 推回 reminders 调度器
// （调度器持有 ReminderState 单一可变源，避免锁竞争）。
// 同时立即下发 hide 事件到前端（不等调度器节拍）。

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tokio::sync::mpsc;

use crate::reminders::{SchedulerControl, SoftPromptKind};
use crate::settings::Settings;
use crate::windows::{hide_reminder_popup, hide_soft_prompt};

/// scheduler 控制 channel 的 Sender（注入 Tauri State）
pub struct SchedulerControlHandle(pub mpsc::Sender<SchedulerControl>);

/// T13：根据 settings.advanced.auto_start 同步注册表项
///
/// Windows：写入/删除 HKCU\Software\Microsoft\Windows\CurrentVersion\Run
/// 失败不抛——开机自启是"体验优化"而非核心功能，自启失败不阻塞其他逻辑
pub fn sync_autostart(app: &AppHandle, settings: &Settings) {
    let manager = app.autolaunch();
    let desired = settings.advanced.auto_start;
    let enabled = manager.is_enabled().unwrap_or(false);
    if desired == enabled {
        return;
    }
    let result = if desired {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(e) = result {
        eprintln!(
            "[mumu] autostart {} failed: {e}",
            if desired { "enable" } else { "disable" }
        );
    }
}

/// 用户点"跳过"按钮
#[tauri::command]
pub async fn reminder_skip(
    app: AppHandle,
    handle: tauri::State<'_, SchedulerControlHandle>,
) -> Result<(), String> {
    hide_reminder_popup(&app);
    handle
        .0
        .send(SchedulerControl::Skip)
        .await
        .map_err(|e| format!("scheduler 通道已关闭: {e}"))?;
    Ok(())
}

/// 倒计时归零自动完成
#[tauri::command]
pub async fn reminder_complete(
    app: AppHandle,
    handle: tauri::State<'_, SchedulerControlHandle>,
) -> Result<(), String> {
    hide_reminder_popup(&app);
    handle
        .0
        .send(SchedulerControl::Complete)
        .await
        .map_err(|e| format!("scheduler 通道已关闭: {e}"))?;
    Ok(())
}

/// 软提示 dismiss（T10.5）—— 收到后立即隐藏弹窗，并把状态变更推回调度器
/// 让 apply_dismiss_soft 记入 dismiss 队列 + 推迟下次触发
#[tauri::command]
pub async fn softprompt_dismiss(
    app: AppHandle,
    handle: tauri::State<'_, SchedulerControlHandle>,
    kind: SoftPromptKind,
) -> Result<(), String> {
    hide_soft_prompt(&app);
    handle
        .0
        .send(SchedulerControl::DismissSoft(kind))
        .await
        .map_err(|e| format!("scheduler 通道已关闭: {e}"))?;
    Ok(())
}

/// T11：设置页"测试提醒"按钮——立即触发 5 秒强提醒
#[tauri::command]
pub async fn trigger_test_reminder(
    handle: tauri::State<'_, SchedulerControlHandle>,
) -> Result<(), String> {
    handle
        .0
        .send(SchedulerControl::TestTrigger)
        .await
        .map_err(|e| format!("scheduler 通道已关闭: {e}"))?;
    Ok(())
}

