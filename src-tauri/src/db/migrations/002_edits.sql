-- Langkah 23: ringkasan bisa diedit pengguna. "Buat ulang ringkasan" minta konfirmasi jika edited = 1.
ALTER TABLE summaries ADD COLUMN edited INTEGER NOT NULL DEFAULT 0;
