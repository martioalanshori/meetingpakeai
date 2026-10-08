-- Langkah 42: sumber waktu tiap keputusan & tugas (ms dari awal meeting; NULL = tidak diketahui).
-- `decision_sources` = JSON array sejajar dengan `decisions` (decisions tetap array string agar
-- pencarian, ekspor, dan editor tidak berubah).
ALTER TABLE summaries ADD COLUMN decision_sources TEXT NOT NULL DEFAULT '[]';
ALTER TABLE action_items ADD COLUMN source_ms INTEGER;
