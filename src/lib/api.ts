// Satu-satunya pintu UI ke Rust. Komponen tidak boleh memanggil invoke()/listen() langsung.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppError,
  AudioTestResult,
  AutoStopWarningPayload,
  JobProgressPayload,
  LevelPayload,
  MeetingDetail,
  MeetingListItem,
  MeetingUpdatedPayload,
  MicPermission,
  OnboardingStatus,
  SearchHit,
  TaskItem,
  PendingConsent,
  QuotaToday,
  TemplateOption,
  RecordingState,
  RecordingWarningPayload,
  Settings,
  SummaryEdit,
  TestApiKeyResult,
  TranscriptSegment,
  UpdateInfo,
} from "./types";

/** Error dari command selalu berbentuk AppError; error lain dibungkus jadi INTERNAL. */
export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "code" in e && "message" in e) return e as AppError;
  return { code: "INTERNAL", message: "Terjadi kesalahan. Detail tersimpan di log." };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toAppError(e);
  }
}

export const api = {
  // Onboarding & API key
  getOnboardingStatus: () => call<OnboardingStatus>("get_onboarding_status"),
  saveApiKey: (key: string) => call<void>("save_api_key", { key }),
  testApiKey: (key?: string) => call<TestApiKeyResult>("test_api_key", { key }),
  deleteApiKey: () => call<void>("delete_api_key"),
  detectApiKeyInClipboard: () => call<string | null>("detect_api_key_in_clipboard"),
  checkMicPermission: () => call<MicPermission>("check_mic_permission"),
  openMicSettings: () => call<void>("open_mic_settings"),
  runAudioTest: () => call<AudioTestResult>("run_audio_test"),
  completeOnboarding: () => call<void>("complete_onboarding"),
  openLogFolder: () => call<void>("open_log_folder"),

  // Rekaman
  startRecording: (sourceApp?: string) =>
    call<{ meetingId: string }>("start_recording", { consentConfirmed: true, sourceApp }),
  pauseRecording: () => call<RecordingState>("pause_recording"),
  resumeRecording: () => call<RecordingState>("resume_recording"),
  setMicMuted: (muted: boolean) => call<RecordingState>("set_mic_muted", { muted }),
  stopRecording: () => call<{ meetingId: string | null }>("stop_recording"),
  getRecordingState: () => call<RecordingState>("get_recording_state"),
  takePendingConsent: () => call<PendingConsent>("take_pending_consent"),
  takePendingMeeting: () => call<string | null>("take_pending_meeting"),
  respondAutoStop: (continueRecording: boolean) =>
    call<void>("respond_auto_stop", { continueRecording }),

  // Meeting
  searchMeetings: (query: string) => call<SearchHit[]>("search_meetings", { query }),
  listActionItems: () => call<TaskItem[]>("list_action_items"),
  listMeetings: (limit: number, offset: number) =>
    call<MeetingListItem[]>("list_meetings", { limit, offset }),
  getMeeting: (id: string) => call<MeetingDetail>("get_meeting", { id }),
  getTranscript: (id: string) => call<TranscriptSegment[]>("get_transcript", { id }),
  renameMeeting: (id: string, title: string) => call<void>("rename_meeting", { id, title }),
  setActionItemDone: (id: number, done: boolean) =>
    call<void>("set_action_item_done", { id, done }),
  deleteMeeting: (id: string) => call<void>("delete_meeting", { id }),
  preparePlayback: (id: string) => call<string>("prepare_playback", { id }),
  saveExport: (fileName: string, contents: string) => call<boolean>("save_export", { fileName, contents }),
  updateSummary: (id: string, edit: SummaryEdit) => call<void>("update_summary", { id, edit }),
  setSpeakerName: (id: string, name: string) => call<void>("set_speaker_name", { id, name }),
  retryJob: (id: string) => call<void>("retry_job", { id }),
  regenerateSummary: (id: string) => call<void>("regenerate_summary", { id }),
  listSummaryTemplates: () => call<TemplateOption[]>("list_summary_templates"),
  setSummaryTemplate: (id: string, template: string | null) =>
    call<void>("set_summary_template", { id, template }),
  retranscribe: (id: string) => call<void>("retranscribe", { id }),
  resolveInterrupted: (id: string, action: "process" | "discard") =>
    call<void>("resolve_interrupted", { id, action }),

  // Pengaturan
  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (patch: Partial<Settings>) => call<Settings>("update_settings", { patch }),
  checkUpdate: () => call<UpdateInfo | null>("check_update"),
  getQuotaToday: () => call<QuotaToday>("get_quota_today"),
  saveProblemReport: () => call<boolean>("save_problem_report"),
  installUpdate: () => call<void>("install_update"),
};

function on<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => cb(e.payload));
}

export const events = {
  recordingState: (cb: (p: RecordingState) => void) => on("recording://state", cb),
  recordingLevel: (cb: (p: LevelPayload) => void) => on("recording://level", cb),
  autoStopWarning: (cb: (p: AutoStopWarningPayload) => void) =>
    on("recording://auto-stop-warning", cb),
  recordingWarning: (cb: (p: RecordingWarningPayload) => void) => on("recording://warning", cb),
  jobProgress: (cb: (p: JobProgressPayload) => void) => on("job://progress", cb),
  meetingUpdated: (cb: (p: MeetingUpdatedPayload) => void) => on("meeting://updated", cb),
  /** Menu tray "Mulai rekam" → buka popup consent di jendela main. */
  trayStartRecording: (cb: () => void) => on<null>("tray://start-recording", () => cb()),
  /** Ada tawaran rekam / meeting selesai untuk jendela main yang sedang fokus. */
  appPending: (cb: () => void) => on<null>("app://pending", () => cb()),
};
