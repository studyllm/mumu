// 提醒调度器
//
// 规格：openspec/changes/add-mumu-eye-care/specs/reminders/spec.md
//
// 实现要点：
// - 主循环：订阅 T06 的 StatisticsEvent
// - 调度三类提醒：
//   - Strong: 20-20-20 强提醒（工作时段 + 间隔 + 屏幕亮 + 未锁屏 + 未全屏 + 未暂停）
//   - EyeDrop: 眼药水提醒（care.eye_drop_interval_minutes 默认 120）
//   - WarmCompress: 热敷提醒（care.warm_compress_time 默认 13:00）
// - 状态管理：last_strong_at / last_eye_drop_at / last_warm_compress_at / pause_until
// - 异常：锁屏立即关弹窗 + 暂停倒计时；关机/休眠不补弹
// - 软提示"3 次连续 dismiss" → 当天静默
//
// 设计：核心触发判定为 pure 函数，单测覆盖；异步外壳 ReminderScheduler::run() 集成 tokio 主循环

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time;

use crate::db::{self, DbState};
use crate::settings::{Settings, SettingsError};
use crate::statistics::StatisticsEvent;

// ============================================================================
// 常量
// ============================================================================

/// 调度器 tick 间隔（秒）。比 T06 的 30 秒更短，便于软提示精确触发
pub const TICK_SECONDS: u32 = 5;

/// 软提示连续 dismiss 次数上限（达到后静默一窗口）
pub const SOFT_DISMISS_LIMIT: usize = 3;

/// 软提示连续 dismiss 的判定窗口（分钟）
pub const SOFT_DISMISS_WINDOW_MINUTES: i64 = 30;

// ============================================================================
// ReminderCommand（调度器 → 前端）
// ============================================================================

/// 调度器发给前端的命令（前端驱动 UI 弹窗）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ReminderCommand {
    /// 显示强提醒弹窗（20-20-20）
    ShowStrongReminder {
        duration_seconds: u32,
        mute_sound: bool, // 跨时段（离下班 20 分钟内）静默
        play_sound: bool, // T12: 综合用户 play_sound 设置 + mute_sound，前端在归零时判断
    },
    /// 显示软提示（眼药水 / 热敷）
    ShowSoftPrompt {
        kind: SoftPromptKind,
        message: String,
    },
    /// 隐藏当前所有弹窗（锁屏 / 暂停 / 重启时）
    HideAllPopups,
    /// T19：托盘图标状态切换（不驱动任何弹窗，纯视觉信号）
    TrayEyeState {
        open: bool, // true=睁眼/工作  false=闭眼/休息
    },
    /// 状态通知（前端 UI 可选显示）
    Log {
        level: String, // "info" / "warn"
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftPromptKind {
    EyeDrop,
    WarmCompress,
}

impl SoftPromptKind {
    pub fn default_message(&self) -> &'static str {
        match self {
            SoftPromptKind::EyeDrop => "该滴眼药水了",
            SoftPromptKind::WarmCompress => "该热敷眼罩了",
        }
    }
}

// ============================================================================
// 调度器状态（pure logic）
// ============================================================================

#[derive(Debug, Clone)]
pub struct ReminderState {
    pub last_strong_at: Option<DateTime<Local>>,
    pub last_eye_drop_at: Option<DateTime<Local>>,
    pub last_warm_compress_at: Option<DateTime<Local>>,
    /// 手动暂停到期时间（None = 未暂停）
    pub pause_until: Option<DateTime<Local>>,
    /// 当前正在显示的强提醒弹窗（用于锁屏时暂停倒计时 + 续弹）
    pub active_strong: Option<ActiveStrong>,
    /// 软提示 dismiss 时间队列（仅保留最近窗口内）
    pub eye_drop_dismissals: VecDeque<DateTime<Local>>,
    pub warm_compress_dismissals: VecDeque<DateTime<Local>>,
}

impl Default for ReminderState {
    fn default() -> Self {
        Self {
            last_strong_at: None,
            last_eye_drop_at: None,
            last_warm_compress_at: None,
            pause_until: None,
            active_strong: None,
            eye_drop_dismissals: VecDeque::new(),
            warm_compress_dismissals: VecDeque::new(),
        }
    }
}

/// T36：主界面"上次提醒 / 下次倒计时"快照
///
/// 给前端一次性拿走三类提醒的关键时间点。
/// 字段命名对齐 should_trigger_* 的判定语义，序列化用 camelCase。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSnapshot {
    pub last_strong_at: Option<DateTime<Local>>,
    pub last_eye_drop_at: Option<DateTime<Local>>,
    pub last_warm_compress_at: Option<DateTime<Local>>,
    /// 各提醒距离下次触发的秒数（None = 不应触发；强提醒返回 0 表示"从未触发过"）
    pub next_strong_secs: Option<i64>,
    pub next_eye_drop_secs: Option<i64>,
    pub next_warm_compress_secs: Option<i64>,
    /// 当前是否在工作时段内（三个 next 的统一前提）
    pub in_work_hours: bool,
    /// 是否手动暂停中（含锁屏自动暂停）
    pub is_paused: bool,
    /// 给前端判断"该行要不要渲染"
    pub eye_drop_enabled: bool,
    pub warm_compress_enabled: bool,
    /// T36+：距离手动暂停到期的剩余秒数（None = 当前未暂停）
    pub pause_remaining_secs: Option<i64>,
    /// 各 hint 的具体原因（None = 没有特殊原因，沿用 next_*_secs / 默认文案）
    pub strong_hint: Option<NextHint>,
    pub eye_drop_hint: Option<NextHint>,
    pub warm_compress_hint: Option<NextHint>,
}

/// T36：主界面"下次提醒"语义提示
///
/// 比单纯的秒数更精确地告诉前端"为什么没有倒计时"。
/// 不影响 should_trigger_* 判定逻辑——只是 UI 文本层。
///
/// 序列化：tag+content 形式 `{ kind: "warmCompressAlreadyToday", value: { hhmm: "13:00" } }`
/// 字段名用短名 `hhmm` 避免与 Rust 端 `next_at_hhmm` 命名割裂
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum NextHint {
    /// 当前正在手动暂停（含锁屏自动暂停）
    Paused,
    /// 当前不在工作时段
    OutOfWorkHours,
    /// 软提示未启用（设置项勾掉）
    Disabled,
    /// 热敷今日已触发，下一次 = 明天固定时刻
    WarmCompressAlreadyToday { hhmm: String },
    /// 眼药水当日已 dismiss 满 3 次（当日静默），下一次 = 明天工作开始
    EyeDropDismissedLimit { hhmm: String },
}

/// T36：计算当前 reminder 快照（pure，便于单测）
///
/// 语义对齐 `should_trigger_strong` / `should_trigger_eye_drop` / `should_trigger_warm_compress`：
/// - 不在前提条件内 → `None`
/// - 距 last + interval → `Some(max(0, secs))`
/// - dismiss 后 `apply_dismiss_soft` 已经把 last 推到未来时刻（+30min / +1h），
///   所以"last + interval"算法对 dismiss 后依然正确
pub fn compute_snapshot(
    state: &ReminderState,
    settings: &Settings,
    now: DateTime<Local>,
) -> ReminderSnapshot {
    // 工作时段 + 暂停
    let now_time = now.time();
    let work_start = parse_hhmm(&settings.reminders.work_start);
    let work_end = parse_hhmm(&settings.reminders.work_end);
    let in_work_hours = within_work_hours(now_time, work_start, work_end);
    let is_paused = state.pause_until.map(|u| u > now).unwrap_or(false);

    // 强提醒 next + hint
    let (next_strong_secs, strong_hint) = if is_paused {
        (None, Some(NextHint::Paused))
    } else if !in_work_hours {
        (None, Some(NextHint::OutOfWorkHours))
    } else {
        // T36+：last=None 视为 last=now，首次启动要等满 interval
        let effective_last = state.last_strong_at.unwrap_or(now);
        let interval_secs = (settings.reminders.interval_minutes as i64) * 60;
        let remaining = (effective_last + chrono::Duration::seconds(interval_secs)) - now;
        (Some(remaining.num_seconds().max(0)), None)
    };

    // 眼药水 next + hint
    let (next_eye_drop_secs, eye_drop_hint) = if !settings.care.eye_drop_enabled {
        (None, Some(NextHint::Disabled))
    } else if is_paused {
        (None, Some(NextHint::Paused))
    } else if !in_work_hours {
        (None, Some(NextHint::OutOfWorkHours))
    } else {
        // dismiss 3 次 → 当日静默
        let mut q = state.eye_drop_dismissals.clone();
        purge_old_dismissals(&mut q, now);
        if q.len() >= SOFT_DISMISS_LIMIT {
            // 下次 = 明天工作开始
            let hhmm = format!("{:02}:{:02}", work_start.hour(), work_start.minute());
            (None, Some(NextHint::EyeDropDismissedLimit { hhmm }))
        } else {
            // T36+：last=None 视为 last=now，首次启动要等满 interval
            let effective_last = state.last_eye_drop_at.unwrap_or(now);
            let interval_secs = (settings.care.eye_drop_interval_minutes as i64) * 60;
            let remaining = (effective_last + chrono::Duration::seconds(interval_secs)) - now;
            (Some(remaining.num_seconds().max(0)), None)
        }
    };

    // 热敷 next + hint（固定时刻：今天/明天 warm_compress_time）
    let (next_warm_compress_secs, warm_compress_hint) = if !settings.care.warm_compress_enabled {
        (None, Some(NextHint::Disabled))
    } else if is_paused {
        (None, Some(NextHint::Paused))
    } else if !in_work_hours {
        (None, Some(NextHint::OutOfWorkHours))
    } else if let Some(last) = state.last_warm_compress_at {
        if last.date_naive() == now.date_naive() {
            // 今日已触发 → 下次 = 明天固定时刻
            (
                None,
                Some(NextHint::WarmCompressAlreadyToday {
                    hhmm: settings.care.warm_compress_time.clone(),
                }),
            )
        } else {
            (compute_next_fixed_secs(now, &settings.care.warm_compress_time), None)
        }
    } else {
        (compute_next_fixed_secs(now, &settings.care.warm_compress_time), None)
    };

    ReminderSnapshot {
        last_strong_at: state.last_strong_at,
        last_eye_drop_at: state.last_eye_drop_at,
        last_warm_compress_at: state.last_warm_compress_at,
        next_strong_secs,
        next_eye_drop_secs,
        next_warm_compress_secs,
        in_work_hours,
        is_paused,
        eye_drop_enabled: settings.care.eye_drop_enabled,
        warm_compress_enabled: settings.care.warm_compress_enabled,
        pause_remaining_secs: state
            .pause_until
            .filter(|u| *u > now)
            .map(|u| (u - now).num_seconds().max(0)),
        strong_hint,
        eye_drop_hint,
        warm_compress_hint,
    }
}

/// T36 辅助：算"下一个 HH:MM 时刻距离现在多少秒"（今天/明天自动跨日）
fn compute_next_fixed_secs(now: DateTime<Local>, hhmm: &str) -> Option<i64> {
    let target = parse_hhmm(hhmm);
    let today_target = now
        .date_naive()
        .and_hms_opt(target.hour() as u32, target.minute() as u32, 0)?;
    let dt = today_target.and_local_timezone(Local).single()?;
    if dt > now {
        Some((dt - now).num_seconds())
    } else {
        // 今天的时刻已过 → 推算明天同一时刻
        let tomorrow = now.date_naive().succ_opt()?;
        let tomorrow_target = tomorrow.and_hms_opt(target.hour() as u32, target.minute() as u32, 0)?;
        let dt2 = tomorrow_target.and_local_timezone(Local).single()?;
        Some((dt2 - now).num_seconds())
    }
}

/// 当前正在显示的强提醒弹窗状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveStrong {
    pub started_at: DateTime<Local>,
    pub duration_seconds: u32,
    pub remaining_seconds: u32,
    /// 触发时的状态（用于判断是否在下班前 20 分钟内 → 静默）
    pub mute_sound: bool,
    /// True = 静默 / 隐藏状态（锁屏 / 暂停时进入）
    pub hidden: bool,
}

// ============================================================================
// 强提醒触发判定（pure）
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrongDecision {
    /// 不触发
    NotTrigger,
    /// 触发（含配置信息）
    Trigger { mute_sound: bool },
    /// 续弹（之前因为锁屏暂停，现在解锁）
    ResumeActive(ActiveStrong),
}

/// 判定强提醒是否应触发
pub fn should_trigger_strong(
    state: &ReminderState,
    settings: &Settings,
    now: DateTime<Local>,
) -> StrongDecision {
    // 续弹判定：上次锁屏时进入隐藏的弹窗，解锁后还在工作时段 → 续弹
    if let Some(active) = &state.active_strong {
        if active.hidden {
            return StrongDecision::ResumeActive(active.clone());
        }
        // T26 修复：active_strong 存在但没隐藏（即倒计时进行中）→ 不重复触发。
        // 旧逻辑会走到间隔判定，而 last_strong_at 只在 skip/complete 时才被 set，
        // 导致倒计时期间每 5 秒一次 tick 都会再 emit ShowStrongReminder，
        // 前端倒计时卡在 16→20→16→20。
        return StrongDecision::NotTrigger;
    }

    // 工作时段
    let now_time = now.time();
    let start = parse_hhmm(&settings.reminders.work_start);
    let end = parse_hhmm(&settings.reminders.work_end);
    if !within_work_hours(now_time, start, end) {
        return StrongDecision::NotTrigger;
    }

    // 暂停
    if let Some(until) = state.pause_until {
        if now < until {
            return StrongDecision::NotTrigger;
        }
    }

    // 距上次间隔
    let interval_secs = (settings.reminders.interval_minutes as i64) * 60;
    // T36+：last_strong_at=None 也视为 last=app_started_at（=now 启动时刻），
    // 即首次启动也要等满 interval 才触发，不再"装了立刻弹"。
    // 这样新用户有 onboarding 时间，不会一开机就被三个提醒淹没。
    let effective_last = state.last_strong_at.unwrap_or(now);
    let elapsed = (now - effective_last).num_seconds();
    if elapsed < interval_secs {
        return StrongDecision::NotTrigger;
    }

    // 跨时段（离下班 20 分钟内）静默
    let end_secs = (end.hour() as i64) * 3600 + (end.minute() as i64) * 60;
    let now_secs = (now_time.hour() as i64) * 3600 + (now_time.minute() as i64) * 60;
    let mute_sound = (end_secs - now_secs).abs() <= 20 * 60;

    StrongDecision::Trigger { mute_sound }
}

// ============================================================================
// 软提示触发判定（pure）
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoftDecision {
    NotTrigger,
    EyeDropDismissedTooMany,
    WarmCompressAlreadyToday,
    Trigger(SoftPromptKind),
}

pub fn should_trigger_eye_drop(
    state: &ReminderState,
    settings: &Settings,
    now: DateTime<Local>,
) -> SoftDecision {
    if !settings.care.eye_drop_enabled {
        return SoftDecision::NotTrigger;
    }
    if !within_work_hours(
        now.time(),
        parse_hhmm(&settings.reminders.work_start),
        parse_hhmm(&settings.reminders.work_end),
    ) {
        return SoftDecision::NotTrigger;
    }
    if let Some(until) = state.pause_until {
        if now < until {
            return SoftDecision::NotTrigger;
        }
    }

    // 已被连续 3 次 dismiss → 不再触发
    let mut q = state.eye_drop_dismissals.clone();
    purge_old_dismissals(&mut q, now);
    if q.len() >= SOFT_DISMISS_LIMIT {
        return SoftDecision::EyeDropDismissedTooMany;
    }

    // 距上次间隔（默认 120 分钟）
    // T36+：last=None 视为 last=now（首次启动也等满 interval 再触发）
    let interval_secs = (settings.care.eye_drop_interval_minutes as i64) * 60;
    let effective_last = state.last_eye_drop_at.unwrap_or(now);
    if (now - effective_last).num_seconds() < interval_secs {
        return SoftDecision::NotTrigger;
    }

    SoftDecision::Trigger(SoftPromptKind::EyeDrop)
}

pub fn should_trigger_warm_compress(
    state: &ReminderState,
    settings: &Settings,
    now: DateTime<Local>,
) -> SoftDecision {
    if !settings.care.warm_compress_enabled {
        return SoftDecision::NotTrigger;
    }
    if !within_work_hours(
        now.time(),
        parse_hhmm(&settings.reminders.work_start),
        parse_hhmm(&settings.reminders.work_end),
    ) {
        return SoftDecision::NotTrigger;
    }
    if let Some(until) = state.pause_until {
        if now < until {
            return SoftDecision::NotTrigger;
        }
    }

    // T36+：首次启动也等下一次 warm_compress_time 才触发，不再"装了立刻弹"
    // last=None 时假设 last=昨天 → date 不匹配 + 跳过今日 → 等下一次固定时刻
    let last_for_check = state.last_warm_compress_at.unwrap_or_else(|| {
        now.date_naive()
            .pred_opt()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .and_then(|naive| naive.and_local_timezone(Local).single())
            .unwrap_or(now)
    });
    if last_for_check.date_naive() == now.date_naive() {
        return SoftDecision::WarmCompressAlreadyToday;
    }

    // 到达配置的 warm_compress_time（默认 13:00）
    let target = parse_hhmm(&settings.care.warm_compress_time);
    let now_secs = now.hour() as i64 * 3600 + now.minute() as i64 * 60;
    let target_secs = target.hour() as i64 * 3600 + target.minute() as i64 * 60;
    if now_secs < target_secs {
        return SoftDecision::NotTrigger;
    }

    SoftDecision::Trigger(SoftPromptKind::WarmCompress)
}

// ============================================================================
// 状态变更 API（pure mutation helpers）
// ============================================================================

pub fn apply_dismiss_strong(state: &mut ReminderState, now: DateTime<Local>) -> Vec<ReminderCommand> {
    if state.active_strong.take().is_some() {
        state.last_strong_at = Some(now);
        vec![
            ReminderCommand::HideAllPopups,
            ReminderCommand::Log {
                level: "info".into(),
                message: "强提醒 dismiss".into(),
            },
        ]
    } else {
        vec![]
    }
}

/// 完成强提醒（倒计时归零）—— 调用 increment_rest_count + add_rest_seconds + last_strong_at
pub fn apply_complete_strong(
    state: &mut ReminderState,
    db: &DbState,
    now: DateTime<Local>,
) -> Vec<ReminderCommand> {
    if let Some(active) = state.active_strong.take() {
        state.last_strong_at = Some(now);
        if let Err(e) = db::increment_rest_count(db) {
            eprintln!("[mumu] increment_rest_count failed: {e}");
        }
        if let Err(e) = db::add_rest_seconds(db, active.duration_seconds) {
            eprintln!("[mumu] add_rest_seconds failed: {e}");
        }
        vec![
            ReminderCommand::HideAllPopups,
            ReminderCommand::Log {
                level: "info".into(),
                message: format!("强提醒完成, duration={}s", active.duration_seconds),
            },
        ]
    } else {
        vec![]
    }
}

/// 软提示 dismiss —— 记录到队列；next 提醒按 spec 推迟
pub fn apply_dismiss_soft(
    state: &mut ReminderState,
    kind: SoftPromptKind,
    now: DateTime<Local>,
) -> Vec<ReminderCommand> {
    match kind {
        SoftPromptKind::EyeDrop => {
            let mut q = state.eye_drop_dismissals.clone();
            purge_old_dismissals(&mut q, now);
            q.push_back(now);
            state.eye_drop_dismissals = q;
            // 下次提醒间隔 30 分钟（spec）
            state.last_eye_drop_at = Some(now + chrono::Duration::minutes(30));
        }
        SoftPromptKind::WarmCompress => {
            let mut q = state.warm_compress_dismissals.clone();
            purge_old_dismissals(&mut q, now);
            q.push_back(now);
            state.warm_compress_dismissals = q;
            // 下次提醒间隔 1 小时
            state.last_warm_compress_at = Some(now + chrono::Duration::hours(1));
        }
    }
    vec![ReminderCommand::HideAllPopups]
}

/// 手动暂停 X 分钟
pub fn apply_manual_pause(state: &mut ReminderState, minutes: u32, now: DateTime<Local>) {
    state.pause_until = Some(now + chrono::Duration::minutes(minutes as i64));
}

/// 暂停到明天工作开始时间
pub fn apply_pause_until_tomorrow(state: &mut ReminderState, settings: &Settings, now: DateTime<Local>) {
    let tomorrow = now.date_naive().succ_opt().unwrap_or(now.date_naive());
    let start = parse_hhmm(&settings.reminders.work_start);
    let when = tomorrow.and_hms_opt(start.hour() as u32, start.minute() as u32, 0);
    if let Some(naive) = when {
        if let Some(dt) = naive.and_local_timezone(Local).single() {
            state.pause_until = Some(dt);
        }
    }
}

/// 锁定 / 关屏时调用 —— 隐藏当前弹窗；pause_until 由上层给
pub fn apply_pause(state: &mut ReminderState, reason: &str, now: DateTime<Local>) {
    if let Some(active) = state.active_strong.as_mut() {
        active.hidden = true;
    }
    if state.pause_until.is_none() {
        state.pause_until = Some(now + chrono::Duration::minutes(30));
    }
    let _ = reason;
}

/// 恢复（解锁 / 重启 / 退出全屏）
pub fn apply_resume(state: &mut ReminderState) {
    state.pause_until = None;
}

// ============================================================================
// 辅助函数
// ============================================================================

fn parse_hhmm(s: &str) -> NaiveTime {
    NaiveTime::parse_from_str(s, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap())
}

fn within_work_hours(now: NaiveTime, start: NaiveTime, end: NaiveTime) -> bool {
    if start <= end {
        now >= start && now <= end
    } else {
        // 跨午夜工作时段（如 22:00 - 06:00）—— MVP 不支持
        now >= start || now <= end
    }
}

/// 删除窗口外的旧 dismiss
fn purge_old_dismissals(q: &mut VecDeque<DateTime<Local>>, now: DateTime<Local>) {
    while let Some(&front) = q.front() {
        if (now - front).num_minutes() > SOFT_DISMISS_WINDOW_MINUTES {
            q.pop_front();
        } else {
            break;
        }
    }
}

// ============================================================================
// 防止 SettingsError 警告（user 命令边界）
// ============================================================================

#[allow(dead_code)]
fn _settings_err_smoke() -> Result<(), SettingsError> {
    Err(SettingsError::NoAppDataDir)
}

// ============================================================================
// 异步主循环（计划在 Tauri runtime 中 spawn）
// ============================================================================

/// scheduler 内部控制消息（前端 invoke → 经 commands.rs → mpsc::Sender 推回）
#[derive(Debug, Clone)]
pub enum SchedulerControl {
    /// 用户点"跳过"
    Skip,
    /// 倒计时归零自动完成
    Complete,
    /// 软提示被 dismiss（kind: EyeDrop / WarmCompress）
    DismissSoft(SoftPromptKind),
    /// T11 设置页点"测试提醒"——立即触发 5 秒强提醒
    TestTrigger,
    /// T32 设置页点"护眼提醒"——立即弹一次弱提示（眼药水 / 热敷）
    /// 不计入正常调度节拍，不影响下次自然触发
    TestSoft(SoftPromptKind),
    /// T08 托盘菜单"暂停 X 分钟"
    PauseMinutes(u32),
    /// T08 托盘菜单"暂停到明早 9:00"
    PauseUntilTomorrow,
    /// T08 托盘菜单"继续"（清除手动暂停）
    Resume,
}

#[allow(dead_code)]
pub struct ReminderScheduler {
    pub db: Arc<DbState>,
    pub settings: Arc<tokio::sync::RwLock<Settings>>,
    pub stats_rx: mpsc::Receiver<StatisticsEvent>,
    pub reminder_tx: mpsc::Sender<ReminderCommand>,
    /// T10 新增：scheduler 内部控制消息（前端 skip/complete）
    pub control_rx: mpsc::Receiver<SchedulerControl>,
}

#[allow(dead_code)]
impl ReminderScheduler {
    pub fn new(
        db: Arc<DbState>,
        settings: Arc<tokio::sync::RwLock<Settings>>,
        stats_rx: mpsc::Receiver<StatisticsEvent>,
        reminder_tx: mpsc::Sender<ReminderCommand>,
        control_rx: mpsc::Receiver<SchedulerControl>,
    ) -> Self {
        Self {
            db,
            settings,
            stats_rx,
            reminder_tx,
            control_rx,
        }
    }

    pub async fn run(mut self, state: Arc<tokio::sync::Mutex<ReminderState>>) {
        // T36：state 由外部传入并通过 Arc 共享给 Tauri command
        // （get_reminder_status_cmd 要读 last_*_at 给主界面展示）
        let mut ticker = time::interval(Duration::from_secs(TICK_SECONDS as u64));
        // 跳过首次立即触发
        ticker.tick().await;

        loop {
            tokio::select! {
                maybe_ev = self.stats_rx.recv() => {
                    if let Some(ev) = maybe_ev {
                        let mut state = state.lock().await;
                        self.handle_statistics_event(&mut state, &ev).await;
                    }
                }
                maybe_ctrl = self.control_rx.recv() => {
                    if let Some(ctrl) = maybe_ctrl {
                        let mut state = state.lock().await;
                        self.handle_control(&mut state, ctrl).await;
                    }
                }
                _ = ticker.tick() => {
                    let mut state = state.lock().await;
                    self.tick(&mut state).await;
                }
            }
        }
    }

    async fn handle_control(&self, state: &mut ReminderState, ctrl: SchedulerControl) {
        let now = Local::now();
        match ctrl {
            SchedulerControl::Skip => {
                let cmds = apply_dismiss_strong(state, now);
                for cmd in cmds {
                    let _ = self.reminder_tx.send(cmd).await;
                }
            }
            SchedulerControl::Complete => {
                let cmds = apply_complete_strong(state, &self.db, now);
                for cmd in cmds {
                    let _ = self.reminder_tx.send(cmd).await;
                }
            }
            SchedulerControl::DismissSoft(kind) => {
                let cmds = apply_dismiss_soft(state, kind, now);
                for cmd in cmds {
                    let _ = self.reminder_tx.send(cmd).await;
                }
            }
            SchedulerControl::PauseMinutes(minutes) => {
                apply_manual_pause(state, minutes, now);
            }
            SchedulerControl::PauseUntilTomorrow => {
                let settings = self.settings.read().await.clone();
                apply_pause_until_tomorrow(state, &settings, now);
            }
            SchedulerControl::Resume => {
                apply_resume(state);
            }
            SchedulerControl::TestTrigger => {
                // T11：测试按钮——立即弹 5 秒强提醒，不影响正常调度
                // 但要遵守 show_popup 设置
                let settings = self.settings.read().await.clone();
                if !settings.reminders.show_popup {
                    let _ = self
                        .reminder_tx
                        .send(ReminderCommand::Log {
                            level: "warn".into(),
                            message: "测试提醒被 show_popup=false 拦截".into(),
                        })
                        .await;
                    return;
                }
                let active = ActiveStrong {
                    started_at: now,
                    duration_seconds: 5,
                    remaining_seconds: 5,
                    mute_sound: false, // 测试提醒始终播放声音
                    hidden: false,
                };
                state.active_strong = Some(active.clone());
                let _ = self
                    .reminder_tx
                    .send(ReminderCommand::ShowStrongReminder {
                        duration_seconds: active.duration_seconds,
                        mute_sound: active.mute_sound,
                        play_sound: true, // T11 测试按钮：始终播放声音
                    })
                    .await;
            }
            SchedulerControl::TestSoft(kind) => {
                // T32：设置页点"护眼提醒"——立刻弹一次弱提示。
                // 不写 dismiss 队列（自然节拍不受影响），也不依赖对应 enabled 开关
                // （用户没启用"眼药水提醒"也可能想看一眼提示长什么样）。
                let _ = self
                    .reminder_tx
                    .send(ReminderCommand::ShowSoftPrompt {
                        kind,
                        message: kind.default_message().to_string(),
                    })
                    .await;
            }
        }
    }

    async fn handle_statistics_event(&self, state: &mut ReminderState, ev: &StatisticsEvent) {
        let now = Local::now();
        match ev {
            StatisticsEvent::Tick { state: st, .. } => {
                if st != "Active" {
                    // T30 修复：锁屏/关屏时如果当前有 active 强提醒弹窗，
                    // 必须主动 dismiss 它（不算休息也不响声音），不能只设 hidden。
                    // 否则前端的 setInterval 仍在跑，归零时会 invoke reminder_complete
                    // 并播木鱼声，但用户其实已经离开电脑了。
                    if state.active_strong.is_some() {
                        let cmds = apply_dismiss_strong(state, now);
                        for cmd in cmds {
                            let _ = self.reminder_tx.send(cmd).await;
                        }
                    } else {
                        apply_pause(state, st, now);
                    }
                    let _ = self.reminder_tx.send(ReminderCommand::HideAllPopups).await;
                } else if let Some(until) = state.pause_until {
                    if until <= now {
                        apply_resume(state);
                    }
                }
            }
            StatisticsEvent::Paused => {
                // 手动暂停：同样的 dismiss 逻辑
                if state.active_strong.is_some() {
                    let cmds = apply_dismiss_strong(state, now);
                    for cmd in cmds {
                        let _ = self.reminder_tx.send(cmd).await;
                    }
                } else {
                    apply_pause(state, "user_paused", now);
                }
                let _ = self.reminder_tx.send(ReminderCommand::HideAllPopups).await;
            }
            StatisticsEvent::Resumed => {
                apply_resume(state);
                // T19：解锁立即把托盘图标翻回睁眼，不等下一个 tick
                let _ = self
                    .reminder_tx
                    .send(ReminderCommand::TrayEyeState { open: true })
                    .await;
            }
        }
    }

    async fn tick(&self, state: &mut ReminderState) {
        let settings = self.settings.read().await.clone();
        let now = Local::now();

        // T38 修复：scheduler 启动时 last_strong_at/last_eye_drop_at=None，
        // 每次 tick 都用 now 当 effective_last → elapsed 永远 0 → 永远 NotTrigger。
        // 正确做法：在第一次进入工作时段时把 last_*_at 初始化为当前时刻，
        // 这样后续 tick 的 elapsed 才会单调增加，最终达到 interval_secs 触发。
        // （保留 T36 "首次启动等满 interval" 的语义：新用户有 onboarding 时间，
        // 不会一开机就被三个提醒淹没。）
        if state.last_strong_at.is_none() {
            let now_time = now.time();
            let start = parse_hhmm(&settings.reminders.work_start);
            let end = parse_hhmm(&settings.reminders.work_end);
            if within_work_hours(now_time, start, end) {
                state.last_strong_at = Some(now);
            }
        }
        if state.last_eye_drop_at.is_none() && settings.care.eye_drop_enabled {
            let now_time = now.time();
            let start = parse_hhmm(&settings.reminders.work_start);
            let end = parse_hhmm(&settings.reminders.work_end);
            if within_work_hours(now_time, start, end) {
                state.last_eye_drop_at = Some(now);
            }
        }

        let strong = should_trigger_strong(state, &settings, now);
        match strong {
            StrongDecision::NotTrigger => {}
            StrongDecision::Trigger { mute_sound } => {
                // T10: 设置 show_popup=false 时不弹窗（仅记日志；状态机仍记录以保持间隔）
                if !settings.reminders.show_popup {
                    state.active_strong = Some(ActiveStrong {
                        started_at: now,
                        duration_seconds: settings.reminders.rest_seconds,
                        remaining_seconds: settings.reminders.rest_seconds,
                        mute_sound,
                        hidden: false,
                    });
                    let _ = self
                        .reminder_tx
                        .send(ReminderCommand::Log {
                            level: "info".into(),
                            message: "show_popup=false，跳过弹窗（状态已记）".into(),
                        })
                        .await;
                    return;
                }
                let active = ActiveStrong {
                    started_at: now,
                    duration_seconds: settings.reminders.rest_seconds,
                    remaining_seconds: settings.reminders.rest_seconds,
                    mute_sound,
                    hidden: false,
                };
                state.active_strong = Some(active.clone());
                let _ = self
                    .reminder_tx
                    .send(ReminderCommand::ShowStrongReminder {
                        duration_seconds: active.duration_seconds,
                        mute_sound: active.mute_sound,
                        play_sound: settings.reminders.play_sound && !active.mute_sound,
                    })
                    .await;
            }
            StrongDecision::ResumeActive(active) => {
                if !settings.reminders.show_popup {
                    if let Some(a) = state.active_strong.as_mut() {
                        a.hidden = false;
                    } else {
                        state.active_strong = Some(active.clone());
                    }
                    let _ = self
                        .reminder_tx
                        .send(ReminderCommand::Log {
                            level: "info".into(),
                            message: "show_popup=false，跳过续弹".into(),
                        })
                        .await;
                    return;
                }
                if let Some(a) = state.active_strong.as_mut() {
                    a.hidden = false;
                } else {
                    state.active_strong = Some(active.clone());
                }
                let _ = self
                    .reminder_tx
                    .send(ReminderCommand::ShowStrongReminder {
                        duration_seconds: active.duration_seconds,
                        mute_sound: active.mute_sound,
                        play_sound: settings.reminders.play_sound && !active.mute_sound,
                    })
                    .await;
            }
        }

        if matches!(
            should_trigger_eye_drop(state, &settings, now),
            SoftDecision::Trigger(SoftPromptKind::EyeDrop)
        ) {
            let _ = self
                .reminder_tx
                .send(ReminderCommand::ShowSoftPrompt {
                    kind: SoftPromptKind::EyeDrop,
                    message: SoftPromptKind::EyeDrop.default_message().to_string(),
                })
                .await;
            state.last_eye_drop_at = Some(now);
        }

        if matches!(
            should_trigger_warm_compress(state, &settings, now),
            SoftDecision::Trigger(SoftPromptKind::WarmCompress)
        ) {
            let _ = self
                .reminder_tx
                .send(ReminderCommand::ShowSoftPrompt {
                    kind: SoftPromptKind::WarmCompress,
                    message: SoftPromptKind::WarmCompress.default_message().to_string(),
                })
                .await;
            state.last_warm_compress_at = Some(now);
        }
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    fn default_settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn work_hours_normal_inclusive() {
        let start = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(18, 0, 0).unwrap();
        assert!(within_work_hours(
            NaiveTime::from_hms_opt(10, 30, 0).unwrap(),
            start,
            end
        ));
        assert!(!within_work_hours(
            NaiveTime::from_hms_opt(8, 59, 0).unwrap(),
            start,
            end
        ));
        assert!(!within_work_hours(
            NaiveTime::from_hms_opt(18, 1, 0).unwrap(),
            start,
            end
        ));
        assert!(within_work_hours(
            NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            start,
            end
        ));
        assert!(within_work_hours(
            NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
            start,
            end
        ));
    }

    #[test]
    fn strong_triggered_inside_work_hours_never_triggered_before() {
        let st = default_settings();
        // T36+：last=None 不再立即触发。last 设为 far past（> interval）才触发
        let mut state = ReminderState::default();
        state.last_strong_at = Some(at(2026, 7, 19, 9, 0));
        let now = at(2026, 7, 19, 10, 30);
        let d = should_trigger_strong(&state, &st, now);
        assert!(matches!(d, StrongDecision::Trigger { mute_sound: false }));
    }

    #[test]
    fn strong_not_triggered_outside_work_hours() {
        let st = default_settings();
        let state = ReminderState::default();
        let now = at(2026, 7, 19, 19, 0);
        assert_eq!(
            should_trigger_strong(&state, &st, now),
            StrongDecision::NotTrigger
        );
    }

    #[test]
    fn strong_respects_interval() {
        let st = default_settings();
        let mut state = ReminderState::default();
        let now = at(2026, 7, 19, 10, 0);
        state.last_strong_at = Some(at(2026, 7, 19, 9, 50)); // 10 分钟前，未到 20 分钟
        assert_eq!(
            should_trigger_strong(&state, &st, now),
            StrongDecision::NotTrigger
        );
        state.last_strong_at = Some(at(2026, 7, 19, 9, 35));
        assert!(matches!(
            should_trigger_strong(&state, &st, now),
            StrongDecision::Trigger { .. }
        ));
    }

    #[test]
    fn strong_mutes_sound_within_20_min_of_quitting() {
        let st = default_settings();
        // T36+：last=None 不立即触发，设 far past
        let mut state = ReminderState::default();
        state.last_strong_at = Some(at(2026, 7, 19, 17, 0));
        let now = at(2026, 7, 19, 17, 45); // 离 18:00 差 15 分钟
        let d = should_trigger_strong(&state, &st, now);
        assert!(matches!(d, StrongDecision::Trigger { mute_sound: true }));
    }

    #[test]
    fn strong_respects_pause() {
        let st = default_settings();
        let mut state = ReminderState::default();
        // T36+：设 far past 让 interval 通过
        state.last_strong_at = Some(at(2026, 7, 19, 9, 0));
        state.pause_until = Some(at(2026, 7, 19, 11, 0));
        let now = at(2026, 7, 19, 10, 30);
        assert_eq!(
            should_trigger_strong(&state, &st, now),
            StrongDecision::NotTrigger
        );
        let later = at(2026, 7, 19, 11, 1);
        assert!(matches!(
            should_trigger_strong(&state, &st, later),
            StrongDecision::Trigger { .. }
        ));
    }

    #[test]
    fn strong_resume_active_when_was_hidden() {
        let st = default_settings();
        let mut state = ReminderState::default();
        state.active_strong = Some(ActiveStrong {
            started_at: at(2026, 7, 19, 10, 0),
            duration_seconds: 20,
            remaining_seconds: 10,
            mute_sound: false,
            hidden: true,
        });
        let now = at(2026, 7, 19, 10, 10);
        let d = should_trigger_strong(&state, &st, now);
        assert!(matches!(d, StrongDecision::ResumeActive(_)));
    }

    /// T26 修复：active_strong 存在但 hidden=false（倒计时进行中）→ NotTrigger。
    /// 否则每 5 秒一次 tick 都会重新 emit → 前端倒计时卡在 16→20 循环
    /// （last_strong_at 只在 skip/complete 时才 set，倒计时期间一直是 None/老值）
    #[test]
    fn strong_not_retriggered_while_countdown_running() {
        let st = default_settings();
        let mut state = ReminderState::default();
        state.active_strong = Some(ActiveStrong {
            started_at: at(2026, 7, 19, 10, 0),
            duration_seconds: 20,
            remaining_seconds: 15,
            mute_sound: false,
            hidden: false, // 倒计时中
        });
        let now = at(2026, 7, 19, 10, 5); // 5 秒后
        assert_eq!(
            should_trigger_strong(&state, &st, now),
            StrongDecision::NotTrigger
        );
    }

    /// T38 回归：模拟 tick 初始化 last_strong_at=启动时刻，
    /// 再过 interval 时间 → 必须触发（之前永远 NotTrigger 是 bug）
    #[test]
    fn strong_triggers_after_initialization_then_interval_elapses() {
        let mut st = default_settings();
        st.reminders.interval_minutes = 1; // 测试用：1 分钟间隔
        let mut state = ReminderState::default();

        // 第一次 tick：last_strong_at=None，scheduler 在 work_hours 内初始化为 t0
        let t0 = at(2026, 7, 19, 10, 0);
        state.last_strong_at = Some(t0);

        // 30 秒后：未到 1 分钟 → NotTrigger
        let t30 = at(2026, 7, 19, 10, 0).with_second(30).unwrap();
        assert_eq!(
            should_trigger_strong(&state, &st, t30),
            StrongDecision::NotTrigger
        );

        // 60 秒后：刚好满 1 分钟 → Trigger
        let t60 = at(2026, 7, 19, 10, 1);
        assert!(matches!(
            should_trigger_strong(&state, &st, t60),
            StrongDecision::Trigger { .. }
        ));
    }

    #[test]
    fn eye_drop_triggered_first_time_in_work_hours() {
        let st = default_settings();
        // T36+：last=None 不立即触发。设 far past（> 120 min）才触发
        let mut state = ReminderState::default();
        state.last_eye_drop_at = Some(at(2026, 7, 19, 7, 0));
        let now = at(2026, 7, 19, 10, 0);
        assert_eq!(
            should_trigger_eye_drop(&state, &st, now),
            SoftDecision::Trigger(SoftPromptKind::EyeDrop)
        );
    }

    #[test]
    fn eye_drop_not_triggered_within_interval() {
        let st = default_settings();
        let mut state = ReminderState::default();
        // 上次 30 分钟前（远小于默认 120 分钟）→ 不触发
        state.last_eye_drop_at = Some(at(2026, 7, 19, 9, 30));
        let now = at(2026, 7, 19, 10, 0);
        assert_eq!(
            should_trigger_eye_drop(&state, &st, now),
            SoftDecision::NotTrigger
        );
        // 121 分钟前 > 120 默认间隔 → 触发
        state.last_eye_drop_at = Some(at(2026, 7, 19, 7, 59));
        assert_eq!(
            should_trigger_eye_drop(&state, &st, now),
            SoftDecision::Trigger(SoftPromptKind::EyeDrop)
        );
    }

    #[test]
    fn eye_drop_disabled_when_settings_off() {
        let mut st = default_settings();
        st.care.eye_drop_enabled = false;
        let state = ReminderState::default();
        let now = at(2026, 7, 19, 10, 0);
        assert_eq!(
            should_trigger_eye_drop(&state, &st, now),
            SoftDecision::NotTrigger
        );
    }

    #[test]
    fn eye_drop_silenced_after_3_consecutive_dismissals() {
        let st = default_settings();
        let mut state = ReminderState::default();
        let now = at(2026, 7, 19, 10, 0);
        for i in 0..3 {
            apply_dismiss_soft(
                &mut state,
                SoftPromptKind::EyeDrop,
                now - chrono::Duration::minutes(i * 10),
            );
        }
        assert_eq!(
            should_trigger_eye_drop(&state, &st, now),
            SoftDecision::EyeDropDismissedTooMany
        );
    }

    #[test]
    fn warm_compress_triggers_at_configured_time() {
        let st = default_settings();
        let state = ReminderState::default();
        let now = at(2026, 7, 19, 13, 0);
        assert_eq!(
            should_trigger_warm_compress(&state, &st, now),
            SoftDecision::Trigger(SoftPromptKind::WarmCompress)
        );
        let mut state = ReminderState::default();
        state.last_warm_compress_at = Some(at(2026, 7, 19, 13, 0));
        let later = at(2026, 7, 19, 13, 30);
        assert_eq!(
            should_trigger_warm_compress(&state, &st, later),
            SoftDecision::WarmCompressAlreadyToday
        );
    }

    #[test]
    fn warm_compress_not_triggered_before_time() {
        let st = default_settings();
        let state = ReminderState::default();
        let now = at(2026, 7, 19, 12, 59);
        assert_eq!(
            should_trigger_warm_compress(&state, &st, now),
            SoftDecision::NotTrigger
        );
    }

    #[test]
    fn dismiss_strong_records_last_at_and_hides() {
        let mut state = ReminderState::default();
        let now = at(2026, 7, 19, 10, 20);
        state.active_strong = Some(ActiveStrong {
            started_at: at(2026, 7, 19, 10, 20),
            duration_seconds: 20,
            remaining_seconds: 5,
            mute_sound: false,
            hidden: false,
        });
        let cmds = apply_dismiss_strong(&mut state, now);
        assert!(cmds
            .iter()
            .any(|c| matches!(c, ReminderCommand::HideAllPopups)));
        assert_eq!(state.last_strong_at, Some(now));
        assert!(state.active_strong.is_none());
    }

    #[test]
    fn dismiss_strong_when_no_active_is_noop() {
        let mut state = ReminderState::default();
        let now = at(2026, 7, 19, 10, 20);
        let cmds = apply_dismiss_strong(&mut state, now);
        assert!(cmds.is_empty());
    }

    #[test]
    fn manual_pause_sets_pause_until() {
        let mut state = ReminderState::default();
        let now = at(2026, 7, 19, 10, 0);
        apply_manual_pause(&mut state, 30, now);
        assert_eq!(
            state.pause_until,
            Some(now + chrono::Duration::minutes(30))
        );
    }

    #[test]
    fn pause_until_tomorrow_uses_work_start() {
        let mut state = ReminderState::default();
        let st = default_settings();
        let now = at(2026, 7, 19, 14, 0);
        apply_pause_until_tomorrow(&mut state, &st, now);
        let expected = at(2026, 7, 20, 9, 0);
        assert_eq!(state.pause_until, Some(expected));
    }

    #[test]
    fn parse_hhmm_invalid_falls_back_to_09_00() {
        // 错格式不能让 reminder scheduler panic
        assert_eq!(
            parse_hhmm("not-a-time"),
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        );
        assert_eq!(
            parse_hhmm(""),
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        );
        assert_eq!(
            parse_hhmm("25:99"),
            NaiveTime::from_hms_opt(9, 0, 0).unwrap()
        );
    }

    #[test]
    fn within_work_hours_overnight_range() {
        // 22:00 - 06:00 跨午夜：03:00 仍在工作时段；08:00 不在；22:00 在
        let start = NaiveTime::from_hms_opt(22, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
        assert!(within_work_hours(
            NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
            start,
            end
        ));
        assert!(within_work_hours(
            NaiveTime::from_hms_opt(23, 30, 0).unwrap(),
            start,
            end
        ));
        assert!(!within_work_hours(
            NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            start,
            end
        ));
    }

    #[test]
    fn eye_drop_dismiss_3_times_in_window_blocks_further() {
        let mut state = ReminderState::default();
        let st = default_settings();
        let now = at(2026, 7, 19, 10, 0);

        // 连续 dismiss 三次（不通过 apply_dismiss_soft，模拟真实场景）
        for i in 0..3 {
            state.eye_drop_dismissals.push_back(now - chrono::Duration::minutes(i * 5));
        }

        let d = should_trigger_eye_drop(&state, &st, now);
        assert_eq!(d, SoftDecision::EyeDropDismissedTooMany);
    }

    #[test]
    fn eye_drop_dismiss_older_than_window_purged() {
        let mut state = ReminderState::default();
        let st = default_settings();
        let now = at(2026, 7, 19, 10, 0);

        // T36+：last_eye_drop_at 需要设成 far past，interval 才能满足
        state.last_eye_drop_at = Some(now - chrono::Duration::hours(3));

        // 30 分钟前的 dismiss 应当被 purge（默认 SOFT_DISMISS_WINDOW_MINUTES=30）
        state
            .eye_drop_dismissals
            .push_back(now - chrono::Duration::minutes(45));
        state
            .eye_drop_dismissals
            .push_back(now - chrono::Duration::minutes(35));

        // purge 后队列空 → 可正常触发
        let d = should_trigger_eye_drop(&state, &st, now);
        assert_eq!(d, SoftDecision::Trigger(SoftPromptKind::EyeDrop));
    }

    #[test]
    fn warm_compress_resets_on_new_day() {
        let st = default_settings();
        let mut state = ReminderState::default();

        // 第一天 14:00 触发过
        state.last_warm_compress_at = Some(at(2026, 7, 18, 14, 0));
        // 第二天 13:01 又是新一天 → 应能再次触发
        let now = at(2026, 7, 19, 13, 1);
        let d = should_trigger_warm_compress(&state, &st, now);
        assert_eq!(d, SoftDecision::Trigger(SoftPromptKind::WarmCompress));
    }

    #[test]
    fn warm_compress_blocked_same_day_after_trigger() {
        let st = default_settings();
        let mut state = ReminderState::default();

        state.last_warm_compress_at = Some(at(2026, 7, 19, 13, 5));
        let now = at(2026, 7, 19, 14, 0);
        let d = should_trigger_warm_compress(&state, &st, now);
        assert_eq!(d, SoftDecision::WarmCompressAlreadyToday);
    }

    #[test]
    fn complete_strong_writes_db_and_clears_active() {
        use crate::db::get_today_stats;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "mumu_rem_test_{}_{}",
            std::process::id(),
            id
        ));
        let _ = std::fs::create_dir_all(&dir);
        let db = DbState::new_at(&PathBuf::from(&dir).join("stats.db")).unwrap();

        let mut state = ReminderState {
            active_strong: Some(ActiveStrong {
                started_at: at(2026, 7, 19, 10, 0),
                duration_seconds: 20,
                remaining_seconds: 0,
                mute_sound: false,
                hidden: false,
            }),
            ..Default::default()
        };

        let now = at(2026, 7, 19, 10, 0);
        let cmds = apply_complete_strong(&mut state, &db, now);

        // active 应清空 + last_strong_at 已设
        assert!(state.active_strong.is_none());
        assert_eq!(state.last_strong_at, Some(now));
        // 应返回 HideAllPopups
        assert!(cmds.iter().any(|c| matches!(c, ReminderCommand::HideAllPopups)));

        // apply_complete_strong 内部已经写库：rest_count=1, rest_seconds=20
        let stats = get_today_stats(&db).unwrap();
        assert_eq!(stats.rest_count, 1);
        assert_eq!(stats.rest_seconds, 20);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dismiss_strong_without_active_returns_empty() {
        let mut state = ReminderState::default();
        let now = at(2026, 7, 19, 10, 0);
        // 没 active 强提醒 → 不应有任何命令
        let cmds = apply_dismiss_strong(&mut state, now);
        assert!(cmds.is_empty());
        // last_strong_at 不应被设
        assert!(state.last_strong_at.is_none());
    }

    // ============================================================================
    // T36 compute_snapshot 单测
    // ============================================================================

    #[test]
    fn snapshot_first_launch_returns_full_interval_for_strong() {
        // T36+：首次启动 last=None → next=full interval（之前是 Some(0)"随时可能"）
        let state = ReminderState::default();
        let settings = default_settings();
        let now = at(2026, 7, 22, 10, 0);
        let snap = compute_snapshot(&state, &settings, now);
        assert!(snap.last_strong_at.is_none());
        // 默认 interval_minutes=20 → next=20min=1200s
        assert_eq!(snap.next_strong_secs, Some(20 * 60));
        assert!(snap.in_work_hours);
        assert!(!snap.is_paused);
    }

    #[test]
    fn snapshot_countdown_decreases_one_per_second() {
        // 验证数字每秒减少 1（前端 1Hz 轮询能看到的递减幅度）
        let mut state = ReminderState::default();
        // 上次 09:55，距 now=10:00 已过 5 分钟；剩余 20-5=15 分钟=900 秒
        state.last_strong_at = Some(at(2026, 7, 22, 9, 55));
        let settings = default_settings();
        let now1 = at(2026, 7, 22, 10, 0);
        let now2 = at(2026, 7, 22, 10, 0) + chrono::Duration::seconds(1);
        let s1 = compute_snapshot(&state, &settings, now1);
        let s2 = compute_snapshot(&state, &settings, now2);
        let v1 = s1.next_strong_secs.unwrap();
        let v2 = s2.next_strong_secs.unwrap();
        assert_eq!(v1 - v2, 1, "first call={v1}, second call={v2}");
    }

    #[test]
    fn snapshot_outside_work_hours_yields_none() {
        // 默认设置 work_start=09:00, work_end=18:00；07:00 不在工作时段
        let mut state = ReminderState::default();
        state.last_strong_at = Some(at(2026, 7, 22, 6, 0));
        let settings = default_settings();
        let now = at(2026, 7, 22, 7, 0);
        let snap = compute_snapshot(&state, &settings, now);
        assert!(!snap.in_work_hours);
        assert!(snap.next_strong_secs.is_none());
    }

    #[test]
    fn snapshot_paused_returns_none_for_all() {
        // 暂停中（在工作时段内）：next_* 全 None，is_paused=true
        let mut state = ReminderState::default();
        state.last_strong_at = Some(at(2026, 7, 22, 9, 30));
        state.last_eye_drop_at = Some(at(2026, 7, 22, 9, 30));
        state.pause_until = Some(at(2026, 7, 22, 11, 0));
        let settings = default_settings();
        let now = at(2026, 7, 22, 10, 0);
        let snap = compute_snapshot(&state, &settings, now);
        assert!(snap.is_paused);
        assert!(snap.next_strong_secs.is_none());
        assert!(snap.next_eye_drop_secs.is_none());
        // T36+: 暂停还剩 1h = 3600s
        assert_eq!(snap.pause_remaining_secs, Some(3600));
    }

    #[test]
    fn snapshot_pause_expired_yields_none() {
        // pause_until 已过期（早于 now）→ 不算暂停
        let mut state = ReminderState::default();
        state.pause_until = Some(at(2026, 7, 22, 9, 0));
        let settings = default_settings();
        let now = at(2026, 7, 22, 10, 0);
        let snap = compute_snapshot(&state, &settings, now);
        assert!(!snap.is_paused);
        assert!(snap.pause_remaining_secs.is_none());
    }

    #[test]
    fn snapshot_eye_drop_disabled_returns_none() {
        // 眼药水关掉 → next_eye_drop_secs=None 且 eye_drop_enabled=false
        let mut state = ReminderState::default();
        state.last_eye_drop_at = Some(at(2026, 7, 22, 9, 30));
        let mut settings = default_settings();
        settings.care.eye_drop_enabled = false;
        let now = at(2026, 7, 22, 10, 0);
        let snap = compute_snapshot(&state, &settings, now);
        assert!(!snap.eye_drop_enabled);
        assert!(snap.next_eye_drop_secs.is_none());
        // 强提醒不应受影响
        assert!(snap.next_strong_secs.is_some());
    }

    #[test]
    fn snapshot_after_dismiss_pushes_last_into_future() {
        // apply_dismiss_soft 把 last 推到 now + 30min，模拟"已 dismiss"
        // 默认 eye_drop_interval_minutes=120；last 在 now + 30min → 距下次还剩 120-30=90min
        let mut state = ReminderState::default();
        let now = at(2026, 7, 22, 10, 0);
        state.last_eye_drop_at = Some(at(2026, 7, 22, 10, 30));
        let settings = default_settings();
        let snap = compute_snapshot(&state, &settings, now);
        // 距 last (10:30) - now (10:00) = 30min；下次要在 last + 120min = 12:30
        // 距下次还有 12:30 - 10:00 = 150min = 9000s
        let secs = snap.next_eye_drop_secs.unwrap();
        assert_eq!(secs, 150 * 60);
    }

    #[test]
    fn snapshot_warm_compress_already_today_returns_hint() {
        // 热敷今日 9:00 已触发 → hint=WarmCompressAlreadyToday{next_at_hhmm="13:00"}（默认）
        let mut state = ReminderState::default();
        state.last_warm_compress_at = Some(at(2026, 7, 22, 9, 0));
        let settings = default_settings();
        let now = at(2026, 7, 22, 10, 0);
        let snap = compute_snapshot(&state, &settings, now);
        assert!(snap.next_warm_compress_secs.is_none());
        match &snap.warm_compress_hint {
            Some(NextHint::WarmCompressAlreadyToday { hhmm }) => {
                assert_eq!(hhmm, "13:00")
            }
            other => panic!("expected WarmCompressAlreadyToday, got {other:?}"),
        }
    }

    #[test]
    fn snapshot_eye_drop_dismissed_limit_returns_hint() {
        // 眼药水 dismiss 满 3 次 → hint=EyeDropDismissedLimit{hhmm="09:00"}
        let mut state = ReminderState::default();
        for _ in 0..3 {
            state.eye_drop_dismissals.push_back(at(2026, 7, 22, 10, 0));
        }
        let settings = default_settings();
        let now = at(2026, 7, 22, 10, 0);
        let snap = compute_snapshot(&state, &settings, now);
        assert!(snap.next_eye_drop_secs.is_none());
        match &snap.eye_drop_hint {
            Some(NextHint::EyeDropDismissedLimit { hhmm }) => {
                assert_eq!(hhmm, "09:00")
            }
            other => panic!("expected EyeDropDismissedLimit, got {other:?}"),
        }
    }
}
