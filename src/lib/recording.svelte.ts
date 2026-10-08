// State rekaman bersama untuk jendela main (tombol Mulai/Stop, popup consent).
import { api, events } from "./api";
import { id } from "./i18n/id";
import { showToast } from "./toast.svelte";
import type { AppError, RecordingState } from "./types";

export const rec = $state({
  state: {
    status: "idle",
    meetingId: null,
    elapsedMs: 0,
    micMuted: false,
    micAlive: true,
    systemAlive: true,
  } as RecordingState,
  busy: false,
  consentOpen: false,
  /** Aplikasi meeting yang terdeteksi (zoom/teams/browser) untuk kolom source_app. */
  sourceApp: null as string | null,
});

let initialized = false;

/** Sinkron dengan backend; dipanggil sekali dari layout jendela main. */
export async function initRecording() {
  if (initialized) return;
  initialized = true;
  await events.recordingState((s) => (rec.state = s));
  await events.trayStartRecording(() => openConsent());
  try {
    rec.state = await api.getRecordingState();
  } catch {
    /* state awal tetap idle */
  }
}

export function openConsent(sourceApp: string | null = null) {
  if (rec.state.status !== "idle") return;
  rec.sourceApp = sourceApp;
  rec.consentOpen = true;
}

/** Dipanggil popup consent setelah checkbox dicentang. */
export async function startRecording() {
  rec.busy = true;
  try {
    await api.startRecording(rec.sourceApp ?? undefined);
    rec.consentOpen = false;
    rec.state = await api.getRecordingState();
  } catch (e) {
    showToast((e as AppError).message, "error", 6000);
  } finally {
    rec.busy = false;
  }
}

export async function stopRecording() {
  rec.busy = true;
  try {
    const res = await api.stopRecording();
    showToast(res.meetingId ? id.toast.saved : id.toast.tooShort, res.meetingId ? "success" : "info");
  } catch (e) {
    showToast((e as AppError).message, "error");
  } finally {
    rec.busy = false;
  }
}
