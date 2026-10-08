-- Langkah 50: intisari 3 poin + pertanyaan terbuka (format notulen standar), instruksi & bahasa buat ulang.
ALTER TABLE summaries ADD COLUMN key_points TEXT NOT NULL DEFAULT '[]';
ALTER TABLE summaries ADD COLUMN open_questions TEXT NOT NULL DEFAULT '[]';
-- Sejajar `open_questions` (ms dari awal meeting atau NULL).
ALTER TABLE summaries ADD COLUMN open_question_sources TEXT NOT NULL DEFAULT '[]';
-- Instruksi tambahan dari pengguna saat "Buat ulang ringkasan" (NULL = tanpa).
ALTER TABLE meetings ADD COLUMN summary_instruction TEXT;
-- Bahasa notulen untuk meeting ini: 'id' / 'en' / 'auto'; NULL = ikuti pengaturan.
ALTER TABLE meetings ADD COLUMN summary_language TEXT;
