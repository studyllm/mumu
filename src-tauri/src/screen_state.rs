// Windows API 屏幕状态监听
//
// 规格：openspec/changes/add-mumu-eye-care/specs/statistics/spec.md
//
// 实现要点：
// - 全模块 #[cfg(windows)] 守卫，非 Windows 平台为编译占位
// - 提供主动查询函数给 T06 消费（每 30 秒轮询一次）
// - 不实现消息循环（事件 push 模型）——MVP 范围内轮询足够，
//   若后续优化可加独立线程跑 GetMessage/DispatchMessage + WM_POWERBROADCAST 等
//
// 实现状态检测：
// - is_screen_on()       通过 SendMessage(HWND_BROADCAST, WM_SYSCOLORCHANGE) 或更稳的
//                       EnumDisplayMonitors 计数判断（无显示器时返回 false）
// - is_user_locked()     通过 OpenInputDesktop 对比 "Default" desktop 名
// - is_user_idle(thresh) 通过 GetLastInputInfo + GetTickCount 计算空闲毫秒
// - is_fullscreen_app()  通过 GetForegroundWindow 窗口矩形 vs 所在 monitor 矩形
//
// 复杂调用全部 unsafe + 静默处理（失败 → 返回 false / 0），
// 避免拖垮 T06 主循环

#[cfg(windows)]
use std::mem;
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST, HDC,
};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};
#[cfg(windows)]
use windows::Win32::System::SystemInformation::GetTickCount;

/// 判定屏幕是否处于"被使用"状态
///
/// T06 状态机的主输入：
/// - Active: 当前在用机（屏亮 + 未锁屏 + 未超 30min 空闲 + 未休眠）
/// - ScreenOff: 显示器关闭
/// - Locked: 工作站锁定
/// - Idle: 超阈值无输入
#[cfg(windows)]
pub fn is_screen_on() -> bool {
    // MVP 简化：只判断是否有显示器；电源状态（屏熄/屏保）由 T06 在第一个状态周期判定
    count_visible_monitors() > 0
}

/// 工作站是否被锁定（Win+L 或屏保触发的锁屏）
///
/// T29 修复：T28 用 GetThreadDesktop vs OpenInputDesktop 句柄对比，
/// 但 Tauri 主进程的 tokio worker thread 可能在不同 desktop session
/// （后台进程跑在 system session），导致两句柄永远不同，
/// 锁屏检测一直返回 true → 屏幕使用时长停摆。
///
/// **可靠信号组合**（任一满足即视为锁屏）：
/// 1. 屏幕关闭（`is_screen_on() == false`）——锁屏进入屏保/息屏
/// 2. 前台窗口**进程名**是锁屏应用（`LockApp.exe` / `LogonUI.exe`）
///    ——这是最可靠的官方锁屏信号（不受 session 隔离影响）
/// 3. 长时间 idle + 前台窗口不可见（兜底，覆盖早期 Windows）
///
/// **保守降级**：API 失败一律返回 false，避免误判锁屏让统计停摆。
#[cfg(windows)]
pub fn is_user_locked() -> bool {
    // 信号 1：屏幕关闭
    if !is_screen_on() {
        return true;
    }

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == HWND(std::ptr::null_mut()) {
            // 没前台窗口：可能是锁屏也可能是 headless 服务。
            // 配合 idle 阈值兜底判断（5 秒无输入 + 无窗口 → 锁屏）
            return idle_milliseconds() > 5_000;
        }

        // 信号 2：前台窗口进程名是锁屏应用
        if let Some(proc_name) = foreground_process_name(hwnd) {
            let lower = proc_name.to_ascii_lowercase();
            // Win10/11 现代锁屏
            if lower == "lockapp.exe" {
                return true;
            }
            // Win7 / 部分旧版本锁屏
            if lower == "logonui.exe" {
                return true;
            }
        }

        // 信号 3：长时间 idle + 窗口不可见（锁屏后某些场景）
        if !IsWindowVisible(hwnd).as_bool() && idle_milliseconds() > 5_000 {
            return true;
        }
    }

    false
}

/// 取前台窗口所属进程的可执行文件名（小写），用于锁屏检测
///
/// 例：返回 `"explorer.exe"`、`"chrome.exe"`、`"lockapp.exe"` 等
#[cfg(windows)]
unsafe fn foreground_process_name(hwnd: HWND) -> Option<String> {
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let mut pid: u32 = 0;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return None;
    }

    let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 512];
    let mut size = buf.len() as u32;
    let pwstr = windows::core::PWSTR(buf.as_mut_ptr());
    let ok = QueryFullProcessImageNameW(process_handle, PROCESS_NAME_WIN32, pwstr, &mut size);
    let _ = windows::Win32::Foundation::CloseHandle(process_handle);
    ok.ok()?;

    let path = String::from_utf16_lossy(&buf[..size as usize]);
    // 取路径最后一段（文件名）
    path.rsplit('\\').next().map(|s| s.to_string())
}

/// 距上次用户输入的毫秒数（鼠标或键盘）
#[cfg(windows)]
pub fn idle_milliseconds() -> u32 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        let ok = GetLastInputInfo(&mut info);
        if !ok.as_bool() {
            return 0;
        }
        let now = GetTickCount();
        now.saturating_sub(info.dwTime)
    }
}

/// 是否超过给定阈值空闲（毫秒）
#[cfg(windows)]
pub fn is_user_idle(threshold_ms: u32) -> bool {
    idle_milliseconds() >= threshold_ms
}

/// 前台窗口是否覆盖全屏（视频/游戏/演示时算 Active，即使 idle）
///
/// 算法：
/// 1. 拿前台窗口句柄
/// 2. 拿它所在 monitor 矩形
/// 3. 拿窗口矩形
/// 4. 窗口面积 ≥ 95% 显示器面积 → 全屏
#[cfg(windows)]
pub fn is_fullscreen_foreground() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == HWND(std::ptr::null_mut()) {
            return false;
        }
        // 拿到前台窗口线程 ID，并获取前台窗口进程信息（用于排除沐目自己的窗口）
        let _thread_id = GetWindowThreadProcessId(hwnd, None);
        // 前台窗口若不可见，跳过
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        // 窗口矩形
        let mut window_rect: RECT = unsafe { mem::zeroed() };
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }
        // 显示器矩形
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_invalid() {
            return false;
        }
        let mut monitor_info = MONITORINFO {
            cbSize: mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let ok = GetMonitorInfoW(monitor, &mut monitor_info);
        if !ok.as_bool() {
            return false;
        }

        // 排除沐目自己的窗口 —— 通过窗口标题简单判断（"沐目"）
        let title = read_window_title(hwnd);
        if title == "沐目" {
            return false;
        }

        let win_area = (window_rect.right - window_rect.left) * (window_rect.bottom - window_rect.top);
        let mon_area = (monitor_info.rcMonitor.right - monitor_info.rcMonitor.left)
            * (monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top);

        if mon_area <= 0 {
            return false;
        }
        // 95% 阈值：留 5% 容差给窗口边框 / 系统 UI 抖动
        (win_area as f64 / mon_area as f64) >= 0.95
    }
}

/// 读窗口标题（仅 Win32 内部使用）
#[cfg(windows)]
unsafe fn read_window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = GetWindowTextW(hwnd, &mut buf);
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

/// 枚举可见显示器数量（screen_on 主判断依据）
#[cfg(windows)]
unsafe extern "system" fn count_monitor_proc(
    _hmonitor: windows::Win32::Graphics::Gdi::HMONITOR,
    _hdc: windows::Win32::Graphics::Gdi::HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> windows::Win32::Foundation::BOOL {
    unsafe {
        let count = &mut *(data.0 as *mut i32);
        *count += 1;
        windows::Win32::Foundation::BOOL(1) // 继续枚举
    }
}

#[cfg(windows)]
fn count_visible_monitors() -> i32 {
    let mut count = 0i32;
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(count_monitor_proc),
            LPARAM(&mut count as *mut _ as isize),
        );
    }
    count
}

/// 综合状态枚举（T06 直接消费）
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenState {
    /// 当前在用（亮屏 + 未锁 + 未空闲 + 屏幕保护未运行）
    Active,
    /// 显示器关闭
    ScreenOff,
    /// 工作站锁定
    Locked,
    /// 超过 30 分钟无输入（视频/游戏已被 Fullscreen 判定，落到 Active）
    Idle,
}

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenState {
    Active,
}

/// 非 Windows 占位实现（开发环境或 Mac 编译时不报错）
#[cfg(not(windows))]
pub fn is_screen_on() -> bool { true }
#[cfg(not(windows))]
pub fn is_user_locked() -> bool { false }
#[cfg(not(windows))]
pub fn idle_milliseconds() -> u32 { 0 }
#[cfg(not(windows))]
pub fn is_user_idle(_threshold_ms: u32) -> bool { false }
#[cfg(not(windows))]
pub fn is_fullscreen_foreground() -> bool { false }
#[cfg(not(windows))]
pub fn ScreenState_default_active() -> ScreenState { ScreenState::Active }

// ============================================================================
// 综合判定（T06 直接调用）
// ============================================================================

/// 一次性读取并归类当前屏幕状态
#[cfg(windows)]
pub fn current_screen_state(idle_threshold_ms: u32) -> ScreenState {
    if !is_screen_on() {
        return ScreenState::ScreenOff;
    }
    if is_user_locked() {
        return ScreenState::Locked;
    }
    // 全屏应用优先：即使 idle 也算 Active
    if is_fullscreen_foreground() {
        return ScreenState::Active;
    }
    if is_user_idle(idle_threshold_ms) {
        return ScreenState::Idle;
    }
    ScreenState::Active
}

#[cfg(not(windows))]
pub fn current_screen_state(_idle_threshold_ms: u32) -> ScreenState {
    ScreenState::Active
}

// ============================================================================
// 单元测试
// ============================================================================
// 注意：实际 Windows API 调用无法在 cargo test 内单测，
// 这些是 sanity 校验（类型 / 编译 / 默认占位）

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_milliseconds_is_non_negative() {
        // 即便失败也不应 panic；返回 0 是合法降级
        let ms = idle_milliseconds();
        assert!(ms < u32::MAX);
    }

    #[test]
    fn fullscreen_foreground_does_not_panic_on_foreground_window() {
        // 验证：在 IDE 自动化运行这个 case 时，前台窗口可能是终端 / 编辑器，
        // 不应有 panic；返回 bool 即可
        let _ = is_fullscreen_foreground();
    }

    #[test]
    fn current_screen_state_returns_valid_variant() {
        let state = current_screen_state(30 * 60 * 1000);
        // 只是确认能跑通，值取决于测试环境
        match state {
            ScreenState::Active | ScreenState::ScreenOff | ScreenState::Locked | ScreenState::Idle => {}
        }
    }
}
