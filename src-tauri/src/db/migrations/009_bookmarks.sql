-- Langkah 44: momen yang ditandai pengguna saat merekam (ms timeline rekaman = waktu transkrip).
CREATE TABLE bookmarks (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  meeting_id  TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  at_ms       INTEGER NOT NULL,
  created_at  INTEGER NOT NULL
);
CREATE INDEX idx_bookmarks_meeting ON bookmarks(meeting_id, at_ms);
