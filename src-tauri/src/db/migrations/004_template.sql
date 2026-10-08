-- Langkah 29 (F14): template ringkasan. meetings.summary_template NULL = otomatis (LLM mengenali jenis).
-- summaries.template = jenis yang dipakai (pilihan pengguna atau hasil kenali otomatis).
ALTER TABLE meetings ADD COLUMN summary_template TEXT;
ALTER TABLE summaries ADD COLUMN template TEXT;
