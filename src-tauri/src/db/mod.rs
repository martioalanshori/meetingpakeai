//! SQLite (rusqlite bundled). Satu koneksi dibagi lewat Mutex; worker & command bergantian memakainya.

pub mod repo_chunks;
pub mod repo_meetings;
pub mod repo_parts;
pub mod repo_segments;
pub mod repo_settings;
pub mod repo_summary;
pub mod repo_usage;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// Migrasi berurutan; indeks + 1 = nilai `PRAGMA user_version` setelah migrasi dijalankan.
const MIGRATIONS: &[&str] = &[include_str!("migrations/001_init.sql"), include_str!("migrations/002_edits.sql")];

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[allow(dead_code)] // untuk uji manual / debugging
    pub fn open_in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> AppResult<Self> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA secure_delete = ON;",
        )?;
        migrate(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// Akses koneksi. Jangan menahan guard melewati `.await`.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // Mutex hanya "poisoned" jika thread lain panic saat memegangnya; datanya tetap valid untuk SQLite.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn migrate(conn: &mut Connection) -> AppResult<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let current = usize::try_from(current).map_err(AppError::internal)?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
        tracing::info!("db migrated to version {}", i + 1);
    }
    Ok(())
}

/// Waktu sekarang dalam epoch milidetik UTC (semua kolom waktu di DB memakai ini).
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
