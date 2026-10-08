# Rencana Eksekusi — `feedback2.md`

**Dibuat:** 2026-10-08 · **Basis:** commit `fd7800c` · **Sumber:** `feedback2.md`

**Aturan main:**
- Melanjutkan pola §19: satu langkah = satu sesi = satu commit. Nomor langkah dimulai dari 30.
- **Tanpa testing oleh AI.** Tidak ada `cargo test`, tidak menjalankan aplikasi, dan tidak ada uji manual. Pemilik yang menguji.
- Gerbang tiap langkah hanya pemeriksaan statis: `cargo check` + `cargo clippy --all-targets -- -D warnings` + `npm run check`.
- Tiap langkah ditutup dengan **checklist uji untuk pemilik**.
- **Di luar cakupan:**
  - bagian F (legal, rilis, keamanan lanjutan)
  - C1.3 dan C2.9 (pengujian dan pengukuran oleh pemilik)
  - C3 (CI dan cakupan test)
- Keputusan baru dicatat di tabel "Keputusan" `CLAUDE.md`, dan crate/paket baru di tabel versi.

**Urutan prioritas:** kehilangan data, lalu hal yang langsung terlihat tidak profesional di layar utama, lalu performa (mulai dari quick win), lalu poles layar lain, lalu inovasi.

---

## Fase 1 — Urgent (langkah 30–34)

### Langkah 30 — Keselamatan data
**Item:** C1.1, C1.2, C1.4, C1.5, C2.12

1. **C1.1 Stop gagal tidak boleh menghapus** (`recording.rs:335–347`, `audio/recorder.rs:259–261`)
   - `Recorder::stop` menyelesaikan writer kedua channel walau salah satu gagal, lalu baru mengembalikan error pertama.
   - Saat error, durasi dihitung dari part di disk (ekstrak fungsi bersama dari `queue/recovery.rs:16–37`, mis. `recovery::duration_from_parts`).
   - Penghapusan "< 5 dtk" hanya terjadi bila stop **sukses**. Kalau gagal dan durasi tidak bisa dihitung, meeting ditandai `interrupted`.
2. **C1.2 Gagal tulis audio** (`audio/recorder.rs:27–31`)
   - Penghitung gagal beruntun per channel (`AtomicU32` di `ChannelShared`). Log hanya untuk kegagalan pertama dan tiap kelipatan 100.
   - Monitor (`recording.rs`) membaca penghitung. Lebih dari 20 gagal beruntun → `StopReason::DiskFull` + event `recording://warning { code: "write_failed" }`.
3. **C1.4 Backup sebelum migrasi** (`db/mod.rs:60–70`)
   - Bila ada migrasi yang akan jalan dan file DB sudah ada → `VACUUM INTO 'app.sqlite.bak-v{n}'`. Simpan maksimal 3 backup terakhir.
   - Bila `user_version > MIGRATIONS.len()` → error yang jelas ("Database dibuat versi aplikasi yang lebih baru") dan aplikasi tidak start.
4. **C1.5 Panic hook** (`lib.rs`)
   - `std::panic::set_hook` yang menulis pesan + lokasi + versi secara sinkron ke `logs/crash-YYYYMMDD-HHMMSS.txt`.
   - `save_problem_report` ikut menyertakan file crash terbaru.
5. **C2.12 Updater** (`updater.rs:61–75`): cek ulang `is_recording()` dan worker tepat sebelum `restart()`.

**Uji pemilik:**
- Stop saat disk hampir penuh (atau file part dikunci) → meeting tidak hilang; muncul sebagai terputus atau tetap diproses.
- Buka aplikasi setelah update yang membawa migrasi → ada `app.sqlite.bak-v*`.

### Langkah 31 — Fondasi komponen & token UI
**Item:** B1.1, B1.4, B1.5, B1.6, B1.10, B2.1–B2.5, B2.15, A3

1. **Token `app.css`:**
   - tinta netral `#1C1F26` (A3); biru-hitam lama dipindah ke `--color-ink-strong` untuk tombol utama
   - `ink-faint` digelapkan ke ±`#6B727D` (B2.15)
   - token hover `ink-hover`, `rec-hover`, `bad-hover`
   - keluarga `*-bright` untuk latar gelap (warn, ok, bad, rec)
   - `--text-2xs`
   - radius 3 tingkat (`--radius-sm/md/lg`)
2. **Kelas komponen:** `.btn-sm`, `.btn-lg`, `.btn-danger-solid`, `.label`, `.section-title`, `.on-dark` (outline fokus putih, B1.6).
3. **Ikon baru** di `Icon.svelte`: `more-horizontal`, `x`, `plus`, `check-circle`, `alert-circle`, `arrow-left`, `speaker`, `download`, `pencil` (B1.5).
4. **`Menu.svelte` bersama** (B1.4): tombol pemicu + daftar item, tertutup saat klik di luar atau Escape, navigasi ↑/↓/Home/End, fokus awal masuk menu dan kembali ke pemicu.
5. **`ConfirmDialog.svelte` bersama:** judul, pesan, tombol konfirmasi (varian danger), dan API `confirmDialog({...}): Promise<boolean>`. Dipakai untuk:
   - hapus API key (`AiRoleEditor.svelte:71`, B1.1)
   - hapus rekaman terputus (`+page.svelte:198`, B1.10)
   - hapus meeting dan buat ulang ringkasan (menggantikan dua `<dialog>` di `MeetingDetail.svelte`)
6. **Ganti semua warna hex mentah dan ukuran huruf arbitrer** dengan token (B2.1–B2.3): `recorder/+page.svelte`, `Toaster.svelte`, `MeetingDetail.svelte`, `AppRail.svelte`, `RecordButton.svelte`. Buang glyph ✔ ✖ ← + dari `i18n/id.ts`.

**Uji pemilik:**
- Tidak ada lagi dialog "tauri.localhost says".
- Menu tertutup saat klik di luar.
- Fokus terlihat (Tab) di widget, banner, dan toast.
- Teks paragraf tidak lagi kebiruan.

### Langkah 32 — Halaman detail meeting (layar paling sering dilihat)
**Item:** A1, A4, A5, A6, A7, A8, B1.7, B1.8, B1.9, B2.7, B2.16, progress bar (B3)

1. **A1 durasi** (`format.ts:25`): "32 mnt", "1 jam 2 mnt"; detik hanya untuk durasi di bawah 1 menit. Satuan dipindah ke i18n.
2. **A8 header:** judul (`<h1>`, ikon pensil saat hover) + satu tombol ikon "Lainnya" (`Menu`) berisi Ekspor MD/TXT, Cetak/PDF, Buat ulang ringkasan (dengan alasan bila nonaktif), Transkrip ulang, Hapus.
3. **A6 toolbar:** Salin/Edit pindah ke ujung kanan baris tab sebagai `btn-sm`; baris toolbar terpisah dihapus.
4. **A4 status tunggal:** saat diproses, satu blok berisi label + persen + bar tipis; badge di meta disembunyikan. Bar tak tentu memakai animasi garis bergerak, bukan bar penuh berdenyut.
5. **A5 pesan error manusiawi** (`queue/worker.rs` `provider_failure`):
   - Petakan kasus umum ke pesan Indonesia + saran: 400 model tidak dikenal, 404, 413, respons tidak terbaca.
   - Detail teknis tetap di log.
   - Banner gagal menampilkan pesan + tombol "Coba lagi" (+ "Buka Pengaturan" bila terkait model/key).
6. **A7 + B1.8 transkrip:**
   - Garis vertikal per baris dihapus.
   - Satu tombol putar per baris di kolom waktu; teks menjadi teks biasa.
   - Baris yang sedang diputar disorot (diselesaikan di langkah 33).
7. **B1.7 tanpa kedipan:** saat ganti meeting, isi lama dipertahankan dengan opacity rendah sampai data baru tiba; skeleton hanya untuk muatan pertama.
8. **B1.9 edit aman:** saat mengedit ringkasan, tab lain nonaktif; "Batal" dengan perubahan meminta konfirmasi (`ConfirmDialog`).
9. **B2.7 empty state:** "—" diganti kalimat sesuai status (gagal, terputus, tanpa percakapan) + aksi.
10. **B2.16:** tautan "Kembali" (ikon `arrow-left`) di header saat `!embedded`.

**Uji pemilik:**
- Buka meeting selesai, sedang diproses, gagal, dan terputus di 1120 px dan 1920 px.
- Ganti meeting di panel kanan tanpa kedipan.
- Tab transkrip bisa dinavigasi keyboard tanpa ratusan tab stop.

### Langkah 33 — Pemutar audio kustom
**Item:** B1.2, C2.5

1. **`AudioPlayer.svelte`** di atas `<audio>` tersembunyi: putar/jeda, seek bar (tinta), waktu tabular, kecepatan 1×/1,5×/2×. Menempel di atas transkrip saat aktif.
2. **Sorot baris yang sedang diputar** dari `timeupdate` (pencarian biner pada `startMs`), dengan auto-scroll lembut bila baris keluar layar (bisa dimatikan dengan scroll manual).
3. **C2.5:** `playback.wav` dibuat di latar belakang setelah job `done` bila audio masih disimpan (bukan saat klik pertama), lalu dihapus bersama retensi. Klik sebelum siap menampilkan status "Menyiapkan audio…".

**Uji pemilik:** putar dari beberapa kalimat, seek, ganti kecepatan, dan buka meeting 1 jam (klik pertama tidak menunggu lama).

### Langkah 34 — Hirarki visual & tata letak responsif
**Item:** A2, A9, A15, B2.6, B2.8, toast (B3), lebar & radius konten (B3), tata letak windowed vs layar penuh (masukan pemilik)

1. **A2 bidang:** rel dan daftar tetap di warna kertas, area isi di bidang putih (`sheet`) dengan tepi kiri. Di layar lebar, panel detail putih penuh tinggi.
2. **A9 identitas:** logo di atas rel (ikon saja di mode ringkas) + judul jendela per halaman ("Meeting Pake AI — {judul meeting}").
3. **A15:** status "Selesai" tidak ditampilkan di daftar; hanya status yang perlu perhatian.
4. **B2.6 skeleton:** daftar meeting, detail, Tugas, dan Pengaturan; state error + "Coba lagi". Versi di Pengaturan tidak tampil sebelum termuat.
5. **B2.8:** `loadingMore` + tombol nonaktif.
6. **Toast:** error memakai `role="alert"`, bertahan 8 dtk atau sampai ditutup, maksimal 3 sekaligus; ikon `x` SVG.
7. **Konsistensi:** lebar konten diseragamkan (daftar/Tugas/Pengaturan `max-w-3xl`, detail `max-w-5xl`) dan radius memakai 3 token.

**Tata letak windowed vs layar penuh** (masukan pemilik: tampilan terasa berbeda jauh antara jendela dan layar penuh). Tujuannya agar perubahan antar-ukuran bertahap, bukan lompat di satu titik.

8. **Dua panel aktif lebih awal.**
   - Batas dua panel turun dari 1280 px ke **1100 px**, sehingga ukuran jendela awal (1120 px) langsung memakai daftar + detail.
   - Batas rel penuh naik dari 1024 px ke **1280 px**: di 1100–1279 px rel otomatis ringkas (ikon) agar panel detail cukup lebar. Kira-kira rel 68 px + daftar ±300 px + detail ±750 px di 1120 px.
   - Breakpoint dipusatkan di `src/lib/viewport.svelte.ts` (`SPLIT_QUERY`, `RAIL_FULL_QUERY`) dan token CSS yang sama, supaya tidak ada angka ganda.
9. **Mode satu kolom tidak menempel ke kiri.**
   - Di bawah 1100 px (dan untuk Tugas & Pengaturan di semua ukuran), kolom isi berada di tengah area isi (`mx-auto`) dengan lebar maksimum tetap, sehingga ruang kosong terbagi rata kiri-kanan.
   - Padding mengikuti lebar (24 px di 800 px, 40 px di ≥ 1280 px).
10. **Tata letak di dalam panel mengikuti lebar panel, bukan lebar layar.**
    - Pakai **container query** Tailwind 4 (`@container` pada area isi detail).
    - Ringkasan + kolom tugas berdampingan saat **panel detail ≥ ±960 px**, tidak lagi menunggu layar ≥ 1536 px. Ini berlaku sama untuk detail mandiri dan panel kanan, sehingga di 1440 px layar penuh pun ringkasan dan tugas sudah berdampingan.
    - Hal yang sama untuk toolbar detail (tombol berlabel di panel lebar, ikon saja di panel sempit) dan baris daftar (status di kanan saat lebar, di bawah judul saat sempit).
11. **Ukuran jendela awal dan transisi.**
    - Jendela awal tetap 1120×760 (sudah dua panel setelah poin 8).
    - Posisi dan ukuran jendela terakhir disimpan dan dipulihkan, sehingga pengguna yang sering memaksimalkan tidak perlu mengulang.
    - Saat ukuran berubah melewati batas, meeting terpilih dan tab aktif tetap dipertahankan.

**Uji pemilik:**
- Tampilan Beranda, detail, Tugas, dan Pengaturan di **800, 1024, 1120, 1280, 1440, dan 1920 px**.
- Perubahan antar-ukuran terasa bertahap; tidak ada titik di mana tata letak tiba-tiba melompat jauh.
- Ukuran jendela awal langsung dua panel.
- Ringkasan + tugas berdampingan di 1440 px.
- Jendela dibuka lagi dengan ukuran terakhir.

---

## Fase 2 — Performa (langkah 35–37)

### Langkah 35 — Quick win performa
**Item:** C2.4, C2.6 (sebagian), C2.7, C2.8, C2.13

1. **C2.4** (`stt/openai.rs:12`): timeout = 60 dtk + 20 dtk per MB, maks 15 menit; `connect_timeout` 15 dtk.
2. **C2.6:**
   - Folder `upload/` dihapus setelah step merging sukses (chunk WAV tidak dibutuhkan lagi; transkrip ulang membuat ulang).
   - Pengaturan menampilkan ukuran `recordings/` + tombol "Hapus semua audio lama" (`ConfirmDialog`).
3. **C2.7:** penyapuan retensi menyertakan `interrupted` dengan batas 30 hari.
4. **C2.8:** `remove_dir_all` dan akses DB berat di worker dibungkus `spawn_blocking`.
5. **C2.13:** pesan error berulang di-rate-limit (1 per jenis per menit) + batas ukuran log per hari.

**Uji pemilik:** ukuran folder rekaman turun setelah meeting selesai; tombol hapus audio bekerja; proses tetap jalan di koneksi lambat.

### Langkah 36 — Unggah lebih kecil & paralel
**Item:** C2.3, C2.2

1. **C2.3 kompresi:**
   - Chunk dikodekan ke **FLAC** (lossless, ±50% lebih kecil, diterima Whisper/Groq/OpenAI) memakai encoder Rust murni (evaluasi `flacenc`; catat versi di `CLAUDE.md`).
   - Fallback ke WAV bila encoder gagal.
   - Offset map tidak berubah karena timeline sama. Batas ukuran chunk tetap di bawah 25 MB.
2. **C2.2 paralel** (`queue/worker.rs:432`):
   - Chunk STT diproses dengan konkurensi 3 (`futures::stream::buffer_unordered` atau `JoinSet` + `Semaphore`).
   - Untuk Groq, rate limiter tetap memeriksa tiap request.
   - Progres dan `mark_done` tetap per chunk; urutan hasil tidak penting karena merge mengurutkan menurut waktu.

**Uji pemilik:** meeting 30–60 menit selesai lebih cepat dibanding sebelumnya; hasil transkrip tidak berubah kualitasnya; bandingkan Groq dan satu penyedia lain.

### Langkah 37 — Transkripsi bertahap selama merekam (E1 / C2.1)
**Item:** C2.1, E1. Langkah terbesar dan paling berdampak; dikerjakan setelah 35–36 stabil.

**Desain:**
1. **Pemicu.** Selama merekam, `RecordingService` memberi tahu worker setiap kali part 60 dtk di **kedua channel** final, lewat event internal (bukan UI).
2. **Praproses inkremental per channel.**
   - Simpan "titik potong" terakhir (sampel) per channel di tabel baru `live_progress`, mis. migrasi `006_live.sql`: `meeting_id, channel, processed_samples`.
   - Bila audio final yang belum diproses ≥ ±5 menit, jalankan VAD pada rentang itu, buat chunk + offset map dengan `orig_ms` absolut, insert ke `upload_chunks` (idx berlanjut), lalu transkrip.
   - Potongan hanya di jeda VAD (region yang menyeberang batas ditunda ke putaran berikutnya agar kalimat tidak terpotong).
3. **Setelah Stop.** Step `preprocessing` hanya memproses sisa setelah `processed_samples`, lalu `transcribing` (sisa chunk), `merging`, dan `summarizing` seperti biasa.
4. **Batasan.**
   - Bila API key belum ada, penyedia gagal, atau offline: lewati mode bertahap; semua diproses setelah Stop seperti sekarang.
   - Transkrip ulang dan recovery tetap memakai jalur penuh (hapus `live_progress`).
5. **UI.** Widget menampilkan teks kecil "Transkrip berjalan"; detail meeting yang sedang direkam belum menampilkan transkrip (bisa jadi fitur berikutnya).
6. **Prioritas worker.** Meeting live tidak memblokir antrean meeting lain lebih dari satu chunk.

**Uji pemilik:**
- Rekam 15–30 menit → notulen siap kurang dari ±1 menit setelah Stop.
- Matikan internet di tengah rekaman → hasil akhir tetap lengkap setelah online.
- Kill proses saat merekam → recovery dan "Proses" tetap benar.

---

## Fase 3 — Poles layar lain (langkah 38–40)

### Langkah 38 — Pengaturan
**Item:** A10, A11, B2.9, B2.10, B2.11, B2.12, B3 (ShortcutInput, gaya bahasa)

1. **Sub-navigasi** (tab vertikal di kiri isi, atau segmen di atas): Rekaman, Layanan AI, Aplikasi, Bantuan. Tiap kelompok dalam bidang putih.
2. **Kontrol kustom:** `Select.svelte`, radio-card (pilihan retensi, bahasa), dan switch untuk toggle.
3. **B2.9 simpan otomatis per kontrol** dengan toast "Tersimpan"; tombol "Simpan" global dihapus. Shortcut dan autostart tetap memvalidasi dulu.
4. **A11 baris API key:** status di kiri; "Ganti"/"Hapus" di `Menu` kecil; "Buat API key" sebagai tautan di bawah input. Tombol "Uji & simpan" nonaktif disertai alasan ("Ubah penyedia, model, atau key untuk menguji").
5. **Teks:**
   - B2.10 "Simpan laporan masalah…"
   - B2.11 petunjuk Nama Anda
   - B2.12 glosarium istilah di kepala `id.ts`: **Notulen** = dokumen hasil; **Ringkasan** = paragraf; **Tugas** = action item; **Audio komputer** = channel sistem. Seragamkan seluruh teks, termasuk "Action Items" menjadi "Tugas".
   - Gaya bahasa: "Ubah" untuk "Edit", "Hentikan" untuk "Stop", tanda baca toast.
6. **ShortcutInput:** diberi label, dan petunjuk ganda dihapus.

**Uji pemilik:** semua pengaturan tersimpan tanpa tombol Simpan; istilah konsisten di semua layar.

### Langkah 39 — Widget, onboarding, Tugas, cetak
**Item:** A12, A13, A14, B1.3, B2.13, B2.14, B2.17, B3 (Tugas, teks kosong ganda, angka konfigurasi)

1. **Widget:**
   - ikon mic & speaker di kiri meter (A12, B2.14)
   - tombol stop memakai `rec`
   - tinggi jendela diukur dari konten dengan `ResizeObserver` → `setSize` (B2.13)
   - error hilang otomatis setelah 6 dtk
   - angka "10 menit"/"2 menit" diambil dari payload event
2. **Onboarding:**
   - bidang putih di tengah, logo lebih besar (A13)
   - tombol "Kembali" per langkah
   - instruksi "Ucapkan…" tampil sebelum tombol tes (B2.17)
3. **Tugas:**
   - filter "Hanya tugas saya" (A14)
   - pencocokan nama per kata utuh, bukan substring
   - empty state membedakan "belum ada tugas" dari "tersaring"
4. **B1.3 Cetak/PDF:** markup notulen asli (kop: logo, judul, tanggal, durasi; Ringkasan, Keputusan, Tugas, Topik; transkrip opsional) + `@media print` dan `@page { margin: 18mm }`.

**Uji pemilik:** widget saat auto-stop/error tidak terpotong; PDF hasil cetak terlihat seperti dokumen notulen.

### Langkah 40 — Data besar & meeting panjang
**Item:** C2.10, C2.11, B3 (aksesibilitas tab, judul)

1. **Virtualisasi daftar meeting** (render baris terlihat saja, tinggi baris tetap) dan transkrip di atas 1.000 segment (blok per 200 baris dengan `content-visibility`, atau virtual list).
2. **C2.11:**
   - Peringatan 10 menit sebelum batas 4 jam (event + notifikasi + baris di widget).
   - Saat batas tercapai, rekaman berlanjut otomatis sebagai meeting baru bertajuk "(lanjutan)", bukan berhenti.
3. **Aksesibilitas:** tab dengan `aria-controls`/`aria-labelledby` + navigasi panah; judul `break-words`.

**Uji pemilik:** gulir ratusan meeting dan transkrip panjang tetap mulus.

---

## Fase 4 — Inovasi gelombang 1 (langkah 41–44)

### Langkah 41 — Glosarium kosakata (E2)
1. **Setting `sttGlossary`:** teks bebas, maks 800 karakter, satu istilah per baris. Ditampilkan di Pengaturan → Rekaman dengan contoh.
2. **Prompt STT** (`queue/worker.rs:472`, `prompt: None`): kirim glosarium + nama PJ yang sering muncul dari meeting 30 hari terakhir (dedup, dipotong sesuai batas).
3. **Prompt LLM:** baris "Ejaan nama & istilah yang benar: …" di prompt CHUNK/FINAL/MERGE.

**Uji pemilik:** nama dan istilah yang dimasukkan tertulis benar di transkrip dan notulen.

### Langkah 42 — Sumber waktu untuk setiap poin (E3)
1. **Prompt FINAL/MERGE:** `keputusan` menjadi `[{ "teks": "...", "sumber": "HH:MM:SS" }]` dan action item mendapat field `sumber`. Parser menerima format lama (string) agar meeting lama tetap terbaca.
2. **Skema:** migrasi `ALTER TABLE action_items ADD COLUMN source_ms`; `summaries.decisions` menyimpan objek JSON. Validasi `sumber` berada dalam durasi meeting (di luar rentang → null).
3. **UI:** chip waktu kecil di tiap keputusan/tugas. Klik membuka tab Transkrip, menggulir ke baris terdekat, dan memutar audio (bila tersedia).

**Uji pemilik:** chip mengarah ke bagian transkrip yang benar.

### Langkah 43 — Draf pesan tindak lanjut + salin berformat (E4, E10)
1. **Tombol "Buat pesan tindak lanjut"** di toolbar Ringkasan → satu panggilan LLM dari notulen (bukan transkrip) → panel dengan draf (formal Indonesia; opsi Inggris).
   - Aksi: Salin, Buka di email (`mailto:` dengan subjek = judul).
   - Draf disimpan per meeting (kolom baru) agar tidak dibuat ulang.
2. **E10:** `CopyButton` menulis `text/html` + `text/plain` (`navigator.clipboard.write`), dengan judul tebal dan daftar berpoin saat ditempel ke email/Docs/Word.

**Uji pemilik:** draf masuk akal dan siap kirim; tempel ke Gmail/Outlook/Docs tetap rapi.

### Langkah 44 — Tandai momen penting (E5)
1. **Tombol bintang di widget** + shortcut global kedua (default `Ctrl+Alt+B`, bisa diubah) → simpan `elapsed_ms` ke tabel `bookmarks(meeting_id, at_ms)` (migrasi baru).
2. **Ringkasan:** momen ditandai dikirim ke LLM ("pengguna menandai menit-menit ini sebagai penting, wajib dibahas") dan ditampilkan sebagai bagian "Momen ditandai" berisi kutipan transkrip ±20 dtk.
3. **Transkrip:** penanda bintang di baris terdekat.

**Uji pemilik:** tandai 2–3 momen saat meeting → muncul di ringkasan dan transkrip.

---

## Backlog (setelah langkah 44, urut prioritas)

1. E6 Tanya meeting ini
2. E8 Rangkaian meeting & status tindak lanjut
3. E9 Pengingat tenggat + `.ics`
4. E7 Tanya semua meeting
5. E11 Ringkasan mingguan
6. E12 Indikator kualitas transkrip
7. E13 Statistik bicara
8. E17 Lewati hening saat memutar
9. E14 Transkripsi lokal
10. E15 Kalender
11. E16 Integrasi alat tugas
12. B3 sisa (kunci i18n mati, umpan balik ganda CopyButton, `minutes.ts` whatsapp, radius minor)

Bagian **F** di `feedback2.md` tetap ditunda sampai pemilik memutuskan menyiapkan rilis.

---

## Ringkasan urutan

| Fase | Langkah | Fokus | Estimasi |
|---|---|---|---|
| 1 Urgent | 30–34 | Keselamatan data + fondasi UI + detail meeting + pemutar + hirarki & tata letak responsif | ±8–10 hari |
| 2 Performa | 35–37 | Quick win, FLAC + paralel, transkripsi bertahap | ±6–8 hari |
| 3 Poles | 38–40 | Pengaturan, widget/onboarding/Tugas/PDF, data besar | ±5–6 hari |
| 4 Inovasi | 41–44 | Glosarium, sumber waktu, draf pesan, tandai momen | ±6–8 hari |
