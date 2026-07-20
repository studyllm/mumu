// 屏幕使用时长统计
//
// 规格：openspec/changes/add-mumu-eye-care/specs/statistics/spec.md
//
// 实现要点：
// - 主循环：每 30 秒检测一次屏幕状态（screen_state::current_screen_state）
// - 状态机：保存 last_state；切换时产生 Action 列表
// - 累加：仅在 Active 时累计（30 秒入账 30 秒）
// - 暂停：Lock 切回 / Lock 离开 触发 start_pause / end_pause
// - 跨日：每次 tick 检测今天日期（record_screen_on 已经按 today 路由）
// - 通知：通过 mpsc::Sender 推给 T07 提醒调度器
//
// 设计：核心状态机逻辑为 pure 函数 `step()`，可单元测试
// 异步外壳 `StatisticsLoop::run()` 调 step() + tokio 计时

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time;

#[cfg(windows)]
use crate::screen_state::{current_screen_state, ScreenState};

use crate::db::{self, DbError, DbState};

// ============================================================================
// 常量
// ============================================================================

/// 检测间隔（秒）。MVP 用 30 秒与 spec 一致，T18 性能验证阶段可压到 60 秒
pub const TICK_SECONDS: u32 = 30;

/// 空闲判定阈值（毫秒）—— 30 分钟无鼠标/键盘即 Idle
pub const IDLE_THRESHOLD_MS: u32 = 30 * 60 * 1000;

// ============================================================================
// 通知事件（T07 提醒调度器订阅）
// ============================================================================

/// 状态切换通知（T07 据此决定是否触发提醒）
///
/// - 每隔 30 秒推一次 `Tick`
/// - 锁定时推 `Paused`
/// - 解锁/恢复时推 `Resumed`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatisticsEvent {
    /// 每 N 秒正常心跳（T07 用它驱提醒节拍）
    Tick {
        today_total_seconds: u32,
        state: String, // "Active" / "ScreenOff" / "Locked" / "Idle"
    },
    /// 用户进入不可用状态（锁屏 / 灭屏 / 长时间 idle）
    Paused,
    /// 用户从不可用状态恢复
    Resumed,
}

// ============================================================================
// 状态机（pure logic，可单测）
// ============================================================================

/// 内部运行时状态——仅在主循环内使用
#[derive(Debug, Clone)]
pub struct RuntimeState {
    pub last: StateSnapshot,
    pub last_date: String,
    /// 已累计但还未写入 db 的秒数（aggregate flush）
    pub pending_active_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    pub state: StateKind,
    pub date: String,
}

impl StateSnapshot {
    pub fn active(date: &str) -> Self {
        Self {
            state: StateKind::Active,
            date: date.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    Active,
    ScreenOff,
    Locked,
    Idle,
}

impl StateKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            StateKind::Active => "Active",
            StateKind::ScreenOff => "ScreenOff",
            StateKind::Locked => "Locked",
            StateKind::Idle => "Idle",
        }
    }
}

/// 单次状态切换产生的副作用（执行期）
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// 写入 N 秒到今日 total_seconds（通过 db::record_screen_on 累加）
    AddActiveSeconds(u32),
    /// 标记状态为暂停（触发 T07 也对应把 reminder_paused 设上）
    Pause(String),
    /// 标记状态为恢复
    Resume,
    /// 状态无变化 / 跨日产生的内部动作
    NoOp,
}

/// 状态机推进——根据 (prev, curr) 计算一组动作
///
/// 这是 T06 的核心逻辑，**全部为 pure 函数**，可单元测试。
pub fn step(prev: &StateSnapshot, curr: &StateSnapshot, tick_seconds: u32) -> Vec<Action> {
    let mut actions = Vec::new();

    // 跨日：日期变化 → 标记一次 NoOp 让上层知道，累加逻辑按 today 自动路由
    if prev.date != curr.date {
        actions.push(Action::NoOp);
    }

    // 累加逻辑：当前是 Active → 加 tick_seconds
    if curr.state == StateKind::Active {
        actions.push(Action::AddActiveSeconds(tick_seconds));
    }

    // 暂停状态切换
    let was_paused = matches!(prev.state, StateKind::Locked | StateKind::ScreenOff);
    let now_paused = matches!(curr.state, StateKind::Locked | StateKind::ScreenOff);

    if !was_paused && now_paused {
        actions.push(Action::Pause(curr.state.as_str().to_string()));
    } else if was_paused && !now_paused {
        actions.push(Action::Resume);
    }

    actions
}

// ============================================================================
// 异步主循环（Tauri runtime 中 spawn）
// ============================================================================

/// 主循环结构——持有 db 句柄 + 通知 sender
#[derive(Clone)]
pub struct StatisticsLoop {
    pub db: Arc<DbState>,
    pub notify: mpsc::Sender<StatisticsEvent>,
}

impl StatisticsLoop {
    pub fn new(db: Arc<DbState>, notify: mpsc::Sender<StatisticsEvent>) -> Self {
        Self { db, notify }
    }

    /// 启动主循环（在 Tauri builder 中 .spawn）
    ///
    /// 每 `TICK_SECONDS` 拉一次状态 → 调 step → 执行 Action（写 db）→ 推事件
    pub async fn run(self) {
        let mut last = StateSnapshot::active(&today_string());
        let mut interval = time::interval(Duration::from_secs(TICK_SECONDS as u64));
        // 跳过第一次立即触发（让启动时不立刻累加）
        interval.tick().await;

        loop {
            interval.tick().await;

            let curr_kind = poll_state_kind();
            let curr = StateSnapshot {
                state: curr_kind,
                date: today_string(),
            };

            // 状态机动作
            let actions = step(&last, &curr, TICK_SECONDS);
            for a in &actions {
                if let Action::AddActiveSeconds(secs) = a {
                    if let Err(e) = db::record_screen_on(&self.db, *secs) {
                        eprintln!("[mumu] record_screen_on failed: {e}");
                    }
                }
            }
            // 同步到 db pause_records
            for a in &actions {
                match a {
                    Action::Pause(reason) => {
                        if let Err(e) = db::start_pause(&self.db, reason) {
                            eprintln!("[mumu] start_pause failed: {e}");
                        }
                    }
                    Action::Resume => {
                        if let Err(e) = db::end_pause(&self.db) {
                            eprintln!("[mumu] end_pause failed: {e}");
                        }
                    }
                    _ => {}
                }
            }

            // 推送事件给订阅者（T07）
            let total = db::get_today_stats(&self.db)
                .map(|s| s.total_seconds)
                .unwrap_or(0);
            let _ = self
                .notify
                .send(StatisticsEvent::Tick {
                    today_total_seconds: total,
                    state: curr_kind.as_str().to_string(),
                })
                .await;

            // Pause/Resume 通知
            for a in &actions {
                let ev = match a {
                    Action::Pause(_) => Some(StatisticsEvent::Paused),
                    Action::Resume => Some(StatisticsEvent::Resumed),
                    _ => None,
                };
                if let Some(e) = ev {
                    let _ = self.notify.send(e).await;
                }
            }

            last = curr;
        }
    }
}

// ============================================================================
// 系统层包装
// ============================================================================

#[cfg(windows)]
fn poll_state_kind() -> StateKind {
    let s = current_screen_state(IDLE_THRESHOLD_MS);
    match s {
        ScreenState::Active => StateKind::Active,
        ScreenState::ScreenOff => StateKind::ScreenOff,
        ScreenState::Locked => StateKind::Locked,
        ScreenState::Idle => StateKind::Idle,
    }
}

#[cfg(not(windows))]
fn poll_state_kind() -> StateKind {
    // 非 Windows 占位——开发用，永远 Active（方便 Mac/Linux 编译/调试）
    StateKind::Active
}

/// 当前日期字符串 YYYY-MM-DD（本地时区），跨日检测用
pub fn today_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

// ============================================================================
// 单元测试（state machine pure logic）
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(kind: StateKind, date: &str) -> StateSnapshot {
        StateSnapshot {
            state: kind,
            date: date.to_string(),
        }
    }

    #[test]
    fn active_to_active_only_adds_seconds() {
        let prev = snap(StateKind::Active, "2026-07-19");
        let curr = snap(StateKind::Active, "2026-07-19");
        let actions = step(&prev, &curr, 30);
        assert!(actions.contains(&Action::AddActiveSeconds(30)));
        assert!(!actions.iter().any(|a| matches!(a, Action::Pause(_) | Action::Resume)));
    }

    #[test]
    fn active_to_locked_triggers_pause_not_add() {
        let prev = snap(StateKind::Active, "2026-07-19");
        let curr = snap(StateKind::Locked, "2026-07-19");
        let actions = step(&prev, &curr, 30);
        assert!(!actions.iter().any(|a| matches!(a, Action::AddActiveSeconds(30))));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Pause(s) if s == "Locked")));
    }

    #[test]
    fn locked_back_to_active_triggers_resume_and_adds() {
        let prev = snap(StateKind::Locked, "2026-07-19");
        let curr = snap(StateKind::Active, "2026-07-19");
        let actions = step(&prev, &curr, 30);
        assert!(actions.contains(&Action::Resume));
        assert!(actions.contains(&Action::AddActiveSeconds(30)));
    }

    #[test]
    fn screen_off_resume_yields_resume() {
        let prev = snap(StateKind::ScreenOff, "2026-07-19");
        let curr = snap(StateKind::Active, "2026-07-19");
        let actions = step(&prev, &curr, 30);
        assert!(actions.contains(&Action::Resume));
    }

    #[test]
    fn idle_does_not_pause_but_does_not_accumulate() {
        let prev = snap(StateKind::Active, "2026-07-19");
        let curr = snap(StateKind::Idle, "2026-07-19");
        let actions = step(&prev, &curr, 30);
        assert!(!actions.iter().any(|a| matches!(a, Action::AddActiveSeconds(_))));
        // Idle 不算锁屏/关屏（用户随时回返）
        assert!(!actions.iter().any(|a| matches!(a, Action::Pause(_))));
    }

    #[test]
    fn cross_day_emits_noop_and_starts_new_accumulation() {
        let prev = snap(StateKind::Active, "2026-07-19");
        let curr = snap(StateKind::Active, "2026-07-20");
        let actions = step(&prev, &curr, 30);
        assert!(actions.contains(&Action::AddActiveSeconds(30)));
        // 跨日不算 "paused → not paused"，不应出现 Resume
        assert!(!actions.contains(&Action::Resume));
    }

    #[test]
    fn locked_stays_locked_yields_no_duplicate_pause() {
        let prev = snap(StateKind::Locked, "2026-07-19");
        let curr = snap(StateKind::Locked, "2026-07-19");
        let actions = step(&prev, &curr, 30);
        assert!(!actions.iter().any(|a| matches!(a, Action::Pause(_))));
        assert!(!actions.iter().any(|a| matches!(a, Action::Resume)));
    }

    /// DbState/SQLite 行为已在 db.rs 单测过；这里只确保 DbError 类型可见
    #[allow(dead_code)]
    fn _db_error_smoke() -> Result<(), DbError> {
        let _err: DbError = DbError::SqlFailed(rusqlite::Error::InvalidQuery);
        Ok(())
    }
}
