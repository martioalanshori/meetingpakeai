-- Template ringkasan dihapus (keputusan pemilik, 2026-10-08): semua meeting memakai satu format notulen standar.
ALTER TABLE meetings DROP COLUMN summary_template;
ALTER TABLE summaries DROP COLUMN template;
