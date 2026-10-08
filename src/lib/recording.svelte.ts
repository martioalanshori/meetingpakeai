// State rekaman bersama untuk jendela main (tombol Mulai/Stop, banner "Meeting terdeteksi").
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
  /** Tawaran dari deteksi meeting: jenis aplikasi (zoom/teams/browser), null jika tidak ada. */
  offer: null as string | null,
});

let initialized = false;

/** Sinkron dengan backend; dipanggil sekali dari layout jendela main. */
export async function initRecording() {
  if (initialized) return;
  initialized = true;
  await events.recordingState((s) => {
    rec.state = s;
    if (s.status !== "idle") rec.offer = null;
  });
  try {
    rec.state = await api.getRecordingState();
  } catch {
    /* state awal tetap idle */
  }
}

/** Mulai rekam langsung (consent diminta pengguna di luar aplikasi). */
export async function startRecording(sourceApp: string | null = null) {
  rec.busy = true;
  rec.offer = null;
  try {
    await api.startRecording(sourceApp ?? undefined);
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
