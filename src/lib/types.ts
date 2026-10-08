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
  /** Label proyek/klien. */
  tags: string[];
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

/** Data Beranda (`home_overview`). */
export type HomeOverview = {
  processing: { id: string; title: string; status: MeetingStatus; progressDone: number; progressTotal: number } | null;
  attentionCount: number;
  attentionId: string | null;
  queuePaused: boolean;
  recent: { id: string; title: string; startedAt: number; line: string | null; lineKind: "decision" | "summary" | null }[];
  urgentTasks: TaskItem[];
  openTasks: number;
  hasMeetings: boolean;
  meetingDetection: boolean;
  autostart: boolean;
  /** Contoh pertanyaan dari meeting pengguna sendiri. */
  suggestions: string[];
};

export type AskAllResult = {
  answer: string;
  refs: { meetingId: string; title: string; startedAt: number; atMs: number | null }[];
};

export type QaItem = { id: number; question: string; answer: string; sources: number[]; createdAt: number };

export type FollowUp = { subject: string; body: string; lang: "id" | "en" };

export type MeetingSummary = {
  status: "ok" | "empty";
  summary: string | null;
  decisions: string[];
  /** Sejajar `decisions`. */
  decisionSources: (number | null)[];
  /** Intisari 3 poin. */
  keyPoints: string[];
  /** Belum diputuskan / pertanyaan terbuka + sumber waktu (sejajar). */
  openQuestions: string[];
  openQuestionSources: (number | null)[];
  /** Status tugas terbuka dari meeting sebelumnya dalam rangkaian yang sama. */
  followupStatus: { itemId: number; task: string; status: "selesai" | "dibahas" | "belum_disebut"; note: string; done: boolean }[];
  followupFrom: { meetingId: string; title: string } | null;
  /** Draf pesan tindak lanjut; null = belum dibuat. */
  followUp: FollowUp | null;
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
  language: "id" | "auto" | "mixed";
  failedStep: string | null;
  audioDeleted: boolean;
  labels: { mic: string; system: string };
  summary: MeetingSummary | null;
  actionItems: ActionItem[];
  /** Momen ditandai saat merekam (ms). */
  bookmarks: number[];
  /** Transkrip sudah dirapikan AI. */
  transcriptTidied: boolean;
};

export type TranscriptSegment = {
  id: number;
  channel: Channel;
  startMs: number;
  endMs: number;
  text: string;
  /** Whisper ragu (audio kurang jelas). */
  lowConfidence?: boolean;
};

export type AudioRetention = "after_transcript" | "days7" | "forever";

export type Settings = {
  userDisplayName: string;
  sttLanguage: "id" | "auto" | "mixed";
  /** Groq: model transkrip lebih akurat (lebih lambat). */
  sttHighAccuracy: boolean;
  /** Rapikan typo & tanda baca transkrip dengan AI. */
  tidyTranscript: boolean;
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
  /** Shortcut global tandai momen saat merekam; "" = mati. */
  bookmarkShortcut: string;
  /** Rekam otomatis (hitung mundur) saat Zoom/Teams/Google Meet terdeteksi. */
  autoRecord: boolean;
  /** Bahasa notulen: Indonesia / Inggris / ikuti bahasa meeting. */
  notesLanguage: "id" | "en" | "auto";
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
  /** Tenggat terstruktur YYYY-MM-DD. */
  dueDate: string | null;
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
  code: "device_lost" | "system_silent" | "system_ok" | "write_failed" | "limit_soon" | "paused_long";
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
