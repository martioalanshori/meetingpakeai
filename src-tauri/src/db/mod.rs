//! SQLite (rusqlite bundled). Satu koneksi dibagi lewat Mutex; worker & command bergantian memakainya.

pub mod repo_bookmarks;
pub mod repo_chunks;
pub mod repo_live;
pub mod repo_meetings;
pub mod repo_notes;
pub mod repo_parts;
pub mod repo_search;
pub mod repo_segments;
pub mod repo_settings;
pub mod repo_summary;
pub mod repo_usage;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{AppError, AppResult, ErrorCode};

/// Migrasi berurutan; indeks + 1 = nilai `PRAGMA user_version` setelah migrasi dijalankan.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_init.sql"),
    include_str!("migrations/002_edits.sql"),
    include_str!("migrations/003_fts.sql"),
    include_str!("migrations/004_template.sql"),
    include_str!("migrations/005_drop_template.sql"),
    include_str!("migrations/006_live.sql"),
    include_str!("migrations/007_sources.sql"),
    include_str!("migrations/008_follow_up.sql"),
    include_str!("migrations/009_bookmarks.sql"),
    include_str!("migrations/010_notes.sql"),
    include_str!("migrations/011_summary_extras.sql"),
    include_str!("migrations/012_qa.sql"),
    include_str!("migrations/013_task_due.sql"),
];

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        backup_before_migration(&conn, path)?;
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

/// Backup yang disimpan (yang lebih lama dihapus).
const MAX_BACKUPS: usize = 3;

fn user_version(conn: &Connection) -> AppResult<usize> {
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    usize::try_from(v).map_err(AppError::internal)
}

/// Sebelum migrasi pada DB yang sudah berisi: salin ke `app.sqlite.bak-v{n}` (VACUUM INTO, konsisten
/// walau mode WAL). DB dari versi aplikasi yang lebih baru ditolak agar tidak dirusak build lama.
fn backup_before_migration(conn: &Connection, path: &Path) -> AppResult<()> {
    let current = user_version(conn)?;
    if current > MIGRATIONS.len() {
        return Err(AppError::with_message(
            ErrorCode::Internal,
            "Database dibuat oleh versi Meeting Pake AI yang lebih baru. Pasang versi terbaru aplikasi.",
        ));
    }
    if current == 0 || current == MIGRATIONS.len() {
        return Ok(());
    }
    let Some(dir) = path.parent() else { return Ok(()) };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "app.sqlite".into());
    let backup = dir.join(format!("{name}.bak-v{current}"));
    if !backup.exists() {
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
        tracing::info!("backup database sebelum migrasi: {}", backup.display());
    }
    // Simpan beberapa backup terbaru saja.
    let prefix = format!("{name}.bak-v");
    let mut backups: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    backups.sort();
    while backups.len() > MAX_BACKUPS {
        let (_, old) = backups.remove(0);
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

fn migrate(conn: &mut Connection) -> AppResult<()> {
    let current = user_version(conn)?;
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
