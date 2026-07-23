// 沐目 - 护眼工具后端入口
//
// 模块组织：
// - settings:      设置读写（T03）✅
// - db:            SQLite 数据层（T04）✅
// - screen_state:  Windows API 屏幕状态监听（T05）✅
// - statistics:    屏幕使用时长统计（T06）✅
// - reminders:     提醒调度器（T07）✅
// - tray:          系统托盘（T08）✅
// - windows:       窗口管理（弹窗、主界面、设置）（T09-T11）
// - commands:      Tauri invoke 命令桥（T10/T11）

use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
// 模块改为 pub 是为了让 src-tauri/tests/ 下的 integration test 能直接 import
// 不暴露任何 trait / type API，只是把模块名从私有变成可见
pub mod settings;
pub mod db;
pub mod screen_state;
pub mod statistics;
pub mod reminders;
pub mod tray;
pub mod windows;
pub mod commands;

use settings::{
    read_settings_cmd, write_settings_cmd, reset_settings_cmd, Settings, SettingsHandle,
};
use tray::install_tray;
use db::{DbState, DailyStats};
use commands::{
    reminder_skip, reminder_complete, softprompt_dismiss, trigger_test_reminder,
    trigger_test_soft_prompt, get_reminder_status_cmd, resume_reminders_cmd,
    SchedulerControlHandle, sync_autostart,
};

/// 示例 Tauri command（保留以便前端可以快速测试 invoke 通路）
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! 来自沐目 👁", name)
}

/// 拉取今日统计（T09 主界面使用）
#[tauri::command]
fn get_today_stats_cmd(
    state: tauri::State<'_, Arc<DbState>>,
) -> Result<DailyStats, String> {
    db::get_today_stats(&state).map_err(|e| e.to_string())
}

/// 隐藏主窗口（T09 关闭按钮用，不退出进程）
#[tauri::command]
fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 显示并聚焦主窗口（托盘双击从 setup 调用；前端也可调）
#[tauri::command]
fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 启动时读盘拿到用户已有设置；如果文件不存在 / 解析失败则用默认
    let initial_settings = settings::read_settings().unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // T13 开机自启：Mac 用 LaunchAgent（Windows 平台被 plugin 自动忽略），
        // --minimized 让自启时只出现托盘图标，不弹主窗口（更符合"后台护眼工具"定位）
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(Arc::new(DbState::new().expect("init db")))
        .manage(SettingsHandle::new(initial_settings.clone()))
        .invoke_handler(tauri::generate_handler![
            greet,
            read_settings_cmd,
            write_settings_cmd,
            reset_settings_cmd,
            get_today_stats_cmd,
            hide_main_window,
            show_main_window,
            reminder_skip,
            reminder_complete,
            softprompt_dismiss,
            trigger_test_reminder,
            trigger_test_soft_prompt,
            get_reminder_status_cmd,
            resume_reminders_cmd,
        ])
        .setup(move |app| {
            // T08: 安装系统托盘
            install_tray(app.handle())?;

            // ---- T10/T11: 启动后台任务 ----
            // 复用 manage 里注册的同一个 db（同一连接；避免双开文件锁）
            let db: tauri::State<'_, Arc<DbState>> = app.state();
            let db = Arc::clone(&db);

            // 复用 manage 里注册的 SettingsHandle
            let settings_handle: tauri::State<'_, SettingsHandle> = app.state();
            let settings_arc: Arc<tokio::sync::RwLock<Settings>> =
                Arc::clone(&settings_handle.0);

            // T06 → T07 → 事件总线 → 前端
            let (stats_tx, stats_rx) = tokio::sync::mpsc::channel(32);
            let (reminder_tx, mut reminder_rx) = tokio::sync::mpsc::channel(32);
            let (control_tx, control_rx) = tokio::sync::mpsc::channel(8);

            // T36：scheduler state 提到外部 Arc<Mutex>，让 get_reminder_status_cmd 也能读
            let reminder_state = Arc::new(tokio::sync::Mutex::new(reminders::ReminderState::default()));
            app.manage(Arc::clone(&reminder_state));

            // 1) T06 屏幕使用统计
            tauri::async_runtime::spawn(statistics::StatisticsLoop::new(
                Arc::clone(&db),
                stats_tx,
            ).run());

            // 2) T07 提醒调度器
            tauri::async_runtime::spawn(reminders::ReminderScheduler::new(
                Arc::clone(&db),
                Arc::clone(&settings_arc),
                stats_rx,
                reminder_tx,
                control_rx,
            ).run(Arc::clone(&reminder_state)));

            // 3) ReminderCommand → 前端事件总线
            let app_handle_for_translate = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(cmd) = reminder_rx.recv().await {
                    windows::translate_command_to_frontend(&app_handle_for_translate, cmd).await;
                }
            });

            // 4) 暴露 SchedulerControlHandle 给前端 invoke
            app.manage(SchedulerControlHandle(control_tx));

            // T13 启动时同步一次开机自启：用户磁盘上的设置 vs 当前注册表可能不一致
            // （例如首次启动 / 卸载重装 / 用户手动改注册表）
            sync_autostart(&app.handle(), &initial_settings);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

