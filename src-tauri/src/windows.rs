// 窗口管理（T09/T10/T10.5/T11）
//
// 强提醒弹窗（T10）：
// - 320×200，定位主显示器右下角 24px 偏移
// - alwaysOnTop / skipTaskbar / focus:false（不抢焦点）
// - 半透明背景 + 20px backdrop blur + 12px 圆角（CSS 见 ReminderPopup.tsx）
//
// 设计：纯 helper，不持有状态；由 reminders 调度器经 lib.rs 的
// ReminderCommand → 前端事件总线调用。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime};

use crate::reminders::{ReminderCommand, SoftPromptKind};
use crate::tray;

/// 强提醒 payload（与前端 ShowStrongReminderPayload 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowStrongReminderPayload {
    pub duration_seconds: u32,
    /// T10 保留字段：跨工作时段（离下班 20 分钟内）静默
    pub mute_sound: bool,
    /// T12 新增：是否应播放木鱼声（综合用户设置 + mute_sound）
    pub play_sound: bool,
}

/// 弱提示 payload（与前端 ShowSoftPromptPayload 对齐）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowSoftPromptPayload {
    pub kind: SoftPromptKind,
    pub message: String,
}

/// 边缘偏移（spec: 24px from edges）
const EDGE_OFFSET_PX: i32 = 24;

/// 窗口尺寸（与 tauri.conf.json 一致）
const REMINDER_W: i32 = 320;
const REMINDER_H: i32 = 200;
const SOFTPROMPT_W: i32 = 280;
const SOFTPROMPT_H: i32 = 80;

/// 显示强提醒弹窗
pub fn show_reminder_popup<R: Runtime>(app: &AppHandle<R>, payload: ShowStrongReminderPayload) {
    let Some(window) = app.get_webview_window("reminder") else {
        eprintln!("[mumu] reminder window not registered");
        return;
    };

    // 计算主显示器右下角位置
    if let Some(pos) = bottom_right_position(&window, REMINDER_W, REMINDER_H, EDGE_OFFSET_PX) {
        if let Err(e) = window.set_position(pos) {
            eprintln!("[mumu] set_position failed: {e}");
        }
    }

    if let Err(e) = window.emit_to("reminder", "show-strong-reminder", payload) {
        eprintln!("[mumu] emit show-strong-reminder failed: {e}");
    }
    if let Err(e) = window.show() {
        eprintln!("[mumu] reminder window show failed: {e}");
    }
}

/// 隐藏强提醒弹窗（同时下发 hide-all-popups 事件给前端）
pub fn hide_reminder_popup<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("reminder") else {
        return;
    };
    let _ = window.emit_to("reminder", "hide-all-popups", ());
    let _ = window.hide();
}

/// 显示弱提示弹窗（眼药水 / 热敷）
pub fn show_soft_prompt<R: Runtime>(app: &AppHandle<R>, payload: ShowSoftPromptPayload) {
    let Some(window) = app.get_webview_window("softprompt") else {
        eprintln!("[mumu] softprompt window not registered");
        return;
    };

    if let Some(pos) =
        bottom_right_position(&window, SOFTPROMPT_W, SOFTPROMPT_H, EDGE_OFFSET_PX)
    {
        if let Err(e) = window.set_position(pos) {
            eprintln!("[mumu] set_position failed: {e}");
        }
    }

    if let Err(e) = window.emit_to("softprompt", "show-soft-prompt", payload) {
        eprintln!("[mumu] emit show-soft-prompt failed: {e}");
    }
    if let Err(e) = window.show() {
        eprintln!("[mumu] softprompt window show failed: {e}");
    }
}

/// 隐藏弱提示弹窗
pub fn hide_soft_prompt<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("softprompt") else {
        return;
    };
    let _ = window.emit_to("softprompt", "hide-soft-prompt", ());
    let _ = window.hide();
}

/// 计算窗口在 webview 所属显示器右下角"可工作区"内的位置（已乘 scale factor）
///
/// T23 修复：
/// - 改用 `work_area()` 而非 `size()` —— 避开任务栏（Dock），弹窗显示在任务栏**上方**
/// - 改用 `current_monitor()` 而非 `primary_monitor()` —— 弹窗在哪个显示器就定位到哪个
///   （多屏环境修复：副屏打开设置时点"测试提醒"，旧逻辑把弹窗定位到主屏，导致看不见）
/// - `offset` 已乘 `scale_factor`，统一为物理像素（webview set_position 用物理像素）
fn bottom_right_position<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    width: i32,
    height: i32,
    offset: i32,
) -> Option<PhysicalPosition<i32>> {
    // 用 webview 所属 monitor 而非全屏 primary —— 避免多屏时弹窗跑到错的屏
    let monitor = window.current_monitor().ok().flatten()?;
    let work = monitor.work_area();
    let pos = bottom_right_in_monitor(
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
        monitor.scale_factor(),
        width,
        height,
        offset,
    );
    Some(pos)
}

/// pure 函数：根据 monitor 信息计算右下角位置
///
/// 抽出来便于单测。参数顺序：
/// - `work_x/y/w/h`：该显示器"可工作区"（去掉任务栏）的**物理像素**绝对位置 + 尺寸
/// - `scale`：DPI 缩放（逻辑→物理倍数）
/// - `width/height/offset`：弹窗**逻辑像素**尺寸 + 边距
///
/// 返回**物理像素**位置（webview set_position 用物理像素）
fn bottom_right_in_monitor(
    work_x: i32, work_y: i32, work_w: u32, work_h: u32,
    scale: f64, width: i32, height: i32, offset: i32,
) -> PhysicalPosition<i32> {
    // 弹窗占的物理像素 = 逻辑像素 × scale（先乘后 round，避免 f64 as i32 截断）
    let popup_w_phys = ((width + offset) as f64 * scale).round() as i32;
    let popup_h_phys = ((height + offset) as f64 * scale).round() as i32;
    let work_right = work_x + work_w as i32;
    let work_bottom = work_y + work_h as i32;
    // saturating_sub 防 i32 underflow；再 max(0, ...) 把"弹窗比屏大"的负数 clamp 到 0
    // （弹窗左上角至少在屏内，右下被裁剪比跑屏外好）
    let x = work_right.saturating_sub(popup_w_phys).max(0);
    let y = work_bottom.saturating_sub(popup_h_phys).max(0);
    PhysicalPosition { x, y }
}

/// 把 ReminderCommand 翻译成前端事件 + 窗口动作
///
/// T10：ShowStrongReminder / HideAllPopups / Log
/// T10.5：ShowSoftPrompt
/// T19：托盘图标状态切换（HideAllPopups → 闭眼；其他 → 睁眼）
pub async fn translate_command_to_frontend<R: Runtime>(app: &AppHandle<R>, cmd: ReminderCommand) {
    match cmd {
        ReminderCommand::ShowStrongReminder {
            duration_seconds,
            mute_sound,
            play_sound,
        } => {
            tray::set_eye_state(app, true);
            show_reminder_popup(
                app,
                ShowStrongReminderPayload {
                    duration_seconds,
                    mute_sound,
                    play_sound,
                },
            );
        }
        ReminderCommand::HideAllPopups => {
            tray::set_eye_state(app, false);
            hide_reminder_popup(app);
            hide_soft_prompt(app);
        }
        ReminderCommand::TrayEyeState { open } => {
            tray::set_eye_state(app, open);
        }
        ReminderCommand::ShowSoftPrompt { kind, message } => {
            tray::set_eye_state(app, true);
            show_soft_prompt(
                app,
                ShowSoftPromptPayload {
                    kind,
                    message,
                },
            );
        }
        ReminderCommand::Log { level, message } => {
            eprintln!("[mumu][{}] {}", level, message);
        }
    }
}

// ============================================================================
// Tests — T23 修复验证：弹窗定位避开任务栏 + 副屏场景
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 主屏 1920×1080 100% DPI（scale=1.0），任务栏在底部 40px
    /// work_area 物理像素 = 1920×1040
    /// 弹窗 320×200 + 24px offset（逻辑）→ 物理 344×224
    /// 期望落点 (1920-344, 1040-224) = (1576, 816)
    #[test]
    fn primary_monitor_100pct_dock_at_bottom() {
        let pos = bottom_right_in_monitor(
            0, 0, 1920, 1040,        // work_area（物理像素，1920×1080 屏 - 底部 40px 任务栏）
            1.0,                     // scale
            320, 200, 24,            // 弹窗逻辑尺寸 + 边距
        );
        assert_eq!(pos.x, 1920 - (320 + 24));  // 1576
        assert_eq!(pos.y, 1040 - (200 + 24));  // 816
    }

    /// HiDPI 125%（scale=1.25）
    /// 显示器 1920×1080 逻辑，物理 = 2400×1350
    /// work_area 物理像素 = 2400×1300（底部 50px 物理任务栏）
    /// 弹窗 320×200 + 24px offset（逻辑）→ 物理 430×280
    /// 期望落点 (2400-430, 1300-280) = (1970, 1020)
    #[test]
    fn hidpi_125pct_scales_correctly() {
        let pos = bottom_right_in_monitor(
            0, 0, 2400, 1300,        // work_area（物理像素）
            1.25,
            320, 200, 24,            // 弹窗逻辑尺寸 + 边距
        );
        // (320+24) * 1.25 = 430, (200+24) * 1.25 = 280
        assert_eq!(pos.x, 2400 - ((320 + 24) as f64 * 1.25).round() as i32);
        assert_eq!(pos.y, 1300 - ((200 + 24) as f64 * 1.25).round() as i32);
    }

    /// **多显示器场景**（T23 修复核心）：
    /// 副屏在主屏右侧，物理像素 1920×1040，绝对坐标 x=1920
    /// 旧逻辑用 primary_monitor() → 弹窗定位到主屏 → 用户在副屏看不见
    /// 新逻辑用 current_monitor() + work_area 绝对坐标 → 弹窗落在副屏右下角
    #[test]
    fn secondary_monitor_right_of_primary() {
        let pos = bottom_right_in_monitor(
            1920, 0, 1920, 1040,     // 副屏 work_area（绝对 x=1920）
            1.0,
            320, 200, 24,
        );
        // 期望落在副屏右下角：1920 + (1920-344) = 3496, 1040-224 = 816
        assert_eq!(pos.x, 1920 + 1920 - 344);
        assert_eq!(pos.y, 1040 - 224);
    }

    /// 任务栏在屏幕**顶部**（macOS 风格 / 用户自定义）→ work_area.y > 0
    /// 旧逻辑：work_area.size.height 是 1000，但用 work_y=0 当起点会落到任务栏下方被遮
    /// 新逻辑：用 work_area 起点 (0, 40)，弹窗落在 y = 40+1000-224 = 816
    #[test]
    fn dock_at_top_not_overlapped() {
        let pos = bottom_right_in_monitor(
            0, 40, 1920, 1000,       // work_area 顶部 40px 是任务栏
            1.0,
            320, 200, 24,
        );
        assert_eq!(pos.x, 1920 - 344);
        assert_eq!(pos.y, 40 + 1000 - 224);
    }

    /// 防回归：saturating_sub + max(0, ...) 避免弹窗跑屏外
    /// work_area 100×100，弹窗占 344×224 物理 → 屏放不下，clamp 到 (0, 0) 至少左上角可见
    #[test]
    fn small_screen_saturates_to_zero() {
        let pos = bottom_right_in_monitor(
            0, 0, 100, 100,
            1.0,
            320, 200, 24,
        );
        // 100 - 344 = -244 → max(0, ...) = 0
        assert_eq!(pos.x, 0);
        assert_eq!(pos.y, 0);
    }
}
