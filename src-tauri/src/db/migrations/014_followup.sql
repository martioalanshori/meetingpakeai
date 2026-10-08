-- Langkah 55: status tindak lanjut tugas terbuka dari meeting sebelumnya yang berkaitan (meeting rutin).
-- `followup_status` = JSON [{itemId, task, status: selesai|dibahas|belum_disebut, note}].
ALTER TABLE summaries ADD COLUMN followup_status TEXT NOT NULL DEFAULT '[]';
ALTER TABLE summaries ADD COLUMN followup_from TEXT;
