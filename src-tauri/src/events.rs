//! Jembatan ke UI tanpa bergantung pada tipe Tauri (PRD §6.2: `EventSink` di-inject).
//! Implementasi Tauri ada di `bridge.rs`.

use serde::Serialize;

pub const EV_RECORDING_STATE: &str = "recording://state";
pub const EV_RECORDING_LEVEL: &str = "recording://level";
pub const EV_AUTO_STOP_WARNING: &str = "recording://auto-stop-warning";
pub const EV_RECORDING_WARNING: &str = "recording://warning";
pub const EV_JOB_PROGRESS: &str = "job://progress";
pub const EV_MEETING_UPDATED: &str = "meeting://updated";

pub trait EventSink: Send + Sync {
    /// Kirim event ke semua jendela.
    fn emit_json(&self, event: &str, payload: serde_json::Value);
    /// Notifikasi Windows.
    fn notify(&self, title: &str, body: &str);
    /// Rekaman mulai/berhenti: ikon tray, label menu tray, jendela widget.
    fn recording_changed(&self, recording: bool);
    /// Job selesai: notifikasi "Notulen siap" yang membuka detail meeting.
    fn meeting_done(&self, _meeting_id: &str, title: &str) {
        self.notify("Notulen siap", title);
    }
}

pub fn emit<T: Serialize>(sink: &dyn EventSink, event: &str, payload: &T) {
    match serde_json::to_value(payload) {
        Ok(v) => sink.emit_json(event, v),
        Err(e) => tracing::error!("serialize event {event}: {e}"),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingUpdated<'a> {
    pub meeting_id: &'a str,
}
