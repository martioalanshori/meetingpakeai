-- Langkah 49: catatan pribadi pengguna per meeting (ditulis selama/sesudah meeting, ikut membentuk notulen).
CREATE TABLE meeting_notes (
  meeting_id  TEXT PRIMARY KEY REFERENCES meetings(id) ON DELETE CASCADE,
  text        TEXT NOT NULL,
  updated_at  INTEGER NOT NULL
);
