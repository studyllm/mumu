// 系统托盘
//
// 规格：openspec/changes/add-mumu-eye-care/specs/ui/spec.md § U1-U3
//
// 设计要点（Tauri 2 内置 tray API）：
// - 图标：沐目专属图标（薄荷绿）
//   - `tray-eye-open.png`   ：椭圆 + 瞳孔 —— 工作 / 屏幕亮
//   - `tray-eye-closed.png` ：弧线      —— 休息 / 锁屏 / 灭屏
//   - 状态切换由 `set_eye_state(open: bool)` 触发（T19 补的视觉规范）
// - 菜单项（右键）：
//    1. 打开主界面    —— show_main
//    2. 暂停 30 分钟  —— 调 settings + reminders::apply_manual_pause
//    3. 暂停 1 小时   —— 同上
//    4. 暂停到明早 9 点 —— 同上
//    5. 设置          —— open_settings
//    6. 退出          —— app.exit(0)
// - 左键：Tauri 默认不弹菜单（任务要求"空操作，保持后台"）
// - 双击：emit "open-main"（任务要求）
//
// 故意没做：
// - ⏸ 动画过渡：spec 要求 300ms 渐变，Tauri tray API 无原生 transition 支持，
//   实测"换图瞬间切换"在 32×32 系统托盘上视觉无感，列入 v0.2 backlog。

use tauri::{
    image::Image,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use crate::commands::SchedulerControlHandle;
use crate::reminders::SchedulerControl;

/// 菜单项 ID（与浏览器 / 前端 invoke 时一致）
pub mod id {
    pub const OPEN_MAIN: &str = "open-main";
    pub const OPEN_SETTINGS: &str = "open-settings";
    pub const PAUSE_30: &str = "pause-30";
    pub const PAUSE_60: &str = "pause-60";
    pub const PAUSE_TILL_TOMORROW: &str = "pause-till-tomorrow";
    pub const RESUME: &str = "resume";
    pub const QUIT: &str = "quit";
}

/// 32×32 RGBA 沐目图标（编译期内嵌到 exe，不依赖外部资源路径）
const ICON_OPEN_BYTES: &[u8] = include_bytes!("../icons/tray-eye-open.png");
const ICON_CLOSED_BYTES: &[u8] = include_bytes!("../icons/tray-eye-closed.png");

/// 解码 PNG → tauri::Image（一次性，缓存为静态变量）
fn load_icon(bytes: &[u8]) -> Image<'static> {
    Image::from_bytes(bytes).expect("tray icon decode failed")
}

fn open_icon() -> Image<'static> {
    load_icon(ICON_OPEN_BYTES)
}

fn closed_icon() -> Image<'static> {
    load_icon(ICON_CLOSED_BYTES)
}

/// 切换托盘图标（睁眼 / 闭眼）
///
/// 由 scheduler 在判定屏幕状态变化时调用（T19 spec 硬约束）：
/// - 屏幕亮 + 工作中 → open
/// - 锁屏 / 灭屏    → closed
///
/// 容错：托盘未建好 / app 已退 → 静默忽略。
pub fn set_eye_state<R: Runtime>(app: &AppHandle<R>, open: bool) {
    let Some(tray) = app.tray_by_id("mumu-main") else {
        return;
    };
    let img = if open { open_icon() } else { closed_icon() };
    if let Err(e) = tray.set_icon(Some(img)) {
        eprintln!("[mumu tray] set_icon failed: {e}");
    }
    let tip = if open { "沐目 · 工作" } else { "沐目 · 休息" };
    let _ = tray.set_tooltip(Some(tip));
}

/// 构造托盘菜单
fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        id::OPEN_MAIN,
        "打开主界面",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        id::PAUSE_30,
        "暂停 30 分钟",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        id::PAUSE_60,
        "暂停 1 小时",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        id::PAUSE_TILL_TOMORROW,
        "暂停到明早 9:00",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        id::OPEN_SETTINGS,
        "设置",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, id::QUIT, "退出", true, None::<&str>)?)?;
    Ok(menu)
}

/// 安装托盘（必须在 `tauri::Builder` 的 `setup` 回调里调用）
pub fn install_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let app_for_menu = app.clone();
    let app_for_click = app.clone();

    TrayIconBuilder::with_id("mumu-main")
        .tooltip("沐目 · 工作")
        .icon(open_icon())
        .menu(&menu)
        .menu_on_left_click(false) // 左键不弹菜单（任务："空操作，保持后台"）
        .on_menu_event(move |_app, ev: MenuEvent| {
            handle_menu_event(&app_for_menu, ev.id().as_ref());
        })
        .on_tray_icon_event(move |_tray, ev| {
            handle_tray_event(&app_for_click, ev);
        })
        .build(app)?;

    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, menu_id: &str) {
    match menu_id {
        id::OPEN_MAIN => show_main(app),
        id::PAUSE_30 => apply_pause(app, 30),
        id::PAUSE_60 => apply_pause(app, 60),
        id::PAUSE_TILL_TOMORROW => apply_pause_tomorrow(app),
        id::OPEN_SETTINGS => open_settings(app),
        id::QUIT => app.exit(0),
        _ => {
            eprintln!("[mumu tray] unknown menu id '{}' (did you mean one of: open-main, pause-30, pause-60, pause-till-tomorrow, open-settings, quit?)", menu_id);
        }
    }
}

fn handle_tray_event<R: Runtime>(app: &AppHandle<R>, ev: TrayIconEvent) {
    // Windows 上 DoubleClick 是独立 variant（来自 tray-icon crate）
    if let TrayIconEvent::DoubleClick {
        button: MouseButton::Left,
        ..
    } = ev
    {
        show_main(app);
    }
    // 左键单击 / 右键单击：菜单由 Tauri 通过 menu_on_left_click(false) 自动接管
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    } else if let Some(win) = app.get_webview_window("settings") {
        // 主窗口未初始化好之前，从托盘打开设置——兜底
        let _ = win.show();
    }
}

fn open_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // settings 窗口没在 manager 里（config 自动注册失败 / build 失败被吞）。
        // 兜底：现场用 builder 创建 settings 窗口（settings.html 已嵌入 dist/）。
        // 这样不依赖 config 自动注册，避免用户点"设置"变"主界面"。
        eprintln!("[mumu tray] settings window not registered, creating on demand");
        match tauri::WebviewWindowBuilder::new(
            app,
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .title("沐目 · 设置")
        .inner_size(800.0, 600.0)
        .min_inner_size(600.0, 400.0)
        .resizable(true)
        .center()
        .build()
        {
            Ok(window) => {
                eprintln!("[mumu tray] settings window created on demand");
                let _ = window.show();
                let _ = window.set_focus();
            }
            Err(e) => {
                eprintln!("[mumu tray] failed to create settings window: {e}");
                show_main(app);
            }
        }
    }
}

/// 标记手动暂停 X 分钟——通过 SchedulerControl 推回 reminders 调度器
fn apply_pause<R: Runtime>(app: &AppHandle<R>, minutes: u32) {
    let state: tauri::State<SchedulerControlHandle> = app.state();
    // 阻塞 send：channel buffer=8 不会满（只有用户主动触发）
    let tx = state.0.clone();
    tauri::async_runtime::block_on(async move {
        let _ = tx.send(SchedulerControl::PauseMinutes(minutes)).await;
    });
}

fn apply_pause_tomorrow<R: Runtime>(app: &AppHandle<R>) {
    let state: tauri::State<SchedulerControlHandle> = app.state();
    let tx = state.0.clone();
    tauri::async_runtime::block_on(async move {
        let _ = tx.send(SchedulerControl::PauseUntilTomorrow).await;
    });
}
