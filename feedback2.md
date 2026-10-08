# Feedback 2 — Tampilan Profesional, Performa & Inovasi

**Tanggal:** 8 Oktober 2026 · **Basis:** commit `fd7800c` (v0.1.0)

**Cara analisis:**
- **Visual:** build frontend dijalankan di Edge headless dengan backend tiruan berisi data contoh (8 meeting, notulen, transkrip, tugas). Tiap layar dipotret di 800×600, 1120×760, dan 1920×1080, juga dalam kondisi sedang merekam.
- **Kode UI:** audit seluruh file Svelte, CSS, dan teks i18n.
- **Keandalan & performa:** audit backend Rust (perekaman, antrean, penyimpanan, pengujian).

Semua temuan sudah dicek ke kode. Lokasi ditulis `file:baris`, relatif ke root repo.

**Prioritas:**
- **P0:** wajib, mengganggu pengguna atau berisiko kehilangan data.
- **P1:** penting untuk kesan profesional dan performa.
- **P2:** penyempurnaan.

---

## 0. Ringkasan

Fungsinya sudah lengkap dan alurnya jalan. Yang membuatnya belum terasa produk jadi ada di tiga lapis.

1. **Detail yang terlihat "web prototype".** Dialog `confirm()` bawaan browser, pemutar audio bawaan, select/radio bawaan, menu yang tidak tertutup saat klik di luar, ikon berupa karakter teks (⋯ ✕ ✔), dan durasi "32 m 00 d".
2. **Hirarki visual datar.** Rel, daftar, dan isi semuanya di atas warna kertas yang sama tanpa bidang yang membedakan. Toolbar menggantung sendirian, dan informasi status diulang tiga kali di halaman yang sedang diproses.
3. **Keandalan dan performa belum terbukti.** Ada satu bug yang bisa menghapus rekaman (C1.1). Pemrosesan baru dimulai setelah Stop dan chunk dikirim satu per satu (C2.1–C2.2). Checklist uji manual belum dijalankan.

Bagian **E** berisi inovasi agar notetaker benar-benar bermanfaat. Item legal dan rilis ditunda ke bagian **F** atas keputusan pemilik.

---

## A. Tampilan — temuan dari tangkapan layar

| # | Prio | Yang terlihat | Kenapa terasa tidak profesional | Perbaikan |
|---|---|---|---|---|
| A1 | **P0** | Durasi tampil "32 m 00 d", "14 m 00 d" di setiap baris dan header | Satuan "d" (detik) membingungkan, dan "00 d" adalah informasi kosong yang diulang di semua baris | `format.ts:25` → "32 mnt", "1 jam 2 mnt"; detik hanya untuk durasi < 1 menit |
| A2 | **P0** | Semua bidang (rel, daftar, isi) memakai warna kertas `#EEF1EC` yang sama | Tidak ada kedalaman; mata tidak tahu mana navigasi dan mana isi | Rel di warna kertas, area isi di bidang putih (`sheet`) dengan tepi, atau sebaliknya. Di layar lebar, panel detail sebaiknya putih |
| A3 | **P0** | Teks paragraf ringkasan terlihat kebiruan seperti link (`--color-ink #1E2433` di bobot 400) | Paragraf panjang terbaca seperti tautan; mengurangi keterbacaan | Tinta dibuat lebih netral (mis. `#1C1F26`); biru-hitam hanya untuk tombol utama |
| A4 | **P0** | Halaman diproses menampilkan "Membuat ringkasan" tiga kali: badge di meta, label di atas bar, dan bar progres | Repetitif; terlihat belum dipoles | Satu komponen status: label + persen + bar tipis; badge di meta disembunyikan saat bar tampil |
| A5 | **P0** | Banner gagal menampilkan "Layanan AI menolak permintaan: HTTP 400" | Pesan teknis mentah; pengguna tidak tahu harus apa | Pesan per kasus ("Model tidak dikenali penyedia. Periksa nama model di Pengaturan.") + detail teknis di log |
| A6 | **P1** | Baris Salin/Edit berdiri sendiri di bawah tab, kanan atas, dengan ruang kosong di kirinya | Membuang satu baris vertikal; terlihat seperti tempelan | Pindahkan ke baris tab (kanan), atau jadikan ikon kecil di header bagian |
| A7 | **P1** | Transkrip: setiap baris punya garis vertikal abu-abu 3 px | Deretan garis terlihat seperti kutipan berulang; ramai tanpa makna (label pembicara sudah dihapus atas keputusan pemilik) | Hapus garis per baris; cukup kolom waktu + teks dengan jarak antar-paragraf. Sorot baris yang sedang diputar |
| A8 | **P1** | Header detail: judul besar + tombol "Ekspor" bergaris + "⋯" | Tombol sekunder bersaing dengan judul; "⋯" adalah karakter teks | Satu tombol ikon "Lainnya" (ikon SVG) berisi Ekspor, Buat ulang, Transkrip ulang, Hapus; atau Ekspor jadi ikon |
| A9 | **P1** | Rel kiri tanpa identitas aplikasi (logo dihapus dari rel) | Jendela utama terlihat generik; tidak ada "rumah" visual | Logo kecil di atas rel (ikon saja di mode ringkas), atau set judul jendela per halaman |
| A10 | **P1** | Pengaturan: satu kolom panjang (±1.600 px), radio & select bawaan Windows, tombol "Uji & simpan" abu-abu nonaktif tampak rusak | Terasa seperti formulir web; tidak ada pengelompokan visual | Kelompokkan per kartu/bagian dengan navigasi sub-halaman (Rekaman, Layanan AI, Aplikasi, Bantuan); kontrol kustom; tombol nonaktif dengan teks penjelas |
| A11 | **P1** | Baris "API key" berisi lima elemen sejajar: label, status, Ganti, Hapus, Buat API key | Padat; aksi destruktif (Hapus) sejajar dengan link biasa | Status di satu baris; aksi di menu kecil atau tombol sekunder di kanan |
| A12 | **P1** | Widget rekaman: kedua meter berupa garis abu-abu identik saat hening; tombol stop merah muda | Tidak jelas meter mana yang mic dan mana suara peserta | Ikon mic dan speaker kecil di kiri tiap meter; stop memakai merah rekam yang sama dengan aplikasi |
| A13 | **P2** | Onboarding: konten kecil di tengah kanvas kosong besar | Polos; momen pertama pengguna kurang meyakinkan | Bidang putih di tengah dengan bayangan halus, logo lebih besar, ilustrasi sederhana "dua suara" |
| A14 | **P2** | Tugas: filter "Hanya untuk saya (PJ: Saya)" | Kalimat janggal saat nama default "Saya" | "Hanya tugas saya" saja; nama ditampilkan di Pengaturan |
| A15 | **P2** | Status "Selesai" bertitik hijau di setiap baris (mode satu kolom) | Kebisingan visual; mayoritas meeting memang selesai | Tampilkan status hanya jika bukan "Selesai" (seperti mode dua panel) |

---

## B. Tampilan — temuan dari audit kode UI

### B1. Kritis (P0)

| # | Masalah | Lokasi | Perbaikan |
|---|---|---|---|
| B1.1 | Hapus API key memakai `confirm()` bawaan browser ("tauri.localhost says…") | `src/lib/components/AiRoleEditor.svelte:71` | Komponen `ConfirmDialog.svelte` bersama (pola `dialog.sheet-dialog` sudah ada di `MeetingDetail.svelte`) |
| B1.2 | Pemutar audio = elemen `<audio controls>` bawaan WebView2 | `MeetingDetail.svelte:577` | Kontrol kustom: putar/jeda, seek bar tinta, waktu tabular; sorot kalimat yang sedang diputar |
| B1.3 | "Cetak / simpan PDF" mencetak `<pre>` teks polos | `MeetingDetail.svelte:253–257` | Stylesheet `@media print` yang mencetak markup asli (judul, daftar, kop tanggal, logo) dengan margin `@page` |
| B1.4 | Menu ⋯ dan Ekspor tidak tertutup saat klik di luar; tanpa navigasi panah; fokus tidak masuk/kembali | `MeetingDetail.svelte:298–370` | Komponen `Menu.svelte` bersama: klik-luar, ↑/↓/Home/End, fokus awal & kembali |
| B1.5 | Karakter teks sebagai ikon: "⋯", "✕", "+ Tambah", "✔/✖", "←" | `MeetingDetail.svelte:339`, `Toaster.svelte:21`, `i18n/id.ts:8,161,284–287` | Tambah ikon `more-horizontal`, `x`, `plus`, `check-circle`, `alert-circle`, `arrow-left` di `Icon.svelte`; buang glyph dari teks |
| B1.6 | Outline fokus tinta tidak terlihat di latar gelap (widget, banner, toast) | `app.css:63–66` | `.on-dark :focus-visible { outline-color: #fff }` |
| B1.7 | Membuka meeting di layar lebar mengosongkan panel lalu menampilkan "Memuat…" → layout melompat setiap klik | `MeetingDetail.svelte:89–105, 268–269` | Pertahankan isi lama (diredupkan) sampai data baru datang, atau skeleton berstruktur sama |
| B1.8 | Setiap kalimat transkrip adalah `<button>` → ribuan tab stop di meeting 1 jam | `MeetingDetail.svelte:591–596` | Satu tombol putar per baris (di timestamp), teks sebagai teks biasa |
| B1.9 | Pindah tab saat mengedit ringkasan membuang perubahan tanpa peringatan; "Batal" juga | `MeetingDetail.svelte:441–443`, `SummaryEditor.svelte:96` | Kunci tab saat mengedit atau minta konfirmasi bila ada perubahan |
| B1.10 | "Hapus" rekaman terputus langsung menghapus permanen tanpa konfirmasi | `src/routes/+page.svelte:198` | Dialog konfirmasi + gaya `btn-danger` |

### B2. Penting (P1)

| # | Masalah | Lokasi | Perbaikan |
|---|---|---|---|
| B2.1 | Warna hex mentah di luar token (`#2e3650`, `#b9241a`, `#f0b43c`, `#ff7a6e`, `#4a3410`, `#931c13`, dll.) | `app.css:101,126`, `recorder/+page.svelte:144–225`, `Toaster.svelte:13`, `MeetingDetail.svelte:612` | Token `ink-hover`, `rec-hover`, `bad-hover`, keluarga `*-bright` untuk latar gelap |
| B2.2 | Status "Dijeda" dua warna berbeda (widget amber terang, rel cokelat) | `recorder/+page.svelte:144`, `RecordButton.svelte:58` | Satu token + varian terang untuk latar gelap |
| B2.3 | Ukuran huruf arbitrer yang menduplikasi skala (`text-[0.9375rem]`, `text-[1.0625rem]`, `text-[10px]`, `text-[0.6875rem]`) | `AppRail.svelte:33`, `MeetingDetail.svelte:428,466,475`, `recorder:150,156`, `RecordButton.svelte:43` | Pakai token skala; tambah `--text-2xs` bila perlu |
| B2.4 | Gaya label field berbeda di tiap halaman (font-medium / text-sm / font-semibold / font-bold) | `settings:86–134`, `AiRoleEditor:90–113`, `SummaryEditor:50–67` | Kelas `.label` dan `.section-title` tunggal |
| B2.5 | Ukuran tombol diatur manual di banyak tempat | `onboarding:83`, `CopyButton.svelte:25`, `AiRoleEditor:119`, `MeetingDetail:330` | Varian `.btn-sm` / `.btn-lg` |
| B2.6 | Loading hanya teks "Memuat…"; Pengaturan kosong saat memuat dan tanpa state error; "Versi " tanpa nomor sebelum termuat | `+page.svelte:234`, `tasks:70`, `settings:80,170` | Skeleton; state error dengan tombol coba lagi |
| B2.7 | "—" sebagai empty state ringkasan saat proses berhenti/gagal | `MeetingDetail.svelte:445,526` | Kalimat penjelas + aksi |
| B2.8 | "Muat lebih banyak" tanpa state loading → klik ganda memuat data ganda | `+page.svelte:113–121,288` | Flag `loadingMore` + disabled |
| B2.9 | Dua alur simpan di satu halaman Pengaturan; tombol "Simpan" di bagian Aplikasi ikut menyimpan Rekaman; tanpa penanda perubahan belum disimpan | `settings/+page.svelte:154` | Simpan otomatis per kontrol (dengan toast) atau bar simpan sticky saat ada perubahan |
| B2.10 | "Kirim laporan masalah" sebenarnya hanya menyimpan file | `i18n/id.ts:248–249` | "Simpan laporan masalah…" |
| B2.11 | Petunjuk "Nama Anda" menyebut label di transkrip, padahal transkrip tanpa label pembicara | `i18n/id.ts:224` | Ganti dengan kegunaan sebenarnya (PJ di tugas & ringkasan) |
| B2.12 | Istilah campur: Notulen/Ringkasan, Action Items/Tugas, Audio komputer/Audio sistem, kapitalisasi "Action Items" vs "Action item" | `i18n/id.ts` (banyak) | Glosarium satu istilah per konsep di kepala `id.ts`, lalu seragamkan |
| B2.13 | Baris widget tinggi tetap 44 px: teks auto-stop dan pesan error panjang terpotong; error tidak hilang | `recorder/+page.svelte:14, 203–225` | Ukur tinggi konten (ResizeObserver → `setSize`); error hilang otomatis |
| B2.14 | Meter widget dibedakan hanya dengan warna; label hanya di `title` pada elemen `aria-hidden` | `recorder/+page.svelte:160–167` | Ikon mic/speaker yang terlihat |
| B2.15 | `--color-ink-faint` kontras ±2,8:1 tapi dipakai untuk informasi (durasi, timestamp, "Diedit", petunjuk) | `app.css:17` | Gelapkan ke ±`#6B727D` atau batasi untuk dekorasi |
| B2.16 | Halaman detail mandiri (jendela 1120 px) tanpa tautan kembali | `routes/meeting/[id]/+page.svelte` | Tautan "Kembali ke Meeting" di header saat `!embedded` |
| B2.17 | Onboarding tanpa tombol "Kembali"; instruksi "Ucapkan sekarang…" baru muncul setelah tes dimulai | `onboarding/+page.svelte:98–193, 149–151` | Tombol kembali per langkah; instruksi tampil sebelum tombol tes |

### B3. Minor (P2)

- **Judul detail:** tidak ber-`<h1>` dan tanpa ikon pensil. `aria-label` input berisi instruksi, dan kata panjang bisa meluap (`MeetingDetail.svelte:278–294`).
- **Tab:** tanpa `aria-controls`/`aria-labelledby` dan tanpa navigasi panah (`MeetingDetail.svelte:421–440`).
- **ShortcutInput:** tanpa label. Petunjuk "Kosongkan untuk mematikan" duplikat dengan tombol "Matikan" (`ShortcutInput.svelte:35–41`).
- **Teks kosong ganda:** di layar lebar teks kosong tampil di dua panel, dan "Tekan Mulai rekam di kiri" tidak cocok saat rel ringkas (`+page.svelte:304`, `id.ts:33`).
- **Empty state Tugas:** selalu menyuruh mengubah filter. Filter `isMine` mencocokkan substring, jadi "Masayu" ikut terhitung sebagai "saya" (`tasks/+page.svelte:22,71–75`).
- **Lebar konten:** berbeda-beda (3xl / 4xl / 6xl) dan radius sudut tidak konsisten (sm/md/lg/xl/0.625/0.875 rem).
- **Warna suara di luar konteks suara:** dipakai untuk sorotan pencarian dan petunjuk clipboard (`Highlight.svelte:12`, `ClipboardKeyHint.svelte:27`).
- **Toast error:** hanya 4 detik, `role="status"` di dalam live region (dibacakan dua kali), dan jumlahnya tanpa batas.
- **CopyButton:** umpan balik ganda (label "Tersalin" + toast), dan gaya `whatsapp` yang sudah tidak dipakai masih ada (`CopyButton.svelte`, `minutes.ts:7–39`).
- **Kunci i18n mati:** `common.back`, `home.tasks`, `home.settings`, `settings.general`, `settings.about`, ikon `star`. Masih ada teks keras di `format.ts:34–45` dan `Wordmark.svelte:16`.
- **Gaya bahasa campur:** "Stop", "Edit", dan "bawaannya Groq" bercampur dengan bahasa baku. Tanda baca toast juga tidak seragam.
- **Angka konfigurasi ditulis keras di teks:** "10 menit", "2 menit" (`id.ts:109,112`), padahal nilainya dari `providers.json`.
- **Progress bar tak tentu:** penuh dan berdenyut, jadi terlihat sudah selesai (`MeetingDetail.svelte:392`).
- **Menu dan dialog:** "Buat ulang ringkasan" yang nonaktif tanpa alasan. Dialog hapus tanpa judul dan tanpa nama meeting.
- **Editor ringkasan:** tanpa teks "Menyimpan…", dan baris di-key berdasarkan indeks saat memakai `splice` (`SummaryEditor.svelte:68,89–95`).

---

## C. Keandalan & performa

Fokus saat ini: **UI dan performa aplikasi**. Item legal, rilis, dan keamanan lanjutan dipindah ke bagian **F (Ditunda)**.

### C1. Keandalan — wajib (P0)

| # | Masalah | Lokasi | Risiko | Perbaikan |
|---|---|---|---|---|
| C1.1 | **Stop yang gagal menghapus meeting beserta audionya.** Jika `recorder.stop()` error (mis. disk penuh / file terkunci), durasi diisi 0 → di bawah 5 dtk → `repo_meetings::delete` + `remove_dir_all` | `src-tauri/src/recording.rs:335–347`, `audio/recorder.rs:259–261` | Rekaman satu meeting hilang permanen tanpa jejak | Saat error, hitung durasi dari part di disk (logika `queue/recovery.rs`) atau tandai `interrupted`; hapus hanya jika stop sukses **dan** durasi < 5 dtk. Selesaikan kedua writer sebelum mengembalikan error |
| C1.2 | Gagal tulis audio saat merekam hanya dicatat ke log; pengguna tidak diberi tahu; cek disk tiap 30 dtk | `audio/recorder.rs:27–31`, `recording.rs:30, 495–499` | Audio bolong diam-diam; log membanjir | Hitung kegagalan, log sekali per channel; setelah N gagal beruntun → stop dengan `DiskFull` + peringatan di widget |
| C1.3 | Langkah 16–29 baru dikompilasi, belum pernah dijalankan; checklist uji §9 kosong; `cargo test` belum dijalankan | `PLAN-perbaikan.md:126, 139–187` | Fitur bisa gagal saat dipakai pertama kali | Jalankan `cargo test --lib` + checklist §9 di build release |
| C1.4 | Migrasi DB hanya maju, tanpa backup; `005` memakai `DROP COLUMN` | `db/mod.rs:60–70` | Data rusak tanpa titik pulih | Salin `app.sqlite` ke `app.sqlite.bak-v{n}` sebelum migrasi; tolak start jika `user_version` lebih tinggi dari yang dikenal |
| C1.5 | Build release `panic = "abort"` tanpa panic hook | `Cargo.toml:54`, `lib.rs:72` | Crash tanpa jejak di log | `std::panic::set_hook` yang menulis sinkron ke `logs/crash-*.txt` |

### C2. Performa (P1)

| # | Masalah | Lokasi | Dampak | Perbaikan |
|---|---|---|---|---|
| C2.1 | **Pemrosesan baru dimulai setelah Stop.** Meeting 1 jam = semua chunk ditranskrip setelah meeting selesai | `queue/worker.rs` (alur `preprocessing → transcribing`) | Pengguna menunggu menit-menit setelah meeting padahal waktu selama meeting menganggur | **Transkripsi bertahap selama merekam**: part 60 dtk yang sudah final langsung di-VAD dan dikirim per ±5 menit; setelah Stop hanya sisa terakhir + ringkasan → notulen siap ±30–60 dtk setelah Stop (lihat E1) |
| C2.2 | Chunk STT dikirim **satu per satu** | `queue/worker.rs:432` | Meeting panjang lambat diproses, terutama di penyedia tanpa batas ketat | Kirim 2–3 chunk paralel (mic & sistem bersamaan) dengan semaphore; untuk Groq tetap dibatasi rate limiter |
| C2.3 | Audio diunggah sebagai WAV PCM16 (±1,9 MB/menit) | `preprocess/chunker.rs` | Unggahan lambat di koneksi rumahan/seluler; timeout | Kompres ke FLAC (lossless, ±50%) atau Opus/OGG (±10× lebih kecil, didukung Whisper) sebelum unggah |
| C2.4 | Timeout STT tetap 120 dtk | `stt/openai.rs:12` | Chunk besar di koneksi 1 Mbps selalu timeout lalu `waiting_network` | Timeout proporsional ukuran file atau connect/read timeout terpisah (berkurang drastis bila C2.3 dikerjakan) |
| C2.5 | `playback.wav` dibuat saat pertama kali timestamp diklik (mencampur seluruh audio) | `playback.rs:15–46` | Klik pertama di meeting 1 jam menunggu beberapa detik; berkas ±115 MB/jam tambahan | Buat di latar belakang setelah job selesai, atau putar langsung dari part per channel tanpa file campuran; hapus otomatis setelah retensi |
| C2.6 | Pemakaian disk per meeting tanpa anggaran (±115 MB/jam/channel + playback + chunk upload); ambang mulai rekam 1 GB | `recording.rs:25`, `config/settings.rs:50` | Disk penuh di tengah pemrosesan | Hapus folder `upload/` setelah transkrip selesai; tampilkan ukuran `recordings/` di Pengaturan + tombol bersihkan; estimasi ruang sebelum mulai rekam |
| C2.7 | Meeting `interrupted` tidak pernah disapu retensi | `db/repo_meetings.rs:245–248` | Audio menumpuk selamanya | Sertakan `interrupted` dalam penyapuan (batas lebih panjang) |
| C2.8 | Operasi blocking (`remove_dir_all`, kunci DB) langsung di task async worker | `queue/worker.rs:149–170` | Tokio worker tertahan saat menghapus folder besar | Bungkus `spawn_blocking` |
| C2.9 | RAM/CPU saat merekam 60 menit belum diukur; jendela main terbuka = 345 MB (WebView2) | `CLAUDE.md` tabel NFR | Target NFR (< 150 MB, < 5% CPU) belum terbukti | Ukur (`Get-Process` tiap 10 dtk ke CSV); pastikan jendela main tetap dihancurkan saat di tray; profil render transkrip panjang |
| C2.10 | Daftar meeting & transkrip dirender penuh (hanya `content-visibility` di transkrip) | `src/routes/+page.svelte`, `MeetingDetail.svelte` | Ratusan meeting / ribuan kalimat → scroll berat | Virtualisasi daftar (render baris terlihat saja); paginasi transkrip saat > 1.000 segment |
| C2.11 | Rekaman dipotong di 4 jam hanya dengan notifikasi | `providers.default.json:18`, `recording.rs:490–491` | Meeting panjang terpotong tanpa peringatan | Peringatan 10 menit sebelumnya + lanjut otomatis sebagai bagian baru |
| C2.12 | Updater bisa me-restart aplikasi saat rekaman baru dimulai selama unduhan | `updater.rs:61–75` | Rekaman terputus | Cek ulang `is_recording()` tepat sebelum `restart()` |
| C2.13 | Log harian tanpa batas ukuran | `lib.rs:66–70` | Satu hari bermasalah bisa memenuhi disk | Rotasi per jam / per ukuran; rate-limit error berulang |

### C3. Kualitas kode (P2)

- **CI.** Belum ada. Tambahkan GitHub Actions `windows-latest` yang menjalankan clippy, `cargo test --lib`, `npm run check`, dan uji migrasi in-memory.
- **Cakupan test.** 24 unit test belum menyentuh `recording.rs`, state machine worker, recovery, rate limiter, atau migrasi.
- **Deteksi meeting bisa salah picu.** Browser apa pun yang memakai mic (mis. voice search) memicu "Meeting terdeteksi". Saat mic dilepas, tawaran auto-stop muncul setelah 30 + 60 dtk. Struktur registry Windows 10 juga belum diverifikasi.
- **Berkas liar di working tree.** `PRD.md  2.md` dan `feedback.md` belum dirapikan.

### C4. Sudah memadai (diperiksa)

- **Penanganan mutex:** semua `lock()` menangani mutex poisoned.
- **`unwrap`/`expect`:** hanya ada pada invarian.
- **API key:** hanya di keyring dan tidak pernah masuk log.
- **Tulis file dari UI:** ekspor dan laporan membuka dialog simpan di Rust.
- **CSP:** ketat.
- **Recovery crash:** header WAV diperbaiki dan meeting ditandai `interrupted`.
- **Offline:** ditangani lewat `waiting_network`.
- **Rate limiter Groq:** hanya aktif untuk Groq (`worker.rs:348`).

---

## D. Urutan pengerjaan yang disarankan

| Tahap | Isi | Hasil |
|---|---|---|
| **1. Keselamatan data** (±1–2 hari) | C1.1, C1.2, C1.4, C1.5 | Rekaman tidak bisa hilang diam-diam |
| **2. Fondasi visual** (±3–4 hari) | Komponen bersama `Menu`, `ConfirmDialog`, `AudioPlayer`, `Select`/`Radio` kustom; 6 ikon baru; token warna, ukuran, radius (B1.1–B1.6, B2.1–B2.5); A1 format durasi; A2–A3 bidang & tinta | Semua layar terasa satu produk; tidak ada lagi kontrol bawaan browser |
| **3. Poles layar** (±3–4 hari) | A4–A15, B1.7–B1.10, B2.6–B2.17 | Detail terasa dipoles; istilah konsisten; pesan error manusiawi |
| **4. Performa** (±3–5 hari) | C2.1 transkripsi bertahap, C2.2 paralel, C2.3 kompresi, C2.4–C2.8, C2.10 | Notulen siap ±1 menit setelah Stop; aplikasi tetap ringan di data besar |
| **5. Inovasi gelombang 1** (±1–2 minggu) | E1–E5 (lihat bagian E) | Notetaker terasa jauh lebih berguna daripada sekadar perekam + ringkasan |
| **6. Verifikasi** (±2–3 hari) | C1.3 checklist §9 + `cargo test`, C2.9 ukur NFR, meeting 1 & 4 jam, Windows 10 | Bukti semuanya berjalan, bukan hanya terkompilasi |

---

## E. Inovasi agar AI Notetaker benar-benar bermanfaat

Fitur yang sudah ada dan tidak diulang di sini: rekam tanpa bot, deteksi meeting, shortcut, notulen otomatis, edit, salin per tab, ekspor, pencarian, halaman Tugas, putar audio dari kalimat, dan ganti penyedia AI. Hal yang sudah dihapus atas keputusan pemilik juga tidak diusulkan ulang: popup consent, template ringkasan, label pembicara di transkrip, dan tampilan kuota.

Fokus usulan ini: **(1) notulen lebih akurat, (2) notulen lebih cepat, (3) notulen ditindaklanjuti**.

### E1. Gelombang 1 — dampak terbesar, biaya sedang

| # | Inovasi | Kenapa bermanfaat | Catatan implementasi |
|---|---|---|---|
| **E1** | **Notulen siap hampir seketika** (transkripsi bertahap selama merekam) | Waktu menunggu setelah Stop paling terasa oleh pengguna; target "notulen siap sebelum Anda menutup Zoom" | Lihat C2.1. Part 60 dtk final → VAD → chunk ±5 menit dikirim selama merekam; setelah Stop hanya sisa + ringkasan. Widget bisa menampilkan "Transkrip berjalan" |
| **E2** | **Kosakata khusus (glosarium)**: nama orang, nama produk, istilah internal, singkatan | Whisper sering salah menulis nama dan istilah campuran Indonesia–Inggris ("Rina" → "Rena", "QA" → "kyu ei"); ini sumber ketidakpercayaan terbesar pada notulen | Parameter `prompt` STT **sudah ada tapi selalu `None`** (`queue/worker.rs:472`). Isi dari daftar di Pengaturan (maks ±800 karakter) + otomatis dari nama PJ & judul meeting sebelumnya. Juga dipakai di prompt LLM untuk ejaan nama |
| **E3** | **Sumber untuk setiap poin notulen** | Setiap keputusan dan action item diberi waktu sumbernya, jadi pengguna bisa langsung mengecek. Klik membuka transkrip di detik itu dan memutar audionya. Kepercayaan pada ringkasan AI naik drastis | Baris transkrip ke LLM sudah berformat `[HH:MM:SS]`; minta field `sumber: "00:12:31"` per keputusan/action item; tampilkan chip waktu kecil; validasi waktu ada di rentang meeting |
| **E4** | **Draf pesan tindak lanjut** satu klik ("Kirim ringkasan ke peserta") | Setelah meeting, pekerjaan berikutnya hampir selalu menulis pesan ringkasan ke tim/klien; ini menghemat 10–15 menit per meeting | Satu panggilan LLM dari notulen (bukan transkrip) → draf formal Bahasa Indonesia (opsi Inggris), siap disalin sebagai teks atau email (`mailto:`). Tab baru atau tombol di toolbar |
| **E5** | **Penanda momen penting saat meeting** (tombol bintang di widget + shortcut, mis. `Ctrl+Alt+B`) | Pengguna tahu momen yang penting ("ini keputusan klien"); AI tidak selalu tahu | Simpan timestamp; kirim ke LLM sebagai "momen yang ditandai pengguna — wajib dibahas"; tampilkan sebagai penanda di transkrip dan bagian "Momen ditandai" di ringkasan |

### E2. Gelombang 2 — menjadikan notulen alat kerja

| # | Inovasi | Kenapa bermanfaat | Catatan implementasi |
|---|---|---|---|
| E6 | **Tanya meeting ini** (chat di samping notulen) | "Berapa harga yang disebut klien?", "Siapa yang setuju soal tenggat?" dijawab dari transkrip, dengan waktu sumber | Satu meeting muat konteks LLM (atau map-reduce untuk meeting panjang); jawaban wajib menyertakan `[HH:MM:SS]` |
| E7 | **Tanya semua meeting** ("Apa keputusan soal anggaran bulan ini?") | Memori tim; fitur paling dicari di notetaker | RAG sederhana: FTS5 yang sudah ada mengambil kandidat potongan → LLM menjawab dengan tautan ke meeting & waktu |
| E8 | **Rangkaian meeting & tindak lanjut otomatis** | Meeting rutin (standup, sync mingguan) saling terkait; pengguna perlu tahu action item minggu lalu sudah dibahas/selesai atau belum | Kenali meeting berulang dari kemiripan judul + hari/jam; sebelum merekam tampilkan "Tugas terbuka dari meeting sebelumnya"; di ringkasan tambah bagian "Status tindak lanjut" (LLM mencocokkan tugas lama dengan transkrip baru) |
| E9 | **Pengingat tenggat action item** | Action item yang tidak diingatkan biasanya terlupa | Tenggat sudah berisi tanggal `YYYY-MM-DD` dalam kurung; parse → notifikasi Windows pagi hari tenggat; opsi ekspor `.ics` ke kalender |
| E10 | **Salin sebagai teks berformat** (HTML) | Tempel ke email/Google Docs/Word langsung rapi (judul tebal, daftar berpoin) tanpa merapikan ulang | `navigator.clipboard.write` dengan `text/html` + `text/plain` dari formatter yang sudah ada |
| E11 | **Ringkasan mingguan** ("Minggu ini: 6 meeting, 9 keputusan, 14 tugas terbuka") | Gambaran besar untuk manajer; bahan laporan mingguan | Agregasi lokal dari DB + satu panggilan LLM opsional; halaman atau kartu di Beranda tiap Senin |

### E3. Gelombang 3 — pembeda produk

| # | Inovasi | Kenapa bermanfaat | Catatan |
|---|---|---|---|
| E12 | **Indikator kualitas transkrip** | Bagian dengan suara buruk diberi tanda halus (dari `avg_logprob`/`no_speech_prob` yang sudah disimpan), jadi pengguna tahu bagian mana yang perlu dicek | Data sudah ada di `transcript_segments`; tampilkan garis bawah putus-putus + tooltip "kualitas audio rendah" |
| E13 | **Statistik bicara** (porsi bicara Saya vs peserta lain, durasi hening) | Berguna untuk sales call, interview, dan coaching ("Anda bicara 70% waktu") | Bisa dihitung dari dua channel tanpa menampilkan label pembicara di transkrip; tampilkan opsional di header |
| E14 | **Transkripsi lokal (offline)** dengan whisper.cpp | Tanpa kuota, tanpa internet, audio tidak keluar dari komputer; menarik untuk kantor dengan kebijakan data ketat | Unduhan model opsional (±150 MB–1,5 GB); butuh evaluasi CPU/RAM; jadikan penyedia "Lokal" di Layanan AI |
| E15 | **Integrasi kalender** (Outlook/Google) | Judul meeting dan daftar peserta otomatis; nama peserta masuk glosarium (E2) | OAuth; mulai dari Outlook (umum di kantor Indonesia) |
| E16 | **Kirim action item ke alat lain** (Microsoft To Do, Google Tasks, Notion, Trello) | Action item langsung jadi tugas di alat yang dipakai tim | Mulai dari satu integrasi paling diminta penguji |
| E17 | **Lewati hening saat memutar audio** + kecepatan 1,5×/2× | Mengecek kutipan jadi lebih cepat | Region VAD sudah dihitung saat praproses; simpan dan pakai untuk melompati hening |

### E4. Ukuran keberhasilan yang diusulkan

| Metrik | Target |
|---|---|
| Waktu dari Stop sampai notulen siap (meeting 1 jam) | < 1 menit (setelah E1) |
| Kesalahan penulisan nama/istilah di glosarium | ↓ 80% (setelah E2) |
| Poin notulen yang punya sumber waktu | 100% (setelah E3) |
| Meeting yang notulennya dibagikan (salin/ekspor/draf pesan) | > 60% |
| Action item yang ditandai selesai dalam 14 hari | naik dari waktu ke waktu (setelah E8–E9) |

---

## F. Ditunda (atas keputusan pemilik, 8 Okt 2026)

Item berikut tetap tercatat supaya tidak terlupa, tetapi **tidak dikerjakan sekarang**. Fokusnya saat ini UI dan performa.

| # | Item | Lokasi | Ringkas |
|---|---|---|---|
| F1 | Versi masih 0.1.0 + changelog | `package.json:3`, `Cargo.toml:3`, `tauri.conf.json:5` | Naikkan versi dari satu skrip saat siap rilis |
| F2 | Installer tidak ditandatangani (SmartScreen) | `tauri.conf.json:49–56` | Sertifikat OV/EV atau Azure Trusted Signing |
| F3 | Auto-update nonaktif tanpa env build | `tauri.conf.json:60–61`, `updater.rs:17–18` | Skrip rilis dengan kunci updater wajib |
| F4 | Kebijakan privasi & syarat penggunaan; arti kolom `consent_at` | `i18n/id.ts:263`, `recording.rs:181` | Alur data, penyedia, kewajiban pengguna meminta izin |
| F5 | Pemberitahuan lisensi pihak ketiga + lisensi aplikasi | — | `cargo about` + `license-checker` → Tentang → Lisensi |
| F6 | Data (termasuk WAV) di profil Roaming | `lib.rs:80` | Pindah ke `app_local_data_dir()` |
| F7 | Uninstall meninggalkan autostart & API key | `desktop.rs:50–51`, `secrets.rs:8` | NSIS `installerHooks` |
| F8 | Enkripsi DB/transkrip, FTS contentless | `db/migrations/003_fts.sql` | SQLCipher dengan key di Credential Manager |
| F9 | Alamat API kustom `http://` ke host mana pun | `ai.rs:170–172` | Batasi `http://` ke localhost |
| F10 | Capability Tauri terlalu luas | `capabilities/default.json:4–16` | Persempit `opener`, hapus `dialog:default`, capability terpisah untuk widget |
| F11 | Metadata bundle (publisher, copyright), WebView2 `embedBootstrapper`, data sistem di laporan masalah | `tauri.conf.json` | Saat menyiapkan rilis publik |
