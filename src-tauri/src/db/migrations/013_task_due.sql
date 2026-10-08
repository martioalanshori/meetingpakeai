-- Langkah 54: tenggat terstruktur (YYYY-MM-DD) untuk pengingat & .ics; teks `due` tetap untuk tampilan.
ALTER TABLE action_items ADD COLUMN due_date TEXT;
-- Isi dari tanggal dalam kurung yang dihasilkan LLM, mis. "Jumat depan (2026-10-16)".
UPDATE action_items
SET due_date = substr(due, instr(due, '(') + 1, 10)
WHERE due GLOB '*([0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9])*';
