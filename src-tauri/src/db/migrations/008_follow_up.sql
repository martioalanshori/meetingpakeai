-- Langkah 43: draf pesan tindak lanjut per meeting (JSON {subject, body, lang}); NULL = belum dibuat.
-- Ikut terhapus saat ringkasan dibuat ulang (row summaries diganti).
ALTER TABLE summaries ADD COLUMN follow_up TEXT;
