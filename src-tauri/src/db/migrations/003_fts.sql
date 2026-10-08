-- Langkah 28 (F11): pencarian teks. kind = title|summary|decision|topic|action|transcript; ref = start_ms transkrip.
-- Tabel virtual tanpa foreign key: baris dihapus manual bersama meeting (repo_meetings::delete).
CREATE VIRTUAL TABLE search_index USING fts5(
  meeting_id UNINDEXED,
  kind UNINDEXED,
  ref UNINDEXED,
  text,
  tokenize = 'unicode61 remove_diacritics 2'
);
