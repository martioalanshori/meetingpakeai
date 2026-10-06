CREATE TABLE meetings (
  id              TEXT PRIMARY KEY,               -- uuid v4
  title           TEXT NOT NULL,                  -- default "Meeting 6 Okt 2026 14.00"
  title_edited    INTEGER NOT NULL DEFAULT 0,
  created_at      INTEGER NOT NULL,
  started_at      INTEGER NOT NULL,
  ended_at        INTEGER,
  duration_ms     INTEGER NOT NULL DEFAULT 0,     -- timeline time (tanpa pause)
  language        TEXT NOT NULL,                  -- 'id' | 'auto'
  source_app      TEXT,                           -- 'zoom' | 'teams' | 'browser' | NULL
  consent_at      INTEGER NOT NULL,               -- waktu pengguna mencentang consent
  status          TEXT NOT NULL,                  -- lihat §13
  failed_step     TEXT,                           -- 'preprocessing'|'transcribing'|'merging'|'summarizing'
  error_code      TEXT,
  error_message   TEXT,
  progress_done   INTEGER NOT NULL DEFAULT 0,
  progress_total  INTEGER NOT NULL DEFAULT 0,
  attempts        INTEGER NOT NULL DEFAULT 0,
  next_run_at     INTEGER,
  audio_deleted   INTEGER NOT NULL DEFAULT 0,
  updated_at      INTEGER NOT NULL
);
CREATE INDEX idx_meetings_created ON meetings(created_at DESC);
CREATE INDEX idx_meetings_status ON meetings(status);

CREATE TABLE recording_parts (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  meeting_id  TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  channel     TEXT NOT NULL CHECK (channel IN ('mic','system')),
  part_index  INTEGER NOT NULL,
  path        TEXT NOT NULL,                      -- relatif terhadap app_data_dir
  samples     INTEGER NOT NULL DEFAULT 0,
  finalized   INTEGER NOT NULL DEFAULT 0,
  UNIQUE (meeting_id, channel, part_index)
);

CREATE TABLE upload_chunks (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  meeting_id      TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  channel         TEXT NOT NULL CHECK (channel IN ('mic','system')),
  idx             INTEGER NOT NULL,
  path            TEXT NOT NULL,
  duration_ms     INTEGER NOT NULL,
  offset_map_json TEXT NOT NULL,
  stt_status      TEXT NOT NULL DEFAULT 'pending' CHECK (stt_status IN ('pending','done','failed')),
  attempts        INTEGER NOT NULL DEFAULT 0,
  response_json   TEXT,
  error_message   TEXT,
  UNIQUE (meeting_id, channel, idx)
);

CREATE TABLE transcript_segments (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  meeting_id        TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  channel           TEXT NOT NULL CHECK (channel IN ('mic','system')),
  start_ms          INTEGER NOT NULL,
  end_ms            INTEGER NOT NULL,
  text              TEXT NOT NULL,
  no_speech_prob    REAL,
  avg_logprob       REAL,
  compression_ratio REAL,
  is_filtered       INTEGER NOT NULL DEFAULT 0,
  is_duplicate      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_segments_meeting ON transcript_segments(meeting_id, start_ms);

CREATE TABLE summaries (
  meeting_id   TEXT PRIMARY KEY REFERENCES meetings(id) ON DELETE CASCADE,
  status       TEXT NOT NULL CHECK (status IN ('ok','empty')),
  summary      TEXT,                              -- field "ringkasan"
  decisions    TEXT NOT NULL DEFAULT '[]',        -- JSON array string
  topics       TEXT NOT NULL DEFAULT '[]',        -- JSON array string
  model        TEXT NOT NULL,
  created_at   INTEGER NOT NULL
);

CREATE TABLE action_items (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  meeting_id  TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  idx         INTEGER NOT NULL,
  task        TEXT NOT NULL,
  assignee    TEXT,
  due         TEXT,
  done        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE meeting_speaker_names (              -- Beta (F10); tabel dibuat sejak MVP
  meeting_id    TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  channel       TEXT NOT NULL CHECK (channel = 'system'),
  display_name  TEXT NOT NULL,
  PRIMARY KEY (meeting_id, channel)
);

CREATE TABLE usage_log (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  ts         INTEGER NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('stt','llm')),
  audio_sec  REAL NOT NULL DEFAULT 0,
  tokens     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_usage_ts ON usage_log(kind, ts);

CREATE TABLE settings (
  key    TEXT PRIMARY KEY,
  value  TEXT NOT NULL                            -- JSON
);
