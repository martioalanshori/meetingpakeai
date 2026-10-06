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
- Di shell sesi lama mungkin perlu `export PATH="$HOME/.cargo/bin:$PATH"`.

## Pengecualian dari PRD (keputusan pemilik proyek, 2026-10-06)

1. **Tanpa test otomatis.** Pemilik menguji manual. `cargo test` dan unit test wajib di §0/§16 tidak ditulis. Gerbang per langkah: `cargo check` + `cargo clippy -- -D warnings` + `npm run check` hijau, lalu checklist uji manual.
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
| Setting teks kosong | `userDisplayName`/`consentMessage` kosong setelah trim → kembali ke default. |
| Default providers | Disematkan di binary (`include_str!` dari `resources/providers.default.json`) lalu ditulis ke app data jika belum ada. |
| reqwest TLS | Fitur `rustls-tls` di PRD §6.1 sudah tidak ada di reqwest 0.13; TLS default 0.13 = rustls (aws-lc-rs). Tetap memenuhi NFR "TLS (rustls)". |
| Modul `groq.rs` | Bagian bersama Groq (client, `ProviderError`, pemetaan HTTP→error, `list_models`) di `src-tauri/src/groq.rs`; dipakai `stt/groq.rs` & `llm/groq.rs`. |
| `get_onboarding_status.micPermission` | Sementara selalu `unknown` sampai langkah 8. |
| UI API key | Bagian API key di Pengaturan dibuat di langkah 3 (agar bisa diuji); key diuji dulu, disimpan hanya jika lolos. |
| Fallback `rubato` (§7.1) | Belum dibuat: autoconvert WASAPI terbukti jalan di mesin dev (lihat Hasil verifikasi). Ditambahkan hanya jika ada laporan device yang menolak format 16 kHz mono. |
| Baca buffer WASAPI | Pakai `read_from_device` (bukan `read_from_device_to_deque` yang memanggil `.unwrap()` saat ReleaseBuffer → panic jika device dicabut). Paket bertanda `silent` ditulis sebagai nol. |
| Buffer WASAPI | `buffer_duration_hns` = default device period (bukan min period seperti contoh crate) agar CPU lebih hemat. |
| Isi celah saat loopback diam | Selain saat paket datang (§7.2), tiap putaran loop (≤100 ms) channel yang tertinggal > 200 ms langsung diisi nol sampai `expected`, agar file di disk mengikuti timeline (penting untuk recovery crash). |
| Batas saat Stop | Saat Stop, jam dibekukan, panjang final dihitung, lalu tulisan dibatasi ke panjang itu dan kekurangan di-pad nol → kedua channel sama persis. |
| `loopback.rs` (§6.2) | Tidak dibuat terpisah: `audio/devices.rs` membuka mic & loopback, `audio/capture.rs` dipakai keduanya. Orkestrasi dua channel di `audio/recorder.rs`. |
| Event tambahan `tray://start-recording` | Tidak ada di §12.4. Dipakai menu tray "Mulai rekam" untuk membuka popup consent di jendela main (F5: tidak ada jalur Start tanpa consent). |
| `events.rs` + `bridge.rs` | Trait `EventSink` (emit, notify, recording_changed) di `events.rs`; implementasi Tauri di `bridge.rs` (tray, widget, notifikasi). |
| `recording.rs` | Service rekaman (validasi Start/Stop, meeting & part di DB, monitor level 10 Hz / auto-stop / reconnect / disk). |
| Notifikasi auto-stop | Teks notifikasi Windows saat auto-stop ditulis di Rust (`recording.rs`), bukan i18n UI. |
| Stop < 5 dtk dari widget | Widget memakai notifikasi Windows (`id.toast.tooShort`) karena jendela main bisa tersembunyi. |
| Posisi widget | Disimpan saat widget disembunyikan (Stop), dipulihkan saat muncul. |
| Contoh uji manual | `src-tauri/examples/record_mic.rs`, `record_both.rs`, `repair_wav.rs`, `record_service.rs`, `groq_probe.rs` — alat uji, bukan bagian app. |
| Rute dinamis `meeting/[id]` | `prerender = false` (dilayani lewat fallback SPA `index.html`). |

## Hasil verifikasi §21

- 2026-10-06: `GET /openai/v1/models` dengan key tidak valid → HTTP 401 (dipetakan ke `INVALID_API_KEY`).
- 2026-10-06 #1 (`groq_probe`, key pemilik): `whisper-large-v3-turbo` dan `qwen/qwen3.8-27b` **ada**.
- 2026-10-06 #3: `verbose_json` → top-level `duration, language, segments, task, text, x_groq`; segment berisi `start, end, text, no_speech_prob, avg_logprob, compression_ratio` (+ id, seek, temperature, tokens). Sesuai PRD. Catatan: 60 dtk mic hening → "Terima kasih." dengan no_speech_prob 0 → VAD wajib.
- 2026-10-06 #4: `reasoning_effort: "none"` + `response_format: json_object` diterima qwen/qwen3.8-27b (HTTP 200, JSON bersih).
- 2026-10-06 #5 header: STT `x-ratelimit-limit-audio-seconds: 7200`, `x-ratelimit-limit-requests: 2000`; LLM `x-ratelimit-limit-requests: 1000`, `x-ratelimit-limit-tokens: 8000`, `x-ratelimit-reset-*`. Sesuai `providers.json` default. `retry-after` hanya muncul saat 429 (belum teramati).
- 2026-10-06 #6 `wasapi` 0.25: capture mic shared+event dengan **autoconvert ke 16 kHz mono PCM16 berhasil** (Windows 11, mesin dev). Rekam 65 dtk → 64,99 dtk audio (part 1 = 960000 sampel tepat), header hound = 44 byte (`data` di offset 36). Loopback = device Render + `Direction::Capture` (crate otomatis set AUDCLNT_STREAMFLAGS_LOOPBACK); perilaku saat hening & device invalidated diuji di langkah 5/7.
- 2026-10-06 loopback (`record_both` 40 dtk, pause 6 dtk, mute 8 dtk, nada 1 kHz diputar): nada tertangkap di channel system (−15,3 dBFS); loopback diam → tidak ada paket, celah terisi nol; panjang mic = system = 641214 sampel (selisih 0 ms); timeline 40,075 dtk vs dinding 46,1 dtk (pause tidak masuk timeline).
- 2026-10-06 repair header: file dengan header ukuran 0 + byte ganjil → diperbaiki benar.

## Progres langkah §19

- [x] 1. Scaffold Tauri 2 + SvelteKit SPA + Tailwind 4, tray, single-instance, 2 jendela
- [x] 2. `error.rs`, `db/` + migrasi 001 + repo, `config/`
- [x] 3. `secrets.rs` + command API key + `test_api_key`
- [x] 4. Capture mic + writer part + repair header
- [x] 5. Loopback + timeline padding + pause/mute + level
- [x] 6. Command rekam + widget recorder + popup consent
- [ ] 7. Device change + auto-stop + cek disk
- [ ] 8. Onboarding UI + izin mic + tes 5 detik
- [ ] 9. VAD + chunker + offset map
- [ ] 10. GroqStt + rate limiter + retry
- [ ] 11. Filter + merge + dedup
- [ ] 12. GroqLlm + prompts + parse + map-reduce
- [ ] 13. Worker + state machine + recovery + retensi
- [ ] 14. UI Beranda + Detail + Pengaturan
- [ ] 15. Uji end-to-end, ukur NFR, build NSIS
