//! Alat uji manual langkah 6: RecordingService (meeting di DB + part) di folder data sementara.
//!
//!   cargo run --example record_service -- <detik> <folder_data>

use std::path::PathBuf;
use std::sync::Arc;

use meeting_pake_ai_lib::config::providers::ProvidersConfig;
use meeting_pake_ai_lib::db::{repo_meetings, repo_parts, Db};
use meeting_pake_ai_lib::events::EventSink;
use meeting_pake_ai_lib::recording::{RecordingService, StopReason};

struct PrintSink;

impl EventSink for PrintSink {
    fn emit_json(&self, event: &str, payload: serde_json::Value) {
        if event != "recording://level" {
            println!("event {event}: {payload}");
        }
    }
    fn notify(&self, title: &str, body: &str) {
        println!("notifikasi: {title} — {body}");
    }
    fn recording_changed(&self, recording: bool) {
        println!("recording_changed: {recording}");
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let secs: u64 = a.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    let dir = PathBuf::from(a.next().expect("argumen folder_data"));
    let db = Arc::new(Db::open(&dir.join("db/app.sqlite")).unwrap());
    let svc = Arc::new(RecordingService::new(
        dir.clone(),
        db.clone(),
        Arc::new(PrintSink),
        ProvidersConfig::default().recording,
        Box::new(|| println!("on_queued dipanggil")),
    ));
    let id = svc.start(None).unwrap_or_else(|e| panic!("start gagal: {}", e.message));
    println!("start: meeting {id}");
    std::thread::sleep(std::time::Duration::from_secs(secs));
    let res = svc.stop(StopReason::Manual).unwrap();
    println!("stop: {res:?}");
    if let Some(id) = res {
        let conn = db.conn();
        let m = repo_meetings::get(&conn, &id).unwrap();
        println!("meeting: title={:?} status={:?} duration_ms={}", m.title, m.status, m.duration_ms);
        for p in repo_parts::list(&conn, &id).unwrap() {
            println!("part: {} #{} {} sampel finalized={}", p.channel, p.part_index, p.samples, p.finalized);
        }
    }
}
