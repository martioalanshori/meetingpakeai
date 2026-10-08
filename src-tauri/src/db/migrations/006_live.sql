-- Langkah 37: transkripsi bertahap selama merekam. processed_samples = batas audio channel yang sudah
-- dipotong jadi chunk unggah; setelah Stop praproses hanya mengerjakan sisanya.
CREATE TABLE live_progress (
  meeting_id        TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  channel           TEXT NOT NULL CHECK (channel IN ('mic','system')),
  processed_samples INTEGER NOT NULL,
  PRIMARY KEY (meeting_id, channel)
);
