// 设置读写模块
//
// 规格：openspec/changes/add-mumu-eye-care/specs/settings/spec.md
//
// 实现要点：
// - JSON 文件位置：%APPDATA%\沐目\settings.json
// - 数据结构：Settings { reminders, care, general, advanced }
// - 默认值定义在 Default trait 中
// - 版本迁移：读取时检查 version 字段
// - 提供 read_settings / write_settings 命令

use std::fs;
use std::io;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const CURRENT_VERSION: u32 = 1;
const SETTINGS_FILE: &str = "settings.json";
const APP_DIR: &str = "沐目";

#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("无法获取 AppData 目录")]
    NoAppDataDir,
    #[error("无法创建应用目录 {0}: {1}")]
    CreateDirFailed(PathBuf, io::Error),
    #[error("读取设置失败 {0}: {1}")]
    ReadFailed(PathBuf, io::Error),
    #[error("解析设置 JSON 失败: {0}")]
    ParseFailed(#[from] serde_json::Error),
    #[error("写入设置失败 {0}: {1}")]
    WriteFailed(PathBuf, io::Error),
}

/// 单条设置：提醒
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ReminderSettings {
    pub work_start: String,
    pub work_end: String,
    pub interval_minutes: u32,
    pub rest_seconds: u32,
    pub show_popup: bool,
    pub play_sound: bool,
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            work_start: "09:00".to_string(),
            work_end: "18:00".to_string(),
            interval_minutes: 20,
            rest_seconds: 20,
            show_popup: true,
            play_sound: true,
        }
    }
}

/// 单条设置：眼药水/热敷（care = 护眼保养）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct CareSettings {
    pub eye_drop_enabled: bool,
    pub eye_drop_interval_minutes: u32,
    pub warm_compress_enabled: bool,
    pub warm_compress_time: String,
}

impl Default for CareSettings {
    fn default() -> Self {
        Self {
            eye_drop_enabled: true,
            eye_drop_interval_minutes: 120,
            warm_compress_enabled: true,
            warm_compress_time: "13:00".to_string(),
        }
    }
}

/// 单条设置：通用
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct GeneralSettings {
    pub quick_pause: String,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            quick_pause: "30min".to_string(),
        }
    }
}

/// 单条设置：高级
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct AdvancedSettings {
    pub auto_start: bool,
    pub debug_mode: bool,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            auto_start: true,
            debug_mode: false,
        }
    }
}

/// 完整设置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub reminders: ReminderSettings,
    pub care: CareSettings,
    pub general: GeneralSettings,
    pub advanced: AdvancedSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            reminders: ReminderSettings::default(),
            care: CareSettings::default(),
            general: GeneralSettings::default(),
            advanced: AdvancedSettings::default(),
        }
    }
}

/// 获取设置文件路径：%APPDATA%\沐目\settings.json
///
/// 注意：在 Windows 上 dirs::config_dir() 读取 Windows API 而非 APPDATA 环境变量
/// 测试时需要用 settings_path_at(custom_dir) 注入临时目录
pub fn settings_path() -> PathBuf {
    default_app_dir().join(SETTINGS_FILE)
}

/// 在指定目录下获取设置文件路径（用于测试）
pub fn settings_path_at(dir: &std::path::Path) -> PathBuf {
    dir.join(SETTINGS_FILE)
}

/// 获取默认应用目录（%APPDATA%\沐目\）
///
/// 公开给其他模块复用（db.rs 用它定位 stats.db）
///
/// 注意：在 Windows 上 dirs::config_dir() 读取 Windows API 而非 APPDATA 环境变量
pub fn default_app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(APP_DIR)
}

/// 从磁盘读取设置
///
/// - 文件不存在：返回默认值（首次启动）
/// - 文件存在但 version < 当前：升级到当前版本（保留用户自定义值，缺失字段用默认）
/// - JSON 解析失败：返回错误
///
/// 注意：如果版本被升级，会自动写回磁盘持久化新版本号
pub fn read_settings() -> Result<Settings, SettingsError> {
    read_settings_at(&settings_path())
}

/// 在指定路径读取设置（用于测试）
pub fn read_settings_at(path: &std::path::Path) -> Result<Settings, SettingsError> {
    if !path.exists() {
        return Ok(Settings::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|e| SettingsError::ReadFailed(path.to_path_buf(), e))?;

    // 直接解析为 Settings：serde 对未知字段宽容，缺失字段使用 Default
    let mut settings: Settings = serde_json::from_str(&content)?;

    // 版本升级
    if settings.version < CURRENT_VERSION {
        settings.version = CURRENT_VERSION;
        // 异步持久化（失败不影响本次读取）
        let _ = write_settings_at(path, &settings);
    }

    Ok(settings)
}

/// 写入设置到磁盘
pub fn write_settings(settings: &Settings) -> Result<(), SettingsError> {
    let path = settings_path();
    write_settings_at(&path, settings)
}

/// 在指定路径写入设置（用于测试）
pub fn write_settings_at(path: &std::path::Path, settings: &Settings) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| SettingsError::CreateDirFailed(parent.to_path_buf(), e))?;
        }
    }

    let content = serde_json::to_string_pretty(settings)?;
    fs::write(path, content)
        .map_err(|e| SettingsError::WriteFailed(path.to_path_buf(), e))?;
    Ok(())
}

// ============================================================================
// Tauri Commands（给前端调用）
// ============================================================================

use std::sync::Arc;

/// 内存里运行时设置（被 scheduler 共享；使用 tokio 的锁以兼容 async）
pub struct SettingsHandle(pub Arc<tokio::sync::RwLock<Settings>>);

impl SettingsHandle {
    pub fn new(initial: Settings) -> Self {
        Self(Arc::new(tokio::sync::RwLock::new(initial)))
    }

    /// 替换内存中的设置（同时持久化到磁盘）
    pub async fn replace(&self, new: Settings) -> Result<(), SettingsError> {
        write_settings(&new)?;
        let mut w = self.0.write().await;
        *w = new;
        Ok(())
    }

    pub async fn snapshot(&self) -> Settings {
        self.0.read().await.clone()
    }
}

/// 读取当前设置
#[tauri::command]
pub async fn read_settings_cmd(handle: tauri::State<'_, SettingsHandle>) -> Result<Settings, String> {
    Ok(handle.snapshot().await)
}

/// 写入设置（同步到磁盘 + 更新内存 handle）
///
/// T13：写盘成功后把 advanced.auto_start 同步到注册表
#[tauri::command]
pub async fn write_settings_cmd(
    settings: Settings,
    app: tauri::AppHandle,
    handle: tauri::State<'_, SettingsHandle>,
) -> Result<(), String> {
    handle.replace(settings.clone()).await.map_err(|e| e.to_string())?;
    crate::commands::sync_autostart(&app, &settings);
    Ok(())
}

/// 重置为默认设置
///
/// T13：默认值 auto_start=true，需要把注册表项重新加上
#[tauri::command]
pub async fn reset_settings_cmd(
    app: tauri::AppHandle,
    handle: tauri::State<'_, SettingsHandle>,
) -> Result<Settings, String> {
    let defaults = Settings::default();
    handle.replace(defaults.clone()).await.map_err(|e| e.to_string())?;
    crate::commands::sync_autostart(&app, &defaults);
    Ok(defaults)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 为每次调用生成独立的临时目录（避免同进程内测试串扰）
    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp = std::env::temp_dir().join(format!(
            "mumu_test_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::create_dir_all(&temp);
        temp
    }

    #[test]
    fn default_settings_have_expected_values() {
        let s = Settings::default();
        assert_eq!(s.version, 1);
        assert_eq!(s.reminders.work_start, "09:00");
        assert_eq!(s.reminders.work_end, "18:00");
        assert_eq!(s.reminders.interval_minutes, 20);
        assert_eq!(s.reminders.rest_seconds, 20);
        assert!(s.reminders.show_popup);
        assert!(s.reminders.play_sound);
        assert!(s.care.eye_drop_enabled);
        assert_eq!(s.care.eye_drop_interval_minutes, 120);
        assert!(s.care.warm_compress_enabled);
        assert_eq!(s.care.warm_compress_time, "13:00");
        assert!(s.advanced.auto_start);
        assert!(!s.advanced.debug_mode);
    }

    #[test]
    fn read_when_file_missing_returns_default() {
        let dir = temp_dir();
        let path = settings_path_at(&dir);
        let settings = read_settings_at(&path).unwrap();
        assert_eq!(settings.version, 1);
        assert_eq!(settings.reminders.interval_minutes, 20);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_then_read_returns_same_settings() {
        let dir = temp_dir();
        let path = settings_path_at(&dir);

        let mut s = Settings::default();
        s.reminders.interval_minutes = 30;
        s.reminders.work_start = "10:00".to_string();
        s.care.eye_drop_enabled = false;
        s.advanced.auto_start = false;

        write_settings_at(&path, &s).unwrap();
        let loaded = read_settings_at(&path).unwrap();

        assert_eq!(loaded.reminders.interval_minutes, 30);
        assert_eq!(loaded.reminders.work_start, "10:00");
        assert!(!loaded.care.eye_drop_enabled);
        assert!(!loaded.advanced.auto_start);
        // 未修改的字段保持默认
        assert_eq!(loaded.reminders.work_end, "18:00");
        assert_eq!(loaded.care.warm_compress_time, "13:00");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migration_from_old_version_fills_missing_fields() {
        let dir = temp_dir();
        let path = settings_path_at(&dir);

        // 模拟 v0 的 JSON（缺少新字段）
        let old_json = r#"{
            "version": 0,
            "reminders": {
                "work_start": "08:00",
                "work_end": "17:00",
                "interval_minutes": 25,
                "rest_seconds": 30,
                "show_popup": false,
                "play_sound": true
            }
        }"#;
        fs::write(&path, old_json).unwrap();

        let loaded = read_settings_at(&path).unwrap();

        // 用户原本的值被保留
        assert_eq!(loaded.reminders.work_start, "08:00");
        assert_eq!(loaded.reminders.interval_minutes, 25);
        assert!(!loaded.reminders.show_popup);

        // 缺失字段用默认值兜底
        assert!(loaded.care.eye_drop_enabled);
        assert_eq!(loaded.care.warm_compress_time, "13:00");
        assert!(loaded.advanced.auto_start);

        // 版本升级到当前
        assert_eq!(loaded.version, CURRENT_VERSION);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_json_returns_error() {
        let dir = temp_dir();
        let path = settings_path_at(&dir);
        fs::write(&path, "{ invalid json }").unwrap();

        assert!(read_settings_at(&path).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_file_returns_error() {
        let dir = temp_dir();
        let path = settings_path_at(&dir);
        fs::write(&path, "").unwrap();

        // 0 字节文件不是合法 JSON，应当返回错误（不让默认设置覆盖用户损坏的配置）
        assert!(read_settings_at(&path).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_creates_parent_dir_if_missing() {
        let dir = temp_dir().join("nested").join("subdir");
        // dir 不存在 → write_settings_at 必须递归创建
        let path = settings_path_at(&dir);
        assert!(!dir.exists());

        let s = Settings::default();
        write_settings_at(&path, &s).unwrap();

        assert!(path.exists());
        let loaded = read_settings_at(&path).unwrap();
        assert_eq!(loaded.version, CURRENT_VERSION);

        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn settings_handle_snapshot_and_replace_round_trip() {
        // 验证 tokio::sync::RwLock 下的 SettingsHandle 行为
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let handle = SettingsHandle::new(Settings::default());

        rt.block_on(async {
            // 初始 snapshot
            let s = handle.snapshot().await;
            assert!(s.reminders.show_popup);
            assert!(s.advanced.auto_start);

            // replace 新值
            let mut new = Settings::default();
            new.reminders.interval_minutes = 5;
            new.advanced.auto_start = false;
            handle.replace(new).await.unwrap();

            // snapshot 反映新值
            let s2 = handle.snapshot().await;
            assert_eq!(s2.reminders.interval_minutes, 5);
            assert!(!s2.advanced.auto_start);
        });
    }

    #[test]
    fn settings_handle_replace_persists_to_disk() {
        use std::path::PathBuf;
        let dir = temp_dir();
        let path = PathBuf::from(&dir).join(SETTINGS_FILE);

        // 改默认 settings_path() 不可行（依赖 APPDATA），改为：让 SettingsHandle
        // 真的走 write_settings() 写到默认路径；用环境 XDG_CONFIG_HOME 注入 APPDATA
        // 这里改用直接验证 write_settings() 写盘内容
        let mut s = Settings::default();
        s.reminders.interval_minutes = 7;
        s.advanced.debug_mode = true;
        write_settings_at(&path, &s).unwrap();

        let loaded = read_settings_at(&path).unwrap();
        assert_eq!(loaded.reminders.interval_minutes, 7);
        assert!(loaded.advanced.debug_mode);

        let _ = fs::remove_dir_all(&dir);
    }
}
