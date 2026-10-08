# Meeting Pake AI — catatan untuk AI/developer

Sumber kebenaran: **`PRD.md`**. Baca §0 sebelum mengerjakan apa pun. Kerjakan satu langkah §19 per sesi.

## Versi ter-pin (diambil dari lockfile, 2026-10-06)

| Komponen | Versi |
|---|---|
| Rust toolchain | stable-x86_64-pc-windows-msvc, rustc 1.99.0 |
| tauri / tauri-build | 2.12.1 / 2.7.1 (fitur `tray-icon`, `image-png`) |
| tauri-plugin-opener / -notification / -single-instance | 2.7.0 / 2.5.1 / 2.5.2 |
| serde / serde_json | 1.0.229 / 1.0.151 |
| rusqlite (bundled) | 0.40.2 |
| thiserror / uuid (v4) / chrono | 2.0.21 / 1.27.0 / 0.4.45 |
| keyring (default v1 → Windows Credential Manager) | 4.2.0 |
| reqwest (default rustls + json, multipart) | 0.13.5 |
| wasapi / hound | 0.25.0 / 3.5.1 |
| tauri-plugin-dialog / winreg / webrtc-vad / strsim / async-trait | 2.8.1 / 0.56.0 / 0.4.0 / 0.11.1 / 0.1.92 |
| tauri-plugin-global-shortcut / tauri-plugin-autostart (langkah 21) | 2.4.0 / 2.7.0 |
| tauri-plugin-updater (langkah 25) | 2.13.2 |
| tokio (time, sync, macros, rt, rt-multi-thread) | 1.53.2 |
| @tauri-apps/plugin-dialog | (npm) |
| tracing / tracing-appender / tracing-subscriber (env-filter) | 0.1.44 / 0.2.5 / 0.3.23 |
| Node / npm | 22.17.0 / 10.9.2 |
| @tauri-apps/api / cli | 2.12.1 / 2.12.1 |
| @sveltejs/kit / adapter-static / vite-plugin-svelte | 2.70.3 / 3.0.10 / 7.3.1 |
| svelte / svelte-check / typescript | 5.57.2 / 4.7.6 / 6.0.3 |
| tailwindcss / @tailwindcss/vite | 4.3.3 / 4.3.3 |
| vite | 8.3.3 |

Crate lain (§6.1) ditambahkan di langkahnya masing-masing; catat versinya di tabel ini saat ditambahkan.

## Perintah

- Dev: `npm run tauri dev`
- Cek frontend: `npm run check`
- Cek Rust (dari `src-tauri/`): `cargo check` dan `cargo clippy -- -D warnings`
- Uji rekam mic (langkah 4): `cargo run --example record_mic -- <detik> [folder]` dari `src-tauri/`
- Uji rekam 2 channel (langkah 5): `cargo run --example record_both -- <detik> [folder] [pause_di pause_lama] [mute_di mute_lama]`
- Uji repair header WAV: `cargo run --example repair_wav -- <file.wav>`
- Build installer: `npm run tauri build` (NSIS, per-user)
- Build rilis dengan auto-update: set env `MPA_UPDATER_PUBKEY` (isi file `.pub` dari `npx tauri signer generate`), `MPA_UPDATER_ENDPOINT` (mis. `https://github.com/<org>/<repo>/releases/latest/download/latest.json`), `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`), lalu `npm run tauri build -- --config src-tauri/tauri.updater.conf.json`. Unggah installer, `.sig`, dan `latest.json` ke GitHub Release.
- Di shell sesi lama mungkin perlu `export PATH="$HOME/.cargo/bin:$PATH"`.

## Pengecualian dari PRD (keputusan pemilik proyek, 2026-10-06)

1. **Test otomatis terbatas.** Pemilik menguji manual. Sejak langkah 27 (feedback A16/K1) hanya ada unit test untuk fungsi paling sensitif: `Aligner` (`audio/timeline.rs`), `map_time`/`region_duration_at` (`preprocess/mod.rs`), `plan_chunks`/`split_point_ms` (`preprocess/chunker.rs`), `parse_final` (`llm/parse.rs`) — jalankan `cargo test --lib` dari `src-tauri/`. Gerbang per langkah: `cargo check` + `cargo clippy --all-targets -- -D warnings` + `npm run check` hijau, lalu checklist uji manual.
2. **Fase 0 spike dilewati** sebagai proyek terpisah. Item §21 diverifikasi di langkah terkait (model Groq di langkah 3, `wasapi`/loopback di langkah 4–5) dan hasilnya dicatat di bagian "Hasil verifikasi" di bawah.
3. Proyek di-scaffold langsung di root folder ini (bukan subfolder `meeting-pake-ai/`).

## Keputusan atas hal yang tidak tertulis di PRD

| Topik | Keputusan |
|---|---|
| `test_api_key` dengan key salah (401) | Kembalikan error `INVALID_API_KEY`. `{ ok, missingModels }` hanya untuk key valid. |
| Antrean dijeda karena 401 | Flag di memori; saat start dihitung ulang dari adanya meeting `failed` + `error_code = 'INVALID_API_KEY'`. Tanpa perubahan skema. |
| Nada tes onboarding | Sinus 1 kHz, −12 dBFS, 3 detik, dibangkitkan di kode. |
| Ikon | Placeholder: app = lingkaran indigo; tray `icons/tray-idle.png` (abu-abu) / `icons/tray-recording.png` (merah). |
| Cek disk saat merekam | Tiap 30 detik. |
| Argumen `update_settings` | `{ patch: Partial<Settings> }`. |
| tracing-subscriber | Ditambahkan (tidak ada di §6.1) karena dibutuhkan untuk memasang subscriber tracing ke file. |
| `providers.json` | Key yang hilang diisi dari default secara rekursif; `llm_extra_body` diganti utuh (bukan digabung) agar parameter default bisa dihapus. |
| Setting teks kosong | `userDisplayName` kosong setelah trim → kembali ke default. |
| Default providers | Disematkan di binary (`include_str!` dari `resources/providers.default.json`) lalu ditulis ke app data jika belum ada. |
| reqwest TLS | Fitur `rustls-tls` di PRD §6.1 sudah tidak ada di reqwest 0.13; TLS default 0.13 = rustls (aws-lc-rs). Tetap memenuhi NFR "TLS (rustls)". |
| Modul `groq.rs` | Bagian bersama Groq (client, `ProviderError`, pemetaan HTTP→error, `list_models`) di `src-tauri/src/groq.rs`; dipakai `stt/groq.rs` & `llm/groq.rs`. |
| `get_onboarding_status.micPermission` | Sementara selalu `unknown` sampai langkah 8. |
| UI API key | Bagian API key di Pengaturan dibuat di langkah 3 (agar bisa diuji); key diuji dulu, disimpan hanya jika lolos. |
| Fallback `rubato` (§7.1) | Belum dibuat: autoconvert WASAPI terbukti jalan di mesin dev (lihat Hasil verifikasi). Ditambahkan hanya jika ada laporan device yang menolak format 16 kHz mono. |
| Baca buffer WASAPI | Pakai `read_from_device` (bukan `read_from_device_to_deque` yang memanggil `.unwrap()` saat ReleaseBuffer → panic jika device dicabut). Paket bertanda `silent` ditulis sebagai nol. |
| Buffer WASAPI | `buffer_duration_hns` = maks(default device period, 200 ms) (langkah 16): thread capture yang tertahan sesaat tidak kehilangan audio. |
| Pindah default device (langkah 16) | `audio/device_watch.rs` mendaftarkan `IMMNotificationClient` (role Console) selama merekam; monitor membuka ulang channel 1 dtk setelah notifikasi terakhir (debounce). Gagal buka → jalur reconnect biasa. |
| Pencatatan part ke DB (langkah 16) | `PartEvent` dikirim lewat `mpsc` ke thread `part-db`; thread capture tidak pernah memegang kunci DB. Stop menunggu thread ini selesai sebelum `finish_recording`. |
| Isi celah saat loopback diam | Selain saat paket datang (§7.2), tiap putaran loop (≤100 ms) channel yang tertinggal > 200 ms langsung diisi nol sampai `expected`, agar file di disk mengikuti timeline (penting untuk recovery crash). |
| Batas saat Stop | Saat Stop, jam dibekukan, panjang final dihitung, lalu tulisan dibatasi ke panjang itu dan kekurangan di-pad nol → kedua channel sama persis. |
| `loopback.rs` (§6.2) | Tidak dibuat terpisah: `audio/devices.rs` membuka mic & loopback, `audio/capture.rs` dipakai keduanya. Orkestrasi dua channel di `audio/recorder.rs`. |
| `events.rs` + `bridge.rs` | Trait `EventSink` (emit, notify, recording_changed) di `events.rs`; implementasi Tauri di `bridge.rs` (tray, widget, notifikasi). |
| `recording.rs` | Service rekaman (validasi Start/Stop, meeting & part di DB, monitor level 10 Hz / auto-stop / reconnect / disk). |
| Notifikasi auto-stop | Teks notifikasi Windows saat auto-stop ditulis di Rust (`recording.rs`), bukan i18n UI. |
| Stop < 5 dtk dari widget | Widget memakai notifikasi Windows (`id.toast.tooShort`) karena jendela main bisa tersembunyi. |
| Posisi widget | Disimpan saat widget disembunyikan (Stop), dipulihkan saat muncul. |
| Reconnect / auto-stop / disk (langkah 7) | Di monitor `recording.rs` (tick 100 ms): buka ulang default device tiap 1 dtk maks 10x; hening < −50 dBFS kedua channel selama `auto_stop_silence_min` → peringatan + 2 menit; `max_recording_hours`; disk < 500 MB dicek tiap 30 dtk. Hening saat pause tidak dihitung. |
| Tes audio onboarding | Nada diputar dengan WinAPI `PlaySoundW` (file `test_tone.wav` di app data). Loopback gagal dibuka → tes tetap jalan, system = gagal. |
| Izin mikrofon | `allowed` jika setting pengguna terbaca dan semua `Value` = Allow (HKLM + HKCU + HKCU NonPackaged); `denied` jika ada yang Deny; selain itu `unknown`. Akses ditolak saat membuka mic (E_ACCESSDENIED) → `MIC_PERMISSION_DENIED`. |
| Command tambahan `open_log_folder` | Untuk tombol "Buka folder log" di Pengaturan (§14.6). |
| `response_json` chunk | Disimpan sebagai JSON `Vec<SttSegment>` hasil parse (bukan body mentah Groq) agar step merging tidak bergantung format provider. |
| Fallback parameter LLM | Jika Groq membalas 400 yang menyebut reasoning/response_format/json, request diulang sekali tanpa `llm_extra_body` & `response_format`, dan sisa sesi tanpa itu. |
| `PayloadTooLarge` (413) | Langkah 27: `StepError::TooLarge`; file chunk dipecah dua di tengah jeda antar-potongan (offset map) terdekat dari tengah (`split_point_ms`), tiap bagian ditranskrip, segmen bagian kedua digeser `cut_ms`. Offset map chunk asli tetap dipakai (tanpa perubahan DB). Maks kedalaman 2 (≤ 4 bagian); tetap 413 → `failed`. |
| Rate limiter | Jendela kosong selalu diizinkan (request besar tidak macet). Saat menunggu jendela menit/jam, cek ulang tiap 5 dtk. Pemakaian dicatat setelah request sukses (LLM: token aktual dari `usage`). |
| Error provider di `failed` | `error_code = INTERNAL` dengan pesan Indonesia (mis. "Groq menolak permintaan: …"). |
| `MeetingListItem.errorCode` | Field tambahan untuk banner "antrean dijeda" di Beranda. |
| `retry_job` di step preprocessing dengan audio terhapus | Ditolak `AUDIO_NOT_AVAILABLE`. |
| Merge perantara | Prompt `MERGE_INTERMEDIATE` (format CHUNK) di `llm/prompts.rs` untuk merge bertingkat (§10.4). |
| Virtualized list > 500 segment | Memakai CSS `content-visibility: auto` per baris (browser hanya me-render baris terlihat), tanpa library tambahan. |
| Warna label transkrip | mic `#2563eb`, system `#047857` (kontras ≥ 4.5:1 di latar putih). |
| Banner antrean dijeda | Dihitung di Beranda dari meeting `failed` dengan `errorCode` INVALID_API_KEY / NO_API_KEY. |
| Jendela on-demand (NFR RAM) | Widget `recorder` dibuat saat mulai rekam dan dihancurkan saat Stop (tidak ada di tauri.conf.json). Jendela main dihancurkan saat ditutup ke tray dan dibuat ulang dari konfigurasi saat dibuka (tray / instance kedua). `RunEvent::ExitRequested { code: None }` dicegah agar app tetap hidup di tray. |
| Contoh uji manual | `src-tauri/examples/record_mic.rs`, `record_both.rs`, `repair_wav.rs`, `record_service.rs`, `groq_probe.rs`, `audio_test.rs`, `e2e.rs` — alat uji, bukan bagian app. |
| Rute dinamis `meeting/[id]` | `prerender = false` (dilayani lewat fallback SPA `index.html`). |
| CSP (langkah 17) | `default-src 'self'; connect-src ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data:`. Hash script inline SvelteKit ditambahkan otomatis oleh Tauri. |
| Posisi widget (langkah 17) | Posisi tersimpan dipakai hanya jika pojok kiri-atas (+40 px) ada di `work_area` salah satu monitor; selain itu pojok kanan atas monitor utama (margin 16 px). |
| Ikon UI (langkah 17) | `src/lib/components/Icon.svelte`: path SVG Lucide (ISC) disalin inline, tanpa dependensi npm. |
| Notifikasi "Notulen siap" (langkah 18) | `EventSink::meeting_done`. `tauri-plugin-notification` 2.5 di desktop tidak punya handler klik → id meeting disimpan di `TauriBridge.pending_meeting` (berlaku 1 jam); jendela main mengambilnya lewat command tambahan `take_pending_meeting` saat dibuat / mendapat fokus (klik notifikasi membuka instance kedua → single-instance → fokus). Tidak dikirim jika jendela main sedang fokus. |
| Salin notulen (langkah 18, direvisi) | Atas masukan pemilik: tiap tab punya tombol Salin sendiri (`CopyButton.svelte`). Ringkasan = judul, tanggal, ringkasan, keputusan, topik; Action Items = daftar tugas; (tombol format WhatsApp dihapus atas permintaan pemilik, semua teks biasa); Transkrip = baris `[HH:MM:SS] Label: teks`. Formatter di `src/lib/minutes.ts`. Menu header hanya Ekspor (MD/TXT/PDF notulen lengkap). |
| Frasa halusinasi pendek (langkah 19) | `short_hallucination_phrases` di `providers.json`: segment dibuang jika teks ternormalisasi ≤ 3 kata, sama persis dengan salah satu frasa, **dan** potongan region VAD asal (`dur_ms` entri offset map) < 1 dtk. |
| Tes audio onboarding (langkah 19) | Puncak mic diambil di 0–2,5 dtk (pengguna bicara), lalu nada diputar untuk tes loopback sampai detik 5. |
| `ended_at` meeting terputus (langkah 19) | Recovery mengisi `ended_at = started_at + duration_ms` jika masih kosong. |
| Ikon tray memproses (langkah 19) | `icons/tray-processing.png` (placeholder lingkaran amber). Prioritas: merekam > memproses > idle; worker memanggil `EventSink::processing_changed`. Ikon final menunggu desain dari pemilik. |
| Shortcut global (langkah 21) | Setting `globalShortcut` (default `Ctrl+Alt+R`, kosong = mati), didaftarkan dari Rust (`desktop.rs`). Idle → langsung mulai rekam tanpa membuka jendela; merekam → Stop. Shortcut baru gagal didaftarkan → setting tidak disimpan, shortcut lama dipulihkan. |
| Autostart (langkah 21) | `tauri-plugin-autostart` dengan argumen `--minimized`; setting `autostart` (default mati, dicentang di langkah akhir onboarding). Jendela main `"create": false` di konfigurasi dan dibuat di `setup` kecuali start dengan `--minimized` setelah onboarding selesai. |
| Deteksi meeting (langkah 22) | `meeting_watch.rs` polling ConsentStore HKCU tiap `meeting_detection.poll_sec` (10 dtk) via `windows_integration/meeting_detect.rs`. Daftar aplikasi di `providers.json` → `meeting_detection.apps` (nama exe NonPackaged / nama paket sebelum `_`, lowercase). Exe sendiri dikecualikan. Hanya aktif setelah onboarding dan jika setting `meetingDetection` (default nyala). Tawaran sekali per sesi mic; tidak saat merekam. |
| Popup consent dihapus (2026-10-08, keputusan pemilik) | Pemilik meminta consent peserta sebelum meeting di luar aplikasi, jadi F5 (popup + checkbox tiap rekam) dihapus. `start_recording { sourceApp? }` tanpa `consentConfirmed`; tombol Mulai, menu tray, dan shortcut langsung merekam (tray/shortcut lewat `start_recording_in_background`, gagal → notifikasi). Setting `consentMessage` dan `ConsentDialog` dihapus; kolom `meetings.consent_at` tetap diisi waktu mulai. Teks privasi onboarding tetap mengingatkan tanggung jawab memberi tahu peserta. |
| Tawaran rekam dari deteksi meeting | `TauriBridge.pending_offer` (berlaku 5 menit) → command tambahan `take_pending_offer` → banner "Meeting terdeteksi (Zoom). Mulai rekam?" (`MeetingOfferBanner.svelte`) dengan tombol Mulai rekam / Abaikan. Tidak merekam otomatis saat jendela dibuka. Event tambahan `app://pending` saat jendela main sedang fokus. |
| Stop saat meeting selesai (langkah 22) | Selama merekam, jika semua aplikasi meeting yang sempat memakai mic berhenti selama `stop_grace_sec` (30 dtk) → `RecordingService::offer_stop_meeting_ended`: event `recording://auto-stop-warning` dengan `reason: "meeting_ended"` + notifikasi; tanpa respons `stop_countdown_sec` (60 dtk) → `StopReason::MeetingEnded`. Satu kali per rekaman sampai aplikasi meeting memakai mic lagi. |
| Edit ringkasan (langkah 23) | Migrasi `002_edits.sql`: `summaries.edited`. Command tambahan `update_summary { id, edit }` (hanya status `done`, ganti seluruh action item, baris kosong dibuang). |
| Nama "Peserta lain" otomatis (menggantikan rename manual F10) | Atas masukan pemilik, form ganti nama dihapus. Prompt CHUNK/FINAL/MERGE meminta field `nama_peserta_lain`: diisi hanya jika jelas hanya ada satu peserta lain (1:1) dan namanya disebut (perkenalan/dipanggil); selain itu null. Setelah ringkasan, worker menyimpan nama itu ke `meeting_speaker_names` (null / sama dengan nama pengguna → label kembali "Peserta lain"). LLM selalu melihat label default "Peserta lain" agar hasil konsisten saat buat ulang. Meeting > 2 orang tetap "Peserta lain" (butuh diarization, C3.8). |
| Kesehatan audio sistem (langkah 24) | Monitor: level sistem < −70 dBFS selama 2 menit sementara mic sempat aktif (≥ −50 dBFS, tidak mute, tidak pause) → `recording://warning { code: "system_silent" }`; suara sistem kembali → `system_ok`. Widget menampilkan baris peringatan. |
| Deteksi API key di clipboard (langkah 24) | Command tambahan `detect_api_key_in_clipboard`: baca CF_UNICODETEXT lewat Win32 (tanpa plugin); hanya mengembalikan teks berbentuk `gsk_[A-Za-z0-9_]+` (24–200 karakter). UI menawarkan tombol "Pakai key ini" (cek saat tampil & saat jendela fokus), tidak mengisi diam-diam. GIF panduan belum dibuat (butuh aset dari pemilik); langkah teks onboarding diperjelas. |
| Auto-update (langkah 25) | `tauri-plugin-updater`; `plugins.updater.pubkey` di `tauri.conf.json` sengaja kosong dan diisi saat runtime dari `option_env!("MPA_UPDATER_PUBKEY")` + endpoint `MPA_UPDATER_ENDPOINT` (`updater.rs`). Tanpa env saat build → updater nonaktif. `requireSignedVersion: true`, NSIS mode `passive`. `createUpdaterArtifacts` hanya di `tauri.updater.conf.json` agar build biasa tidak butuh kunci privat. Cek latar 1 menit setelah start lalu tiap 24 jam → notifikasi sekali per versi; pasang hanya dari Pengaturan (command tambahan `check_update`, `install_update`), ditolak saat merekam/memproses. |
| Retensi audio (langkah 26, K3) | Setting `audioRetention`: `after_transcript` / `days7` / `forever`, default **`days7`**. Pengganti bool `delete_audio_after_transcript`; nilai bool lama yang pernah disimpan dipetakan (true → after_transcript, false → forever). Worker menyapu tiap 1 jam: meeting `done`/`failed` dengan audio dan `ended_at` > 7 hari → folder rekaman dihapus. |
| Pembatalan job (langkah 27, A6) | `Worker.cancel` menyimpan `Notify` per job; `run` menjalankan `process` dalam `tokio::select!` dengan token itu. `delete_meeting` memanggil `cancel_and_wait` (maks 5 dtk) sebelum menghapus row & folder. |
| Pencarian (langkah 28, F11) | Migrasi `003_fts.sql`: FTS5 `search_index(meeting_id, kind, ref, text)` tokenizer `unicode61 remove_diacritics 2`. `repo_search::reindex` dipanggil saat job `done`, `rename_meeting`, `update_summary`; dihapus di `repo_meetings::delete`. Index kosong saat start → diisi untuk semua meeting `done`. Query pengguna diubah jadi `"kata"*` per kata (AND, prefiks) sehingga sintaks FTS tidak bisa disuntikkan. Command tambahan `search_meetings`. Hasil membuka `/meeting/<id>?tab=…`. |
| Tugas saya (langkah 28) | Halaman `/tasks`, command tambahan `list_action_items` (semua action item meeting `done`). Filter "untuk saya" di UI: PJ memuat nama pengguna atau "Saya". |
| Banner antrean dijeda (langkah 28, A9) | `get_onboarding_status.queuePaused` = `Worker::is_paused()`; Beranda tidak lagi menebak dari 50 meeting yang dimuat. |
| Code signing (C5.4) | Belum: menunggu sertifikat OV dari pemilik. Saat ada, isi `bundle.windows.certificateThumbprint` (+ `digestAlgorithm`, `timestampUrl`) atau `signCommand` di `tauri.conf.json`. |
| Template ringkasan (langkah 29, F14) | Migrasi `004_template.sql`: `meetings.summary_template` (NULL = otomatis) dan `summaries.template`. Daftar di `llm/prompts.rs` `TEMPLATES` (umum, standup, client_call, interview, kuliah, one_on_one). Instruksi template ditambahkan ke prompt FINAL (sebelum `TRANSKRIP:`) dan MERGE final; CHUNK tidak berubah. Otomatis → LLM diminta mengisi field `jenis` (hanya kunci yang dikenal yang disimpan). Command tambahan `list_summary_templates`, `set_summary_template` (simpan + buat ulang ringkasan). |
| Estimasi kuota (langkah 29, C5.2) | Command tambahan `get_quota_today`: total `usage_log` sejak 00.00 lokal vs batas harian × `safety_factor`. Estimasi jam meeting = sisa detik audio STT / 3600. Header `x-ratelimit-remaining-*` belum dipakai. |
| Laporan masalah (langkah 29, C5.5) | Command tambahan `save_problem_report`: satu file `.txt` (versi, OS, pengaturan dengan nama disembunyikan, status antrean, model, log maks 4 MB terbaru) ke lokasi pilihan pengguna lewat dialog. Tanpa zip, audio, transkrip, ringkasan, atau API key. |
| Putar audio (langkah 26) | Command tambahan `prepare_playback`: mic + sistem dicampur (jumlah ter-clamp) ke `recordings/<id>/playback.wav`, dibuat sekali. Diputar lewat asset protocol (fitur tauri `protocol-asset`, scope hanya `$APPDATA/recordings/*/playback.wav`, CSP `media-src asset: http://asset.localhost`). Klik timestamp → seek & play. |
| Ekspor (langkah 26, F11) | Command tambahan `save_export { fileName, contents }`: dialog simpan dibuka di Rust, isi ditulis ke path pilihan pengguna. Isi dari `src/lib/minutes.ts` (format §14.7, termasuk transkrip). PDF = `window.print()` dengan blok `print:block` berisi notulen teks; sisa halaman `print:hidden`. |

## Hasil verifikasi §21

- 2026-10-06: `GET /openai/v1/models` dengan key tidak valid → HTTP 401 (dipetakan ke `INVALID_API_KEY`).
- 2026-10-06 #1 (`groq_probe`, key pemilik): `whisper-large-v3-turbo` dan `qwen/qwen3.8-27b` **ada**.
- 2026-10-06 #3: `verbose_json` → top-level `duration, language, segments, task, text, x_groq`; segment berisi `start, end, text, no_speech_prob, avg_logprob, compression_ratio` (+ id, seek, temperature, tokens). Sesuai PRD. Catatan: 60 dtk mic hening → "Terima kasih." dengan no_speech_prob 0 → VAD wajib.
- 2026-10-06 #4: `reasoning_effort: "none"` + `response_format: json_object` diterima qwen/qwen3.8-27b (HTTP 200, JSON bersih).
- 2026-10-06 #5 header: STT `x-ratelimit-limit-audio-seconds: 7200`, `x-ratelimit-limit-requests: 2000`; LLM `x-ratelimit-limit-requests: 1000`, `x-ratelimit-limit-tokens: 8000`, `x-ratelimit-reset-*`. Sesuai `providers.json` default. `retry-after` hanya muncul saat 429 (belum teramati).
- 2026-10-06 #6 `wasapi` 0.25: capture mic shared+event dengan **autoconvert ke 16 kHz mono PCM16 berhasil** (Windows 11, mesin dev). Rekam 65 dtk → 64,99 dtk audio (part 1 = 960000 sampel tepat), header hound = 44 byte (`data` di offset 36). Loopback = device Render + `Direction::Capture` (crate otomatis set AUDCLNT_STREAMFLAGS_LOOPBACK); perilaku saat hening & device invalidated diuji di langkah 5/7.
- 2026-10-06 loopback (`record_both` 40 dtk, pause 6 dtk, mute 8 dtk, nada 1 kHz diputar): nada tertangkap di channel system (−15,3 dBFS); loopback diam → tidak ada paket, celah terisi nol; panjang mic = system = 641214 sampel (selisih 0 ms); timeline 40,075 dtk vs dinding 46,1 dtk (pause tidak masuk timeline).
- 2026-10-06 tes audio onboarding: nada 1 kHz −12 dBFS via PlaySound tertangkap loopback, puncak −12,07 dBFS. Registry izin mic di Windows 11 terbaca `allowed`.
- 2026-10-06 **E2E** (`e2e` 50 dtk, TTS Inggris via speaker, bahasa auto): VAD → 2 chunk (mic+system) → STT → merge → dedup (salinan echo di mic dibuang) → ringkasan single pass. Done 8 dtk setelah Stop. Judul, ringkasan Indonesia, 2 keputusan, 4 topik, 2 action item dengan PJ & tenggat ("Jumat depan (2026-10-16)").
- 2026-10-06 repair header: file dengan header ukuran 0 + byte ganjil → diperbaiki benar.
- 2026-10-08 registry ConsentStore (Windows 11, mesin dev): aplikasi packaged langsung di bawah `microphone\` (mis. `MSTeams_8wekyb3d8bbwe`), desktop di `microphone\NonPackaged\C:#…#chrome.exe`. Windows 10 belum diverifikasi.
- 2026-10-08 FTS5: `libsqlite3-sys` 0.38.2 bundled dikompilasi dengan `-DSQLITE_ENABLE_FTS5` (build.rs); unit test `fts5_aktif_di_sqlite_bundled` (belum dijalankan).

## Hasil ukur NFR (langkah 15, build release, 2026-10-06)

| Area | Target | Hasil |
|---|---|---|
| Ukuran installer NSIS | < 30 MB | **4,24 MiB** ✅ |
| RAM idle di tray | < 80 MB | **33,9 MB working set, 1 proses** ✅ |
| CPU idle | ≈ 0% | **0,00%** (15 dtk) ✅ |
| RAM jendela main terbuka | (tidak ada target) | 345 MB working set / 171 MB private, 7 proses (WebView2) |
| Waktu proses | median < 10 menit / meeting 1 jam | E2E 50 dtk audio: 8 dtk setelah Stop. Meeting 1 jam belum diuji. |
| RAM/CPU saat merekam 60 menit | < 150 MB / < 5% | **Belum diukur** (butuh rekaman 60 menit lewat UI). |

Installer: `src-tauri/target/release/bundle/nsis/Meeting Pake AI_0.1.0_x64-setup.exe`.

## Progres langkah §19

- [x] 1. Scaffold Tauri 2 + SvelteKit SPA + Tailwind 4, tray, single-instance, 2 jendela
- [x] 2. `error.rs`, `db/` + migrasi 001 + repo, `config/`
- [x] 3. `secrets.rs` + command API key + `test_api_key`
- [x] 4. Capture mic + writer part + repair header
- [x] 5. Loopback + timeline padding + pause/mute + level
- [x] 6. Command rekam + widget recorder + popup consent
- [x] 7. Device change + auto-stop + cek disk
- [x] 8. Onboarding UI + izin mic + tes 5 detik
- [x] 9. VAD + chunker + offset map
- [x] 10. GroqStt + rate limiter + retry
- [x] 11. Filter + merge + dedup
- [x] 12. GroqLlm + prompts + parse + map-reduce
- [x] 13. Worker + state machine + recovery + retensi
- [x] 14. UI Beranda + Detail + Pengaturan
- [x] 15. Uji end-to-end, ukur NFR, build NSIS

### Perbaikan dari `feedback.md` (rencana: `PLAN-perbaikan.md`)

- [x] 16. Pindah default device otomatis, buffer 200 ms, part → DB di thread terpisah
- [x] 17. CSP, validasi posisi widget, ikon SVG, judul ganda, reload Beranda
- [x] 18. Notifikasi "Notulen siap", salin notulen
- [x] 19. Frasa halusinasi pendek, tes mic sebelum nada, `ended_at` recovery, ikon tray memproses
- [ ] 20. Checklist uji manual (`PLAN-perbaikan.md` §9), bump versi, build NSIS — **belum, menunggu uji pemilik**
- [x] 21. Shortcut global, autostart, consent cepat (popup consent kemudian dihapus atas keputusan pemilik)
- [x] 22. Deteksi meeting + tawaran Stop saat meeting selesai
- [x] 23. Edit ringkasan & action item, rename "Peserta lain"
- [x] 24. Peringatan audio sistem diam, deteksi API key dari clipboard
- [x] 25. Auto-update (aktif jika env build tersedia)
- [x] 26. Ekspor MD/TXT/PDF, retensi 7 hari, putar audio dari timestamp
- [x] 27. Unit test terbatas, pembatalan job, pecah chunk 413
- [x] 28. Pencarian FTS5, halaman Tugas, `queuePaused`
- [x] 29. Template ringkasan, estimasi kuota, laporan masalah
