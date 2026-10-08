-- Langkah 51: riwayat "Tanya meeting ini" (feedback3 C1). `sources` = JSON array ms dari awal meeting.
CREATE TABLE meeting_qa (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  meeting_id  TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  question    TEXT NOT NULL,
  answer      TEXT NOT NULL,
  sources     TEXT NOT NULL DEFAULT '[]',
  created_at  INTEGER NOT NULL
);
CREATE INDEX idx_meeting_qa_meeting ON meeting_qa(meeting_id, id);
