//! Uji end-to-end tanpa UI: rekam → antrean (VAD, STT Groq, merge, dedup, ringkasan LLM) di folder data sementara.
//!
//!   cargo run --example e2e -- <detik_rekam> <folder_data> [auto|id]
//!
//! Selama merekam, putar suara (mis. TTS) lewat speaker agar tertangkap loopback.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use meeting_pake_ai_lib::config::providers::ProvidersConfig;
use meeting_pake_ai_lib::config::settings::{self, SettingsPatch, SttLanguage};
use meeting_pake_ai_lib::db::repo_meetings::{self, MeetingStatus};
use meeting_pake_ai_lib::db::{repo_segments, repo_summary, Db};
use meeting_pake_ai_lib::events::EventSink;
use meeting_pake_ai_lib::groq;
use meeting_pake_ai_lib::queue::worker::Worker;
use meeting_pake_ai_lib::recording::{RecordingService, StopReason};
use tokio::sync::Notify;

struct PrintSink;

impl EventSink for PrintSink {
    fn emit_json(&self, event: &str, payload: serde_json::Value) {
        if event == "job://progress" {
            println!("  {payload}");
        }
    }
    fn notify(&self, title: &str, body: &str) {
        println!("notifikasi: {title} — {body}");
    }
    fn recording_changed(&self, _recording: bool) {}
}

fn main() {
    let mut a = std::env::args().skip(1);
    let secs: u64 = a.next().and_then(|s| s.parse().ok()).unwrap_or(30);
    let dir = PathBuf::from(a.next().expect("folder_data"));
    let lang = if a.next().as_deref() == Some("id") { SttLanguage::Id } else { SttLanguage::Auto };

    let db = Arc::new(Db::open(&dir.join("db/app.sqlite")).unwrap());
    settings::apply_patch(
        &db.conn(),
        SettingsPatch { stt_language: Some(lang), audio_retention: Some(meeting_pake_ai_lib::config::settings::AudioRetention::Forever), ..Default::default() },
    )
    .unwrap();
    let providers = ProvidersConfig::default();
    let wake = Arc::new(Notify::new());
    let sink: Arc<dyn EventSink> = Arc::new(PrintSink);

    let w = wake.clone();
    let rec = Arc::new(RecordingService::new(
        dir.clone(),
        db.clone(),
        sink.clone(),
        providers.recording.clone(),
        Box::new(move || w.notify_one()),
    ));
    let id = rec.start(None).unwrap_or_else(|e| panic!("start: {}", e.message));
    println!("merekam {secs} dtk (meeting {id}) …");
    std::thread::sleep(Duration::from_secs(secs));
    let id = rec.stop(StopReason::Manual).unwrap().expect("rekaman terlalu pendek");
    println!("rekaman selesai, mulai antrean");

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let worker = Arc::new(Worker::new(dir.clone(), db.clone(), providers, groq::build_client(), sink, wake.clone()));
    rt.spawn(worker.clone().run());
    let started = std::time::Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let m = repo_meetings::get(&db.conn(), &id).unwrap();
        if matches!(m.status, MeetingStatus::Done | MeetingStatus::Failed) {
            println!("status akhir: {:?} ({:?} {:?}) dalam {} dtk", m.status, m.error_code, m.error_message, started.elapsed().as_secs());
            println!("judul: {}", m.title);
            break;
        }
        if started.elapsed() > Duration::from_secs(600) {
            println!("timeout, status {:?}", m.status);
            break;
        }
    }
    let conn = db.conn();
    println!("== transkrip ==");
    for s in repo_segments::list_visible(&conn, &id).unwrap() {
        println!("[{:>6} ms] {}: {}", s.start_ms, s.channel, s.text);
    }
    println!("== ringkasan ==");
    println!("{:#?}", repo_summary::get(&conn, &id).unwrap());
    for a in repo_summary::action_items(&conn, &id).unwrap() {
        println!("- [ ] {} | PJ: {:?} | Tenggat: {:?}", a.task, a.assignee, a.due);
    }
}
