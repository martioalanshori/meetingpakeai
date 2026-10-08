# Feedback 3 — Lebih Berguna & Lebih Mulus sebagai AI Notetaker

**Tanggal:** 8 Oktober 2026 · **Basis:** commit `94b9ead` (setelah langkah 30–44)

**Cara analisis:** perjalanan pengguna ditelusuri dari sebelum meeting sampai minggu-minggu sesudahnya. Di tiap tahap dicek apa yang sudah dikerjakan aplikasi, di mana pengguna masih harus berpikir atau menunggu, dan di mana nilai notulen berhenti. Semua klaim "belum ada" sudah dicek ke kode: daftar command di `src-tauri/src/lib.rs`, prompt di `llm/prompts.rs`, dan UI di `src/`.

**Prioritas:**
- **P0:** dampak terbesar pada "berguna" atau "mulus".
- **P1:** penting.
- **P2:** penyempurnaan atau pembeda produk.

**Tidak diusulkan ulang** (keputusan pemilik):
- popup consent
- template ringkasan
- label/nama pembicara di transkrip
- tampilan kuota
- salin format WhatsApp
- filter "tugas saya"

Item legal dan rilis tetap ditunda (lihat `feedback2.md` §F).

---

## 0. Ringkasan

Rekam → notulen sekarang sudah cepat dan rapi. Keputusan dan tugas punya sumber waktu, ada glosarium, pesan tindak lanjut, dan penanda momen. Celah yang tersisa bukan di kualitas notulen satu meeting, tapi di **empat sambungan** perjalanan pengguna.

1. **Masuk ke aplikasi masih terbatas pada rekaman langsung.** Rekaman yang sudah ada (Zoom cloud, voice note, rekaman HP) tidak bisa diproses sama sekali. Judul meeting baru diketahui setelah AI selesai.
2. **Selama meeting aplikasi "buta".** Transkrip bertahap sudah dibuat setiap 5 menit, tapi tidak bisa dilihat. Pengguna juga tidak punya tempat menulis catatan sendiri.
3. **Sesudah meeting notulen hanya bisa dibaca, belum bisa ditanya.** Pengguna belum bisa bertanya ke notulen, memperbaiki transkrip, atau meminta versi lain.
4. **Antar-meeting belum ada memori.** Tugas tidak punya pengingat. Meeting rutin tidak tahu tugas minggu lalu. Pengguna belum bisa bertanya lintas meeting.

Dua hal mendasar ikut terbawa ke semua tahap di atas.

5. **Meeting bisa lewat tanpa terekam.** Deteksi meeting hanya menawarkan lewat notifikasi, dan notifikasi itu mudah tenggelam saat *Do Not Disturb* aktif (bagian A0).
6. **Transkrip kehilangan kalimat.** Filter halusinasi membuang segment jika *salah satu* metrik ragu, padahal Whisper memakai aturan *dan*. Akibatnya kalimat yang benar-benar diucapkan ikut hilang (bagian G).

---

## A. Masuk ke aplikasi — menangkap semua meeting tanpa usaha

### A0. Tidak ada meeting yang terlewat (deteksi & rekam otomatis)

**Kondisi sekarang** (`meeting_watch.rs`, `bridge.rs:253–266`):
- Tiap 10 dtk aplikasi membaca registry privasi Windows (ConsentStore) untuk melihat aplikasi mana yang sedang memakai mic.
- Zoom dan Teams dikenali dengan namanya. Google Meet hanya dikenali sebagai **"browser"** (Chrome/Edge/Firefox/Brave).
- Saat terdeteksi:
  - Jendela main sedang fokus → banner "Meeting terdeteksi. Mulai rekam?".
  - Selain itu → notifikasi Windows.
- Tawaran hanya muncul **sekali per sesi mic**.
- Tidak ada perekaman otomatis.
- Deteksi hanya berjalan bila aplikasi hidup di tray, onboarding selesai, dan setting "Deteksi meeting" menyala.

**Kenapa meeting masih bisa terlewat:**
1. Windows 11 otomatis menyalakan *Do Not Disturb* saat berbagi layar atau saat aplikasi layar penuh. Notifikasi masuk diam-diam ke Notification Center.
2. Tawaran yang diabaikan tidak muncul lagi sampai sesi mic berikutnya.
3. "Browser" terlalu umum: voice note WhatsApp Web dan tes mic di browser ikut memicu, sehingga merekam otomatis untuk browser berisiko.
4. Jika aplikasi ditutup lewat "Keluar" atau autostart mati, deteksi tidak berjalan sama sekali, dan pengguna tidak sadar.

| # | P | Usulan | Catatan implementasi |
|---|---|---|---|
| **A0.1** | **P0** | **Tawaran lewat widget, bukan hanya notifikasi.** Widget kecil muncul di atas semua jendela: "Zoom terdeteksi · Rekam · Abaikan". Widget tetap terlihat walau *Do Not Disturb* menyala. Hilang sendiri setelah 2 menit tanpa respons. | Widget `recorder` mode "tawaran" (jendela dibuat on-demand seperti sekarang, `always_on_top`, tanpa fokus). Notifikasi tetap dikirim sebagai cadangan. |
| **A0.2** | **P0** | **Rekam otomatis dengan hitung mundur.** Setting baru "Rekam otomatis saat meeting terdeteksi" (default **mati**, demi privasi peserta). Zoom/Teams/Meet → widget "Merekam dalam 10 dtk · Batal" lalu langsung merekam. Browser umum → tetap ditanya (A0.1). | Pakai jalur `start_recording_in_background`. Batal → sesi mic itu tidak ditawari lagi. Teks setting mengingatkan kewajiban memberi tahu peserta (sama dengan teks privasi onboarding). |
| **A0.3** | P1 | **Pengingat ulang.** Jika tawaran diabaikan dan aplikasi meeting masih memakai mic setelah 3 menit, tawarkan sekali lagi. | `offered` di `meeting_watch.rs` menyimpan waktu tawaran, bukan hanya kunci sesi. |
| **A0.4** | P1 | **Kenali Google Meet secara spesifik.** Judul jendela browser yang memuat "Meet –" / `meet.google.com` diperlakukan seperti Zoom/Teams: boleh direkam otomatis dan memberi judul meeting (A2). | `EnumWindows` + `GetWindowTextW` untuk proses browser yang sedang memakai mic; pola judul di `providers.json` → `meeting_detection.title_patterns` agar mudah ditambah (mis. Webex, Slack huddle). |
| A0.5 | P1 | **Status deteksi terlihat.** Pengaturan → Rekaman menampilkan status "Deteksi meeting aktif · Autostart: mati (meeting bisa terlewat jika aplikasi tidak berjalan)" + tombol nyalakan. Tooltip tray: "Siap merekam — deteksi meeting aktif". | Membaca `autostart` + `meetingDetection`; tanpa command baru selain yang ada. |

### A1–A4. Sumber & judul meeting

| # | P | Masalah | Usulan | Catatan implementasi |
|---|---|---|---|---|
| **A1** | **P0** | **Tidak bisa memproses file audio/video yang sudah ada.** Tidak ada command impor (daftar command di `lib.rs`). Meeting tatap muka yang direkam di HP, rekaman cloud Zoom/Teams, dan voice note panjang tidak bisa jadi notulen. | Tombol "Impor rekaman" di Beranda + seret-lepas file ke jendela. Format: mp3, m4a, mp4, wav, ogg, webm. File diproses lewat antrean yang sama dan muncul seperti meeting biasa. | Decode dengan crate `symphonia` (Rust murni, tanpa ffmpeg): mp3/aac/m4a/mp4/ogg/flac/wav → resample 16 kHz mono (`rubato`) → ditulis sebagai part channel `mic`, lalu `queued`. Tanggal meeting = waktu modifikasi file (bisa diubah). |
| **A2** | P1 | **Judul awal tidak bermakna.** "Meeting 8 Okt 2026 09.00" (`recording.rs:181`) bertahan sampai ringkasan selesai. Di daftar, rekaman yang sedang berjalan dan yang sedang diproses sulit dibedakan. | Ambil judul dari jendela aplikasi meeting saat mulai rekam, mis. judul jendela Teams/Zoom "Sync Mingguan – Tim Produk". Jika tidak ada, pakai nama aplikasi: "Meeting Zoom 09.00". | `EnumWindows` + `GetWindowTextW` untuk proses `source_app` yang terdeteksi `meeting_detect.rs`; buang akhiran " \| Microsoft Teams" dsb. Tetap `title_edited = 0` agar judul AI bisa menggantikan bila lebih baik. |
| A4 | P2 | **Menu tray hanya punya Buka / Mulai rekam / Keluar** (`lib.rs:222–224`). | Tambah "Notulen terakhir" (langsung membuka meeting terbaru) dan "Tugas terbuka (n)". | Menu tray dibangun ulang saat `meeting://updated`. |

---

## B. Selama meeting — aplikasi ikut "hadir"

| # | P | Masalah | Usulan | Catatan implementasi |
|---|---|---|---|---|
| **B1** | **P0** | **Transkrip bertahap tidak terlihat.** Langkah 37 sudah mentranskrip chunk setiap ±5 menit selama merekam. Tapi segment baru ditulis ke `transcript_segments` di tahap merging setelah Stop, jadi tab Transkrip kosong selama merekam. | Saat meeting `recording` dibuka: tampilkan **"Transkrip sementara"** dari chunk yang sudah selesai, diperbarui saat event `recording://live`. Ditambah tombol **"Ringkas sejauh ini"** untuk yang telat bergabung atau sempat tidak fokus. | Command tambahan `get_live_transcript(id)`: baca `response_json` chunk `done` + offset map → waktu meeting (fungsi yang sama dengan merging, tanpa dedup). "Ringkas sejauh ini" = satu panggilan LLM dengan prompt CHUNK, tidak disimpan. |
| **B2** | **P0** | **Tidak ada tempat untuk catatan sendiri.** Pengguna sering menulis poin singkat ("klien minta diskon 10%", "follow up legal"). Saat ini catatan itu harus ditulis di aplikasi lain dan tidak ikut membentuk notulen. | **Catatan saya**: panel teks saat merekam (di jendela main dan tombol "Catatan" di widget yang membuka jendela catatan kecil). Catatan dikirim ke LLM sebagai prioritas ("pengguna mencatat hal berikut; wajib tercermin dan dikembangkan dari transkrip"). Catatan juga tampil di notulen sebagai bagian sendiri. | Tabel `meeting_notes(meeting_id, text, updated_at)`. Tiap baris diberi waktu otomatis (`elapsed_ms` saat baris ditulis) sehingga sejalan dengan bookmark (langkah 44). Simpan otomatis tiap 2 dtk. |
| B3 | P1 | **Lupa melanjutkan setelah jeda.** Hening saat pause tidak dihitung untuk auto-stop (`recording.rs`), jadi rekaman bisa dijeda berjam-jam tanpa pengingat. | Setelah 10 menit dijeda: notifikasi + baris widget "Rekaman dijeda 10 menit. Lanjutkan atau hentikan?". | Monitor sudah punya pola peringatan (`EV_AUTO_STOP_WARNING`). |
| B4 | P2 | **Widget tidak menunjukkan meeting apa yang direkam.** | Baris kecil judul (dari A2) di bawah timer, muncul saat hover. | — |

---

## C. Sesudah meeting — notulen yang bisa diajak kerja

| # | P | Masalah | Usulan | Catatan implementasi |
|---|---|---|---|---|
| **C1** | **P0** | **Tidak bisa bertanya ke notulen.** Contoh pertanyaan: "Berapa harga yang disebut klien?" atau "Kenapa tenggat dimundurkan?". Saat ini pengguna harus mencari sendiri di transkrip. (E6 di `feedback2.md`, belum dikerjakan.) | Tab atau panel **"Tanya"** di detail. Jawaban pendek wajib menyertakan chip waktu sumber (memakai `sourceChip` dan `jumpTo` dari langkah 42). Ada saran pertanyaan otomatis ("Apa risiko yang dibahas?"). | Meeting ≤ ±1 jam muat satu konteks. Lebih panjang → ambil potongan relevan lewat FTS (`search_index` sudah ada) ±2 menit di sekitar hit. Riwayat tanya-jawab disimpan per meeting. |
| **C2** | P1 | **Format notulen belum memuat hal yang belum selesai.** FINAL hanya `judul, ringkasan, keputusan, action_items, topik` (`prompts.rs:23`). Pertanyaan yang belum terjawab dan risiko yang muncul hilang. Pembaca sibuk juga butuh intisari 3 poin. | Tambah ke format standar (bukan template): **"Intisari"** (3 poin, di atas ringkasan) dan **"Belum diputuskan / pertanyaan terbuka"** (dengan sumber waktu). | Kolom `summaries.key_points`, `summaries.open_questions` (JSON, pola sama dengan `decision_sources`). Ikut di Salin, ekspor, cetak, dan pesan tindak lanjut. |
| **C3** | P1 | **Transkrip tidak bisa diperbaiki.** Tidak ada command edit segment. Nama yang salah dengar tetap salah di transkrip, pencarian, dan "Tanya". | Klik-ubah teks per baris. Saat sebuah kata diganti, tawarkan **"Ganti semua 'Rena' → 'Rina' dan simpan ke glosarium"**. Setelah itu tawarkan buat ulang ringkasan. | Command `update_segment(id, text)` dan `replace_in_transcript(meetingId, from, to)` → `reindex`. Glosarium langsung terisi dari koreksi nyata (memperkuat langkah 41). |
| C4 | P1 | **Buat ulang ringkasan tidak bisa diarahkan.** `regenerate_summary` hanya menerima `id`. | Kolom instruksi opsional saat Buat ulang: "lebih singkat", "fokus ke anggaran", "tulis dalam Bahasa Inggris". | Instruksi ditambahkan sebagai baris di prompt sistem (pola sama dengan glosarium); tidak disimpan sebagai template. |
| C5 | P1 | **Notulen selalu berbahasa Indonesia** (aturan 1 di `prompts.rs`). Meeting dengan klien asing butuh notulen berbahasa Inggris. | Setting "Bahasa notulen: Indonesia / Inggris / ikuti bahasa meeting" + pilihan per meeting di menu Lainnya. | Satu variabel di prompt sistem; pesan tindak lanjut sudah punya pilihan bahasa (langkah 43). |
| C6 | P2 | **Rekaman "(lanjutan)" dan meeting yang direkam terpisah tidak bisa digabung.** | Pilih 2 meeting → "Gabungkan" → satu transkrip dan satu notulen. | Segment digeser `duration_ms` meeting pertama, lalu `summarizing` ulang. |
| C7 | P2 | **Potongan sensitif tidak bisa dihapus.** Contoh: obrolan pribadi sebelum meeting mulai, atau angka rahasia. | Pilih rentang di transkrip → "Hapus bagian ini" (segment + audio dinolkan) → ringkasan dibuat ulang. | Tulis nol ke part WAV pada rentang itu; hapus segment; `reindex`. |

---

## D. Antar-meeting — memori dan tindak lanjut

| # | P | Masalah | Usulan | Catatan implementasi |
|---|---|---|---|---|
| **D1** | **P0** | **Tidak bisa bertanya lintas meeting.** Contoh: "Apa keputusan soal anggaran bulan ini?" (E7). Ini fitur yang membuat notetaker jadi memori tim. | Kolom "Tanya semua meeting" di atas Beranda (menyatu dengan pencarian). Jawaban berisi tautan ke meeting + chip waktu. | RAG sederhana: FTS5 → 10–20 potongan teratas (transkrip ±1 menit, keputusan, tugas) → satu panggilan LLM. Tanpa embedding dulu. |
| **D2** | P1 | **Tenggat tugas hanya teks bebas.** Tidak ada pengingat. Halaman Tugas hanya bisa mencentang: PJ/tenggat tidak bisa diubah dan tugas tidak bisa ditambah. | Di halaman Tugas: ubah PJ & tenggat langsung (pemilih tanggal), tambah tugas manual, **notifikasi pagi hari pada tanggal tenggat** dan saat lewat tenggat, opsi "Tambahkan ke kalender" (.ics). | Kolom baru `action_items.due_date` (ISO) diisi dari tanggal dalam kurung yang sudah dihasilkan LLM; teks `due` tetap untuk tampilan. Worker menyapu pengingat sekali sehari (pola retensi). |
| **D3** | P1 | **Meeting rutin tidak saling kenal.** Standup dan sync mingguan mengulang topik, tapi tugas minggu lalu tidak muncul. | Saat merekam meeting yang mirip sebelumnya (judul/aplikasi/hari & jam sama): widget menawarkan "3 tugas terbuka dari Sync minggu lalu". Notulen mendapat bagian **"Status tindak lanjut"**: LLM mencocokkan tugas lama dengan transkrip baru (selesai / dibahas / belum disebut). | Kemiripan judul (`strsim` sudah dependensi) + `source_app` + jendela hari/jam ±1 jam. Tugas yang dinyatakan selesai bisa dicentang otomatis setelah konfirmasi. |
| D4 | P2 | **Tidak ada pengelompokan selain tanggal.** Pengguna dengan banyak klien/proyek bergantung pada pencarian. | Label proyek/klien per meeting (disarankan otomatis dari judul dan glosarium); filter di Beranda dan Tugas. | Tabel `tags`, `meeting_tags`. |
| D5 | P2 | **Tidak ada gambaran mingguan.** | Kartu Senin di Beranda: "Minggu lalu: 6 meeting, 9 keputusan, 14 tugas terbuka (3 lewat tenggat)" + ringkasan satu paragraf (E11). | Agregasi lokal + satu panggilan LLM opsional. |
| D6 | P2 | **Tugas tidak bisa dikirim ke alat lain** (E16) dan **kalender belum terhubung** (E15). | Mulai dari ekspor .ics (D2). Integrasi Microsoft To Do / Outlook Calendar setelah ada permintaan penguji. | OAuth; sesudah D2 dan D3. |

---

## E. Kepercayaan & kualitas

| # | P | Masalah | Usulan | Catatan implementasi |
|---|---|---|---|---|
| E1 | P1 | **Bagian transkrip yang ragu tidak terlihat.** `avg_logprob` / `no_speech_prob` sudah tersimpan tapi tidak dipakai di UI (E12). | Garis bawah putus-putus halus + tooltip "Audio kurang jelas, cek dengan memutar". Ditambah satu baris di header: "Kualitas audio: baik / cukup / buruk". | Ambang dari distribusi `avg_logprob` per meeting. |
| E2 | P1 | **Poin tanpa sumber tidak dibedakan.** Langkah 42 memberi chip waktu, tapi poin yang `sumber`-nya null (kemungkinan tebakan LLM) tampil sama meyakinkannya. | Poin tanpa sumber diberi penanda halus "tanpa rujukan". Opsional: satu panggilan LLM pemeriksa yang mengecek tiap keputusan terhadap transkrip ±2 menit. | Dihitung dari `decision_sources` / `source_ms` yang sudah ada. |
| E3 | P2 | **Butuh internet dan kuota penyedia.** Kantor dengan kebijakan data ketat tidak bisa memakai layanan cloud (E14). | Penyedia "Lokal" untuk transkrip (whisper.cpp, model unduhan opsional). | Evaluasi CPU/RAM dulu; preset Kustom (Ollama/faster-whisper-server) sudah ada sebagai jalan sementara. |

---

## F. Gesekan kecil yang membuat terasa kurang mulus

| # | P | Temuan | Usulan |
|---|---|---|---|
| F1 | P1 | Setelah Stop, widget langsung hilang. Pengguna tidak tahu kapan notulen siap kecuali membuka aplikasi atau menunggu notifikasi. | Widget berubah jadi kartu kecil "Menyusun notulen… ±1 menit" lalu "Notulen siap · Buka" (tutup otomatis setelah 15 dtk), bukan langsung hilang. |
| F2 | P1 | Hasil "Tanya", pesan tindak lanjut, dan Salin tersebar di tombol berbeda. | Satu tombol **"Bagikan"** di header detail: Salin notulen, Pesan tindak lanjut, Ekspor, Cetak. |
| F3 | P2 | Meeting < 20 kata menjadi notulen "kosong" (`worker.rs:761`) tanpa penjelasan langkah berikutnya. | Teks kosong yang menyarankan cek mikrofon/audio komputer + tombol putar audio. |
| F4 | P2 | Momen ditandai (langkah 44) hanya bisa dibuat saat merekam. | Klik kanan / tombol di baris transkrip → "Tandai momen ini" sesudah meeting. |

---

## G. Akurasi transkrip — kurangi salah kata, kalimat hilang, dan typo

**Kondisi sekarang:**
- Audio 16 kHz mono masuk ke VAD webrtc mode Aggressive (padding 300 ms, gabung jika jarak < 1 dtk; `preprocess/vad.rs`).
- Potongan ±5 menit diunggah sebagai FLAC lossless ke `whisper-large-v3-turbo` dengan `temperature 0` dan `language=id` (kecuali "auto").
- Prompt = daftar glosarium dipisah koma (langkah 41).
- Filter halusinasi dijalankan, lalu penghapus duplikat gema.

| # | P | Temuan | Usulan | Catatan implementasi |
|---|---|---|---|---|
| **G1** | **P0** | **Filter membuang kalimat yang benar-benar diucapkan.** `filter.rs::is_hallucination` membuang segment jika `no_speech_prob > 0.6` **atau** `avg_logprob < -1.0` **atau** `compression_ratio > 2.4`. Whisper sendiri menganggap hening hanya bila `no_speech_prob` tinggi **dan** `avg_logprob` rendah; `compression_ratio` tinggi adalah tanda pengulangan yang harus dicoba ulang. Satu segment bisa berisi beberapa kalimat (±30 dtk), jadi suara pelan, aksen, atau istilah asing membuat kalimat utuh hilang dari transkrip dan notulen. | Buang hanya jika `no_speech_prob > 0.6` **dan** `avg_logprob < -1.0`. `compression_ratio > 2.4` → transkrip ulang potongan itu (G6) atau buang hanya bagian teks yang berulang. Segment ragu lainnya disimpan dan ditandai (E1). | Perubahan beberapa baris + penyesuaian `providers.json`. Sebelum dan sesudahnya ukur dengan G8. |
| **G2** | **P0** | **Prompt Whisper berupa daftar kata.** Whisper meniru gaya prompt (ejaan, kapital, tanda baca); daftar dipisah koma tidak memberi contoh kalimat yang baik dan bisa memancing istilah glosarium muncul di bagian hening. | Prompt sebagai kalimat Indonesia rapi: *"Rapat Sync Mingguan. Hadir: Putri Ramadhani, Budi. Membahas OKR dan sprint review."* (judul meeting + nama + istilah). | `stt_opts` di worker; judul tersedia setelah A2. |
| G3 | P1 | **Tidak ada konteks antar-potongan.** Tiap potongan ditranskrip tanpa tahu kalimat sebelumnya, sehingga ejaan nama dan istilah bisa berbeda antar-potongan. | Tambahkan ±200 karakter terakhir potongan sebelumnya (channel sama) ke prompt. | Bertentangan dengan transkripsi paralel (langkah 36). Kompromi: potongan pertama tiap channel dulu, sisanya paralel dengan konteks dari potongan sebelumnya yang sudah selesai; transkripsi bertahap (langkah 37) sudah berurutan per putaran. |
| **G4** | P1 | **Model turbo lebih cepat tapi kurang akurat untuk bahasa selain Inggris.** | Pilihan "Akurasi tinggi" di Pengaturan → Layanan AI → `whisper-large-v3` (lebih lambat, kuota Groq lebih boros). | Cek dulu ketersediaan model di akun pemilik dengan `examples/groq_probe.rs`. |
| G5 | P1 | **Campuran Indonesia–Inggris ditulis fonetis.** Dengan `language=id`, istilah Inggris sering jadi "miting", "dedlain". | Pilihan bahasa "Campuran Indonesia–Inggris": `language=id` + prompt memuat contoh istilah Inggris yang ditulis benar. | Dikombinasikan dengan glosarium dan G2. |
| G6 | P1 | **Bagian ragu tidak dicoba ulang.** | Potongan dengan `avg_logprob` rendah atau `compression_ratio` tinggi dikirim ulang (hanya rentang itu) dengan `temperature 0.2` dan prompt berkonteks; pilih hasil dengan `avg_logprob` lebih baik. | Butuh kuota tambahan kecil; potong WAV dengan `split_wav` yang sudah ada. |
| G7 | P1 | **Volume tidak diratakan; awal kata pelan bisa terpotong.** Suara mic yang pelan dan audio komputer saat volume Windows rendah dikirim apa adanya. VAD mode Aggressive bisa melewatkan awal kata yang lemah. | Normalisasi level per region sebelum diunggah (target puncak ±−3 dBFS, tanpa clipping). Padding VAD 300 → 400–500 ms. Onboarding/tes audio: sarankan headset bila gema terdeteksi (mic menangkap suara speaker). | Di `chunker.rs` saat menulis chunk (offset map tidak berubah). |
| G8 | P1 | **Tidak ada cara mengukur akurasi.** Perubahan di atas tidak bisa dibuktikan lebih baik atau lebih buruk. | Alat uji `examples/wer.rs`: transkrip 2–3 rekaman contoh dibandingkan dengan teks rujukan yang diketik pemilik → persentase kata salah (WER) dan kalimat hilang, sebelum/sesudah tiap perubahan. | Alat uji, bukan bagian app (sesuai kebijakan "test otomatis terbatas"). |
| G9 | P2 | **Typo dan tanda baca tidak dirapikan.** | Setting opsional "Rapikan transkrip dengan AI": betulkan ejaan sesuai glosarium, typo, dan tanda baca tanpa mengubah makna; teks asli tetap disimpan dan bisa dikembalikan. | Mahal di Groq (batas 8.000 token/menit → meeting 1 jam ±3–5 menit lebih lama); cocok untuk penyedia berbayar. |
| G10 | P2 | **Koreksi pengguna tidak dipelajari.** | Lihat C3: perbaikan kata di transkrip langsung masuk glosarium, jadi meeting berikutnya lebih akurat. | — |

---

## H. Urutan pengerjaan yang disarankan

| Gelombang | Isi | Alasan |
|---|---|---|
| **0 — Fondasi (segera)** | G1 Aturan filter · G2 Prompt kalimat · G8 Alat ukur WER · A0.1–A0.4 Tawaran di widget, rekam otomatis, pengingat ulang, kenali Meet | Dua masalah yang paling merugikan: meeting tidak terekam dan kalimat hilang. Keduanya murah dikerjakan. |
| **1 — Tangkap & hadir** | A1 Impor rekaman · B1 Transkrip sementara + Ringkas sejauh ini · B2 Catatan saya · F1 Kartu "notulen siap" · A0.5 | Memperluas meeting yang bisa ditangkap dan membuat aplikasi terasa hidup selama meeting. B1 memakai data yang sudah dihasilkan langkah 37. |
| **2 — Notulen bisa diajak kerja** | C1 Tanya meeting ini · C2 Intisari & pertanyaan terbuka · C3 Perbaiki transkrip + glosarium · C4/C5 Instruksi & bahasa notulen | Mengubah notulen dari dokumen baca menjadi alat kerja. C3 sekaligus menaikkan akurasi meeting berikutnya. |
| **3 — Memori & tindak lanjut** | D1 Tanya semua meeting · D2 Tugas terstruktur + pengingat · D3 Meeting rutin & status tindak lanjut · A2 Judul dari jendela meeting · G3–G7 Akurasi lanjutan | Nilai yang bertambah seiring makin banyak meeting direkam; alasan pengguna tetap memakai aplikasi. |
| **4 — Pembeda** | E1–E3, C6–C7, D4–D6, A4, B3–B4, F2–F4, G9 | Penyempurnaan setelah inti di atas terbukti dipakai. |

### Ukuran keberhasilan yang diusulkan

| Metrik | Target |
|---|---|
| Meeting yang tertangkap (rekam langsung + impor) per pengguna per minggu | naik setelah A0 & A1 |
| Meeting Zoom/Teams/Meet yang terlewat tanpa rekaman (rekam otomatis menyala) | ±0 |
| Persentase kata salah (WER) pada rekaman uji | turun ≥ 20% relatif setelah G1–G2, diukur dengan G8 |
| Kalimat yang hilang dari transkrip (dibanding rujukan) | < 2% setelah G1 |
| Pengguna yang membuka transkrip sementara / "Ringkas sejauh ini" selama meeting | > 30% meeting panjang (> 30 menit) |
| Meeting yang memakai "Tanya" atau dibagikan (salin/ekspor/pesan) | > 60% |
| Koreksi transkrip yang masuk glosarium | ≥ 1 per 5 meeting pada bulan pertama, lalu menurun (tanda akurasi naik) |
| Tugas dengan tenggat yang selesai sebelum tenggat | naik setelah D2 |
