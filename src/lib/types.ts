// Cermin PRD §12. Nama field harus persis sama dengan serde (camelCase) di Rust.

export type ErrorCode =
  | "NO_API_KEY"
  | "INVALID_API_KEY"
  | "NETWORK"
  | "RATE_LIMITED"
  | "QUOTA_EXHAUSTED"
  | "MIC_PERMISSION_DENIED"
  | "NO_INPUT_DEVICE"
  | "NO_OUTPUT_DEVICE"
  | "ALREADY_RECORDING"
  | "NOT_RECORDING"
  | "DISK_FULL"
  | "NOT_FOUND"
  | "INVALID_STATE"
  | "AUDIO_NOT_AVAILABLE"
  | "LLM_INVALID_OUTPUT"
  | "INTERNAL";

export type AppError = { code: ErrorCode; message: string };

export type MeetingStatus =
  | "recording"
  | "interrupted"
  | "queued"
  | "preprocessing"
  | "transcribing"
  | "merging"
  | "summarizing"
  | "done"
  | "waiting_quota"
  | "waiting_network"
  | "failed";

export type Channel = "mic" | "system";

export type MicPermission = "allowed" | "denied" | "unknown";

export type RecordingState = {
  status: "idle" | "recording" | "paused";
  meetingId: string | null;
  elapsedMs: number;
  micMuted: boolean;
  micAlive: boolean;
  systemAlive: boolean;
};

export type MeetingListItem = {
  id: string;
  title: string;
  startedAt: number;
  durationMs: number;
  status: MeetingStatus;
  progressDone: number;
  progressTotal: number;
  errorMessage: string | null;
  /** Tambahan dari backend: kode error untuk status failed. */
  errorCode: ErrorCode | null;
};

export type ActionItem = {
  id: number;
  task: string;
  assignee: string | null;
  due: string | null;
  done: boolean;
  /** Ms dari awal meeting tempat tugas dibahas; null = tidak diketahui. */
  sourceMs: number | null;
};

export type MeetingSummary = {
  status: "ok" | "empty";
  summary: string | null;
  decisions: string[];
  /** Sejajar `decisions`. */
  decisionSources: (number | null)[];
  topics: string[];
  /** Sudah diubah pengguna. */
  edited: boolean;
};

export type SummaryEdit = {
  summary: string;
  decisions: string[];
  topics: string[];
  actionItems: { task: string; assignee: string | null; due: string | null; done: boolean }[];
};

export type MeetingDetail = MeetingListItem & {
  endedAt: number | null;
  language: "id" | "auto";
  failedStep: string | null;
  audioDeleted: boolean;
  labels: { mic: string; system: string };
  summary: MeetingSummary | null;
  actionItems: ActionItem[];
};

export type TranscriptSegment = {
  id: number;
  channel: Channel;
  startMs: number;
  endMs: number;
  text: string;
};

export type AudioRetention = "after_transcript" | "days7" | "forever";

export type Settings = {
  userDisplayName: string;
  sttLanguage: "id" | "auto";
  /** Retensi audio: hapus setelah transkrip / simpan 7 hari / selamanya. */
  audioRetention: AudioRetention;
  minimizeToTray: boolean;
  /** Shortcut global Mulai/Stop rekam, mis. "Ctrl+Alt+R"; "" = mati. */
  globalShortcut: string;
  autostart: boolean;
  /** Tawarkan rekam saat Zoom/Teams/browser memakai mic; tawarkan Stop saat selesai. */
  meetingDetection: boolean;
  /** Nama & istilah, satu per baris (maks 800 karakter). */
  sttGlossary: string;
};

export type UpdateInfo = { version: string; notes: string | null };

export type AudioTestResult = {
  micOk: boolean;
  micPeakDbfs: number;
  systemOk: boolean;
  systemPeakDbfs: number;
};

export type OnboardingStatus = {
  completed: boolean;
  apiKeySet: boolean;
  micPermission: MicPermission;
  /** Antrean dijeda karena API key tidak valid (status worker). */
  queuePaused: boolean;
};

export type SearchHit = {
  meetingId: string;
  title: string;
  startedAt: number;
  kind: "title" | "summary" | "decision" | "topic" | "action" | "transcript";
  /** Kata yang cocok diapit "[" dan "]". */
  snippet: string;
  startMs: number | null;
};

export type TaskItem = {
  id: number;
  meetingId: string;
  meetingTitle: string;
  startedAt: number;
  task: string;
  assignee: string | null;
  due: string | null;
  done: boolean;
};

export type AiRole = "stt" | "llm";

export type AiPreset = {
  id: string;
  name: string;
  baseUrl: string;
  /** null = penyedia tidak mendukung peran ini. */
  sttModel: string | null;
  llmModel: string | null;
  keyUrl: string | null;
  keyPrefix: string | null;
  keyRequired: boolean;
};

export type Endpoint = { provider: string; baseUrl: string; model: string };

export type AiConfig = {
  stt: Endpoint & { keySet: boolean };
  llm: Endpoint & { keySet: boolean };
  presets: AiPreset[];
  keysSet: string[];
};

export type SaveAiResult = { verified: boolean; modelMissing: boolean };

export type DetectedKey = { provider: string; key: string };

// Payload event (§12.4)
export type LevelPayload = { micDbfs: number; systemDbfs: number };
export type AutoStopWarningPayload = { reason: "silence" | "meeting_ended"; secondsLeft: number; silenceMin?: number };
export type RecordingWarningPayload = {
  code: "device_lost" | "system_silent" | "system_ok" | "write_failed" | "limit_soon";
  channel: Channel;
  minutes?: number;
};
export type JobProgressPayload = {
  meetingId: string;
  status: MeetingStatus;
  progressDone: number;
  progressTotal: number;
};
export type MeetingUpdatedPayload = { meetingId: string };
