//! Data halaman Beranda (docs/rencana/PLAN-beranda.md, langkah B2): satu panggilan untuk semua isi Beranda.

use serde::Serialize;
use tauri::State;

use crate::config::settings;
use crate::db::repo_meetings::{self, MeetingStatus};
use crate::db::repo_summary::{self, TaskView};
use crate::error::AppResult;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeMeeting {
    pub id: String,
    pub title: String,
    pub started_at: i64,
    /// Satu kalimat inti: keputusan pertama, poin intisari pertama, atau kalimat pertama ringkasan.
    pub line: Option<String>,
    /// `decision` | `summary` — awalan yang ditampilkan ("Diputuskan:" / "Ringkasan:").
    pub line_kind: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeProcessing {
    pub id: String,
    pub title: String,
    pub status: MeetingStatus,
    pub progress_done: i64,
    pub progress_total: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeOverview {
    /// Meeting yang sedang diproses / menunggu antrean (yang pertama).
    pub processing: Option<HomeProcessing>,
    /// Jumlah meeting gagal / rekaman terputus + id yang pertama (untuk tautan).
    pub attention_count: i64,
    pub attention_id: Option<String>,
    pub queue_paused: bool,
    pub recent: Vec<HomeMeeting>,
    /// Maks 3: lewat tenggat → hari ini → tenggat terdekat → tugas terbaru.
    pub urgent_tasks: Vec<TaskView>,
    pub open_tasks: i64,
    pub has_meetings: bool,
    pub meeting_detection: bool,
    pub autostart: bool,
    /// 2–3 contoh pertanyaan dari meeting pengguna sendiri (evaluasi Beranda P2).
    pub suggestions: Vec<String>,
}

fn first_sentence(text: &str) -> String {
    let t = text.trim();
    let end = t.char_indices().find(|(_, c)| matches!(c, '.' | '!' | '?')).map_or(t.len(), |(i, c)| i + c.len_utf8());
    let s: String = t[..end].chars().take(160).collect();
    s.trim().to_string()
}

#[tauri::command]
pub async fn home_overview(state: State<'_, AppState>) -> AppResult<HomeOverview> {
    let conn = state.db.conn();
    let s = settings::load(&conn)?;

    let recent_rows: Vec<repo_meetings::MeetingRow> = repo_meetings::list(&conn, 40, 0)?;
    let has_meetings = !recent_rows.is_empty();

    let processing = recent_rows
        .iter()
        .find(|m| m.status.is_running_step() || matches!(m.status, MeetingStatus::Queued))
        .map(|m| HomeProcessing {
            id: m.id.clone(),
            title: m.title.clone(),
            status: m.status,
            progress_done: m.progress_done,
            progress_total: m.progress_total,
        });
    let attention: Vec<&repo_meetings::MeetingRow> = recent_rows
        .iter()
        .filter(|m| matches!(m.status, MeetingStatus::Failed | MeetingStatus::Interrupted))
        .collect();

    let mut recent = Vec::new();
    let mut topics: Vec<String> = Vec::new();
    for m in recent_rows.iter().filter(|m| m.status == MeetingStatus::Done).take(3) {
        let sum = repo_summary::get(&conn, &m.id)?;
        let (line, kind) = match &sum {
            Some(s) if !s.decisions.is_empty() => (Some(s.decisions[0].clone()), Some("decision")),
            Some(s) => (
                s.key_points.first().cloned().or_else(|| s.summary.as_deref().map(first_sentence)).filter(|l| !l.is_empty()),
                Some("summary"),
            ),
            None => (None, None),
        };
        if let Some(s) = &sum {
            topics.extend(s.topics.iter().take(2).cloned());
        }
        recent.push(HomeMeeting {
            id: m.id.clone(),
            title: m.title.clone(),
            started_at: m.started_at,
            line_kind: line.as_ref().and(kind).map(str::to_string),
            line,
        });
    }

    let open: Vec<TaskView> = repo_summary::all_action_items(&conn)?.into_iter().filter(|t| !t.done).collect();
    let open_tasks = open.len() as i64;
    let mut dated: Vec<&TaskView> = open.iter().filter(|t| t.due_date.is_some()).collect();
    dated.sort_by(|a, b| a.due_date.cmp(&b.due_date));
    let mut urgent: Vec<TaskView> = dated.into_iter().take(3).cloned().collect();
    // Tidak ada tenggat sama sekali → tugas terbaru (urutan dari repo: meeting terbaru dulu).
    if urgent.len() < 3 {
        for t in open.iter().filter(|t| t.due_date.is_none()) {
            if urgent.len() >= 3 {
                break;
            }
            urgent.push(t.clone());
        }
    }

    // Contoh pertanyaan dari isi meeting pengguna sendiri: keputusan meeting terakhir, satu topik, satu tugas.
    let short = |s: &str, n: usize| -> String {
        let t: String = s.chars().take(n).collect();
        if s.chars().count() > n { format!("{}…", t.trim_end()) } else { t }
    };
    let mut suggestions: Vec<String> = Vec::new();
    if let Some(m) = recent.first() {
        suggestions.push(format!("Apa keputusan di {}?", short(&m.title, 40)));
    }
    if let Some(topic) = topics.first() {
        suggestions.push(format!("Apa saja yang dibahas soal {}?", short(&topic.to_lowercase(), 40)));
    }
    if let Some(task) = urgent.first() {
        suggestions.push(format!("Kenapa perlu {}?", short(&task.task.to_lowercase(), 48)));
    }

    Ok(HomeOverview {
        processing,
        attention_count: attention.len() as i64,
        attention_id: attention.first().map(|m| m.id.clone()),
        queue_paused: state.worker.is_paused(),
        recent,
        urgent_tasks: urgent,
        open_tasks,
        has_meetings,
        meeting_detection: s.meeting_detection,
        autostart: s.autostart,
        suggestions,
    })
}
