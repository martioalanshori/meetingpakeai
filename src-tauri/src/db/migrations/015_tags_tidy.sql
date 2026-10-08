-- Langkah 59: label proyek/klien per meeting (feedback3 D4) dan teks asli transkrip sebelum dirapikan AI (G9).
CREATE TABLE tags (
  id    INTEGER PRIMARY KEY AUTOINCREMENT,
  name  TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE meeting_tags (
  meeting_id  TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  tag_id      INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (meeting_id, tag_id)
);
-- NULL = belum pernah dirapikan.
ALTER TABLE transcript_segments ADD COLUMN original_text TEXT;
