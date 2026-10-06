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
};

export type MeetingSummary = {
  status: "ok" | "empty";
  summary: string | null;
  decisions: string[];
  topics: string[];
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

export type Settings = {
  userDisplayName: string;
  sttLanguage: "id" | "auto";
  deleteAudioAfterTranscript: boolean;
  minimizeToTray: boolean;
  consentMessage: string;
};

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
};

export type TestApiKeyResult = { ok: boolean; missingModels: string[] };

// Payload event (§12.4)
export type LevelPayload = { micDbfs: number; systemDbfs: number };
export type AutoStopWarningPayload = { reason: "silence"; secondsLeft: number };
export type RecordingWarningPayload = { code: "device_lost"; channel: Channel };
export type JobProgressPayload = {
  meetingId: string;
  status: MeetingStatus;
  progressDone: number;
  progressTotal: number;
};
export type MeetingUpdatedPayload = { meetingId: string };
