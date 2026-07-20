// SQLite 数据层
//
// 规格：openspec/changes/add-mumu-eye-care/specs/statistics/spec.md
//
// 实现要点：
// - 库位置：%APPDATA%\沐目\stats.db
// - 表结构：
//   daily_stats   每日聚合（用于主界面显示）
//   pause_records 暂停记录（用于 T06 屏幕使用统计扣除暂停时长）
// - 跨日：写入按本地日期 YYYY-MM-DD 做 key，首次写入新一天自动建行
// - 线程：Connection 包在 Mutex 里（多线程安全，Tauri State 持有）

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Local};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::settings::default_app_dir;

const DB_FILE: &str = "stats.db";

#[derive(Error, Debug)]
pub enum DbError {
    #[error("无法创建数据库目录 {0}: {1}")]
    CreateDirFailed(PathBuf, std::io::Error),
    #[error("打开数据库失败 {0}: {1}")]
    OpenFailed(PathBuf, rusqlite::Error),
    #[error("SQL 执行失败: {0}")]
    SqlFailed(#[from] rusqlite::Error),
}

/// 今日统计快照（主界面展示 + 颜色规则判定）
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DailyStats {
    pub date: String,
    pub total_seconds: u32,
    pub rest_count: u32,
    pub rest_seconds: u32,
}

/// 暂停记录
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PauseRecord {
    pub id: i64,
    pub start_time: String,
    pub end_time: Option<String>,
    pub reason: String,
}

/// 线程安全的数据库句柄（注入 Tauri State）
pub struct DbState {
    pub conn: Mutex<Connection>,
}

impl DbState {
    /// 在默认目录创建数据库 + 初始化表
    pub fn new() -> Result<Self, DbError> {
        let path = db_path();
        Self::new_at(&path)
    }

    /// 在指定路径创建数据库（用于测试）
    pub fn new_at(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| DbError::CreateDirFailed(parent.to_path_buf(), e))?;
            }
        }
        let conn = Connection::open(path).map_err(|e| DbError::OpenFailed(path.to_path_buf(), e))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init()?;
        Ok(db)
    }

    /// 建表（DDL）
    fn init(&self) -> Result<(), DbError> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS daily_stats (
                date          TEXT PRIMARY KEY,
                total_seconds INTEGER NOT NULL DEFAULT 0,
                rest_count    INTEGER NOT NULL DEFAULT 0,
                rest_seconds  INTEGER NOT NULL DEFAULT 0,
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pause_records (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time  TEXT NOT NULL,
                end_time    TEXT,
                reason      TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_pause_records_open
                ON pause_records(end_time) WHERE end_time IS NULL;
            "#,
        )?;
        Ok(())
    }
}

/// 获取默认数据库路径：%APPDATA%\沐目\stats.db
pub fn db_path() -> PathBuf {
    default_app_dir().join(DB_FILE)
}

/// 测试用：注入自定义目录
pub fn db_path_at(dir: &Path) -> PathBuf {
    dir.join(DB_FILE)
}

/// 拿今天日期字符串 YYYY-MM-DD（本地时区）
fn today_string() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// 拿当前时间戳字符串（本地时区 ISO 8601）
fn now_string() -> String {
    let now: DateTime<Local> = Local::now();
    now.format("%Y-%m-%dT%H:%M:%S").to_string()
}

// ============================================================================
// 业务函数（T06 / T07 / T09 会调用）
// ============================================================================

/// 累加当天屏幕使用时长（由 T06 每分钟调用）
pub fn record_screen_on(state: &DbState, seconds: u32) -> Result<(), DbError> {
    let conn = state.conn.lock().expect("db mutex poisoned");
    let date = today_string();
    let now = now_string();

    conn.execute(
        r#"
        INSERT INTO daily_stats (date, total_seconds, rest_count, rest_seconds, created_at, updated_at)
        VALUES (?1, ?2, 0, 0, ?3, ?3)
        ON CONFLICT(date) DO UPDATE SET
            total_seconds = total_seconds + ?2,
            updated_at    = ?3
        "#,
        params![date, seconds, now],
    )?;
    Ok(())
}

/// 累加当天休息次数（由 T07 提醒触发完成时调用）
pub fn increment_rest_count(state: &DbState) -> Result<(), DbError> {
    let conn = state.conn.lock().expect("db mutex poisoned");
    let date = today_string();
    let now = now_string();

    conn.execute(
        r#"
        INSERT INTO daily_stats (date, total_seconds, rest_count, rest_seconds, created_at, updated_at)
        VALUES (?1, 0, 1, 0, ?2, ?2)
        ON CONFLICT(date) DO UPDATE SET
            rest_count = rest_count + 1,
            updated_at = ?2
        "#,
        params![date, now],
    )?;
    Ok(())
}

/// 累加当天实际休息时长（用户完成倒计时时调用）
pub fn add_rest_seconds(state: &DbState, seconds: u32) -> Result<(), DbError> {
    let conn = state.conn.lock().expect("db mutex poisoned");
    let date = today_string();
    let now = now_string();

    conn.execute(
        r#"
        INSERT INTO daily_stats (date, total_seconds, rest_count, rest_seconds, created_at, updated_at)
        VALUES (?1, 0, 0, ?2, ?3, ?3)
        ON CONFLICT(date) DO UPDATE SET
            rest_seconds = rest_seconds + ?2,
            updated_at   = ?3
        "#,
        params![date, seconds, now],
    )?;
    Ok(())
}

/// 拿今日统计（主界面展示用）
pub fn get_today_stats(state: &DbState) -> Result<DailyStats, DbError> {
    let conn = state.conn.lock().expect("db mutex poisoned");
    let date = today_string();

    let row = conn
        .query_row(
            "SELECT total_seconds, rest_count, rest_seconds FROM daily_stats WHERE date = ?1",
            params![date],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as u32,
                    r.get::<_, i64>(1)? as u32,
                    r.get::<_, i64>(2)? as u32,
                ))
            },
        )
        .optional()?;

    Ok(DailyStats {
        date,
        total_seconds: row.map(|r| r.0).unwrap_or(0),
        rest_count: row.map(|r| r.1).unwrap_or(0),
        rest_seconds: row.map(|r| r.2).unwrap_or(0),
    })
}

/// 开始一次暂停（reason: "manual_30min" / "locked" / "fullscreen"）
/// 返回新记录的 id 用于后续 end_pause
pub fn start_pause(state: &DbState, reason: &str) -> Result<i64, DbError> {
    let conn = state.conn.lock().expect("db mutex poisoned");
    let now = now_string();

    conn.execute(
        "INSERT INTO pause_records (start_time, reason) VALUES (?1, ?2)",
        params![now, reason],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 结束当前最早一条未结束的暂停
pub fn end_pause(state: &DbState) -> Result<(), DbError> {
    let conn = state.conn.lock().expect("db mutex poisoned");
    let now = now_string();

    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM pause_records WHERE end_time IS NULL ORDER BY id ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;

    if let Some(id) = id {
        conn.execute(
            "UPDATE pause_records SET end_time = ?1 WHERE id = ?2",
            params![now, id],
        )?;
    }
    Ok(())
}

/// 计算当天累计暂停秒数（用于屏幕使用统计扣除暂停）
pub fn get_today_paused_seconds(state: &DbState) -> Result<u32, DbError> {
    let conn = state.conn.lock().expect("db mutex poisoned");
    let date_prefix = today_string();

    // julianday 差值是 Real（浮点），必须用 f64 读；最后取整 + 钳到 >=0
    let total: f64 = conn
        .query_row(
            r#"
            SELECT COALESCE(SUM(
                (julianday(end_time) - julianday(start_time)) * 86400
            ), 0.0)
            FROM pause_records
            WHERE substr(start_time, 1, 10) = ?1
              AND end_time IS NOT NULL
            "#,
            params![date_prefix],
            |r| r.get(0),
        )?;

    Ok(total.max(0.0) as u32)
}

// get_today_stats_cmd 在 lib.rs 注册（避免重复定义）

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 每次调用一个独立临时目录（避免测试串扰）
    fn fresh_db() -> DbState {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("mumu_db_test_{}_{}", std::process::id(), id));
        let _ = fs::create_dir_all(&dir);
        DbState::new_at(&dir.join(DB_FILE)).expect("create test db")
    }

    #[test]
    fn init_creates_tables() {
        let db = fresh_db();
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('daily_stats','pause_records')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn record_screen_on_accumulates() {
        let db = fresh_db();
        record_screen_on(&db, 30).unwrap();
        record_screen_on(&db, 30).unwrap();
        record_screen_on(&db, 60).unwrap();

        let stats = get_today_stats(&db).unwrap();
        assert_eq!(stats.total_seconds, 120, "30+30+60=120");
    }

    #[test]
    fn increment_rest_count_only_counts() {
        let db = fresh_db();
        increment_rest_count(&db).unwrap();
        increment_rest_count(&db).unwrap();
        increment_rest_count(&db).unwrap();

        let stats = get_today_stats(&db).unwrap();
        assert_eq!(stats.rest_count, 3);
        assert_eq!(stats.rest_seconds, 0);
    }

    #[test]
    fn add_rest_seconds_accumulates_independently() {
        let db = fresh_db();
        add_rest_seconds(&db, 20).unwrap();
        add_rest_seconds(&db, 20).unwrap();
        increment_rest_count(&db).unwrap();

        let stats = get_today_stats(&db).unwrap();
        assert_eq!(stats.rest_count, 1);
        assert_eq!(stats.rest_seconds, 40);
        assert_eq!(stats.total_seconds, 0);
    }

    #[test]
    fn pause_lifecycle_tracks_duration() {
        let db = fresh_db();

        let id1 = start_pause(&db, "locked").unwrap();
        assert!(id1 > 0);
        assert_eq!(get_today_paused_seconds(&db).unwrap(), 0);

        end_pause(&db).unwrap();

        let total = get_today_paused_seconds(&db).unwrap();
        assert!(total < 5, "极短测试间隔应接近 0 秒");

        start_pause(&db, "manual_30min").unwrap();
        end_pause(&db).unwrap();
        assert_eq!(get_today_paused_seconds(&db).unwrap(), total);
    }

    #[test]
    fn empty_db_returns_zero_stats() {
        let db = fresh_db();
        let stats = get_today_stats(&db).unwrap();
        assert_eq!(stats.total_seconds, 0);
        assert_eq!(stats.rest_count, 0);
        assert_eq!(stats.rest_seconds, 0);
        assert!(!stats.date.is_empty());
    }

    #[test]
    fn concurrent_writes_succeed_without_corruption() {
        use std::sync::Arc;
        use std::thread;

        let db = Arc::new(fresh_db());
        let mut handles = vec![];

        for _ in 0..10 {
            let db = Arc::clone(&db);
            handles.push(thread::spawn(move || {
                for _ in 0..20 {
                    record_screen_on(&db, 1).unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let stats = get_today_stats(&db).unwrap();
        assert_eq!(stats.total_seconds, 200, "10×20×1 = 200，Mutex 守住");
    }

    #[test]
    fn record_screen_on_upserts_same_day() {
        let db = fresh_db();
        record_screen_on(&db, 60).unwrap();
        record_screen_on(&db, 90).unwrap();
        record_screen_on(&db, 30).unwrap();

        // 三次写应该是 UPSERT 累加而不是覆盖
        let stats = get_today_stats(&db).unwrap();
        assert_eq!(stats.total_seconds, 180);
    }

    #[test]
    fn pause_can_be_resumed_and_paused_again() {
        let db = fresh_db();

        start_pause(&db, "locked").unwrap();
        end_pause(&db).unwrap();
        let first_total = get_today_paused_seconds(&db).unwrap();

        // 第二次 pause（验证 end_pause 后 start_pause 又能写新行）
        start_pause(&db, "manual_30min").unwrap();
        end_pause(&db).unwrap();

        let second_total = get_today_paused_seconds(&db).unwrap();
        // 两次累计暂停时间应 >= 第一次
        assert!(second_total >= first_total);
    }

    #[test]
    fn end_pause_without_active_start_is_noop() {
        let db = fresh_db();

        // 没有 start_pause 就调 end_pause → 不应 panic，total 仍为 0
        end_pause(&db).unwrap();
        let total = get_today_paused_seconds(&db).unwrap();
        assert_eq!(total, 0);
    }

    #[test]
    fn stats_read_returns_today_date_string() {
        let db = fresh_db();
        let stats = get_today_stats(&db).unwrap();
        // YYYY-MM-DD 格式，长度 10
        assert_eq!(stats.date.len(), 10);
        assert_eq!(stats.date.chars().filter(|c| *c == '-').count(), 2);
    }
}
