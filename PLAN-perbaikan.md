# Rencana Perbaikan — dari `feedback.md` (v0.1.0 → v0.4)

**Dibuat:** 2026-10-08 · **Basis:** commit `3a6d1d7` · **Sumber:** `feedback.md` (7 Okt 2026)

Lanjutan dari §19 PRD: **satu langkah = satu sesi = satu commit**, nomor langkah diteruskan dari 15.
Gerbang tiap langkah tetap sesuai `CLAUDE.md`: `cargo check` + `cargo clippy -- -D warnings` + `npm run check` hijau, lalu checklist uji manual. Setiap keputusan baru dicatat di tabel "Keputusan" `CLAUDE.md`, crate baru dicatat di tabel versi.

---

## 0. Hasil cek klaim feedback terhadap kode

| # | Klaim | Status | Bukti |
|---|---|---|---|
| A1 | Reconnect hanya saat `DEVICE_INVALIDATED` | ✅ benar | `audio/mod.rs:58`; `Recorder::reopen` (`audio/recorder.rs:221`) sudah ada, tinggal dipanggil dari notifikasi |
| A2 | Thread capture mengunci DB | ✅ benar | `on_part` (`recording.rs:175`) memanggil `db.conn()` (satu `Mutex<Connection>`, `db/mod.rs:48`) dari thread capture saat rotasi part (tiap 60 dtk); buffer = `default_period` (`audio/devices.rs:33`) |
| A3 | CSP null | ✅ benar | `tauri.conf.json:27` |
| A4 | Posisi widget tidak divalidasi | ✅ benar | `bridge.rs:70–77` langsung `set_position` |
| A6 | Delete tidak membatalkan job | ✅ benar | `commands/meetings.rs:123` tidak menyentuh worker |
| A7 | Frasa pendek lolos filter | ✅ benar | `hallucination_phrases` hanya frasa panjang (`providers.default.json:15`) |
| A8 | 413 tidak dipecah | ✅ benar | `queue/worker.rs:68` |
| A9 | Banner dari 50 item | ✅ benar | `Worker::is_paused()` ada (`queue/worker.rs:90`) tapi tidak diekspos ke UI |
| A10 | Mic lolos karena nada | ✅ benar | `audio/test_tone.rs:72` nada diputar segera, puncak mic diukur di seluruh 5 dtk |
| A11 | `ended_at` kosong saat recovery | ✅ benar | `queue/recovery.rs:38` hanya `set_duration`; `ended_at` tidak diisi |
| A12 | `saveTitle` ganda | ✅ kemungkinan besar | Enter (`meeting/[id]/+page.svelte:163`) + `onblur` (`:166`) |
| A13 | Reload di tiap `recording://state` | ✅ benar | `routes/+page.svelte:71` |
| A14 | Emoji di widget | ✅ benar | `routes/recorder/+page.svelte:166,187` |
| C3.1 | Belum ada notifikasi "Notulen siap" | ✅ benar | `queue/worker.rs:198` hanya `emit_progress` |

---

## 1. Keputusan yang perlu dari pemilik proyek (sebelum langkah terkait)

| # | Pertanyaan | Usulan default | Dipakai di |
|---|---|---|---|
| K1 | A16 bertentangan dengan pengecualian "Tanpa test otomatis". Tetap tanpa test, atau tambah unit test terbatas untuk 4 fungsi sensitif? | Tambah **hanya** 4 unit test itu (murah, melindungi refactor A2/A8) | Langkah 27 |
| K2 | C3.4 Edit ringkasan bertentangan dengan non-tujuan PRD §2. Jadikan cakupan Beta? | Ya, ubah PRD §2 + catat di §20 Log Keputusan | Langkah 23 |
| K3 | C3.6 Putar audio butuh audio tersimpan. Ubah default retensi ke "7 hari"? | Ya (F13 sudah menyediakan opsi) | Langkah 26 |
| K4 | Shortcut global default | `Ctrl+Alt+R` (bisa diubah di Pengaturan) | Langkah 21 |
| K5 | Ikon final (A15): siapa yang membuat? | Pemilik menyediakan PNG/SVG; AI hanya mengganti file & `tauri icon` | Langkah 19 |
| K6 | Auto-update & code signing butuh akun GitHub Releases + kunci tanda tangan + sertifikat OV | Disiapkan pemilik sebelum v0.3/v0.4 | Langkah 25, 28 |

---

## 2. v0.2 — Siap beta (langkah 16–20)

### Langkah 16 — Stabilitas audio (A1, A2)
**Tujuan:** suara tidak hilang diam-diam saat ganti device dan tidak glitch saat worker sibuk.

1. **A2a – buffer:** `audio/devices.rs:33` → `buffer_duration_hns = max(default_period, 2_000_000)` (200 ms). Perbarui keputusan "Buffer WASAPI" di `CLAUDE.md`.
2. **A2b – lepas DB dari thread capture:** di `recording.rs`, `on_part` hanya `send(PartEvent)` ke `std::sync::mpsc`; satu thread `part-db` (dibuat di `start`, selesai saat channel ditutup di `stop`) yang menulis ke `repo_parts`. Saat `stop`, tunggu thread ini selesai **sebelum** `finish_recording` agar status part final konsisten. Penulisan file tetap di thread capture (flush 1 dtk, kecil) — tidak perlu writer thread terpisah.
3. **A1 – notifikasi default device:** modul baru `audio/device_watch.rs` memakai `IMMNotificationClient` (ikuti contoh `device_notifications.rs` di crate `wasapi` 0.25; cek API-nya lewat Context7/source crate). `OnDefaultDeviceChanged(flow, role=eConsole)` → kirim `Channel::System` (eRender) / `Channel::Mic` (eCapture) ke monitor. Debounce 1 dtk (Windows sering mengirim beberapa event beruntun).
4. Di monitor `recording.rs` (tick 100 ms): jika menerima event perubahan → `Recorder::reopen(channel)` (pakai jalur reconnect yang sama: retry 1 dtk maks 10x). Watcher hanya hidup selama merekam.
5. Log `info` tiap perubahan device + nama device baru.

**Uji manual:** rekam via `record_both` / UI, putar video, ganti output speaker → headset BT lewat ikon volume Windows; ganti mic default; jalankan "Proses" meeting panjang lain bersamaan. Cek: audio system tetap ada setelah ganti (celah ≤ 2 dtk), panjang mic = system, tidak ada glitch di log.

### Langkah 17 — Keamanan & widget (A3, A4, A14, A12, A13)
1. **A3:** set CSP di `tauri.conf.json`:
   `default-src 'self'; connect-src ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: http://asset.localhost`. Jalankan `npm run tauri dev` dan build release, buka semua jendela (main, recorder, onboarding, settings) — pastikan tidak ada pelanggaran CSP di devtools.
2. **A4:** di `bridge.rs` `open_recorder`: ambil `available_monitors()`; posisi tersimpan valid jika titik kiri-atas widget (+ margin 20 px) ada di dalam `work_area` salah satu monitor. Jika tidak → pojok kanan atas `primary_monitor()` (margin 16 px).
3. **A14:** ganti emoji di `routes/recorder/+page.svelte` dengan SVG inline (Lucide: `pause`, `play`, `mic`, `mic-off`, `square`) sebagai komponen kecil `src/lib/components/Icon.svelte`; `aria-label` tetap dari i18n.
4. **A12:** di `saveTitle()` keluar lebih awal jika `!editing`; set `editing = false` sebelum `await`.
5. **A13:** di `routes/+page.svelte`, simpan status rekam terakhir; `reload()` hanya jika berubah dari/ke `idle`.

**Uji manual:** cabut monitor kedua (atau ubah posisi tersimpan di DB ke koordinat luar layar) → widget muncul di layar utama; Enter di edit judul → satu toast; pause/mute tidak memicu reload daftar.

### Langkah 18 — Hasil keluar dari app (C3.1, C3.2)
1. **C3.1 Notifikasi "Notulen siap":**
   - `queue/worker.rs:198` setelah `Done` → `events.notify_meeting_done(id, title)` (method baru di trait `EventSink`, `events.rs`).
   - `bridge.rs`: notifikasi Windows "Notulen siap: {judul}". Klik notifikasi → buka/buat jendela main lalu navigasi ke `/meeting/{id}`. Cek dulu di docs `tauri-plugin-notification` 2.5 (Context7) apakah aksi klik didukung di Windows desktop; jika tidak, pakai pola yang sama dengan `take_pending_consent`: simpan `pending_open_meeting` dan buka saat jendela main berikutnya fokus / saat tray diklik.
   - Tidak memberi notifikasi jika jendela main sedang fokus di detail meeting yang sama.
2. **C3.2 Salin notulen:** di `meeting/[id]/+page.svelte` tombol "Salin notulen" (dropdown: *Teks biasa* / *WhatsApp* `*tebal*`) dan "Salin action items". Formatter murni di `src/lib/format.ts` (`formatMinutes(meeting, summary, style)`), mengikuti §14.7 tanpa transkrip. Pakai `navigator.clipboard.writeText` + toast. String baru di `lib/i18n/id.ts`.

**Uji manual:** Stop rekaman dengan jendela main tertutup → notifikasi muncul → klik membuka detail. Tempel hasil salin ke WhatsApp Web & Notepad.

### Langkah 19 — Akurasi kecil (A7, A10, A11, A15)
1. **A7:** di `pipeline/filter.rs` tambah aturan: segment dibuang jika `normalize(text)` ≤ 3 kata **dan** ada di daftar `short_hallucination_phrases` **dan** durasi region VAD asal < 1 dtk. Tambah key `short_hallucination_phrases: ["terima kasih", "ya", "oke", "thank you", "thanks", "you"]` di `providers.default.json` (merge rekursif sudah mengisi key baru untuk pengguna lama). Butuh durasi region → cek apakah offset map/chunk menyimpan batas region; jika tidak, pakai durasi segment (`end - start`) sebagai pendekatan dan catat keputusannya.
2. **A10:** di `audio/test_tone.rs` ukur puncak mic hanya di jendela 0–1,5 dtk, nada diputar mulai 1,5 dtk; UI onboarding meminta pengguna bicara di awal ("Ucapkan sesuatu…"). Hitung ulang ambang `PASS_DBFS` jika perlu.
3. **A11:** `queue/recovery.rs` → saat memproses meeting `interrupted`, isi `ended_at = started_at + duration_ms` (fungsi repo baru atau pakai `finish_recording`).
4. **A15:** ganti `src-tauri/icons/` dengan ikon final dari pemilik (K5): `npx tauri icon <sumber.png>` + `tray-idle.png`, `tray-recording.png`, `tray-processing.png` (ikon tray saat worker memproses — perlu `EventSink::processing_changed`).

### Langkah 20 — Checklist uji beta (A5, bagian B) + build v0.2
Jalankan seluruh tabel B `feedback.md` lewat **UI release build** dan catat hasil di `CLAUDE.md` (bagian "Hasil ukur NFR" + "Hasil verifikasi"). Perbaikan kecil yang ditemukan masuk commit ini; bug besar jadi langkah baru.

| Skenario | Siapa |
|---|---|
| Alur UI lengkap, tray Stop, kill proses → banner terputus | pemilik |
| Meeting nyata 1 jam + ukur RAM/CPU 60 menit (Process Explorer / `Get-Process` tiap 10 dtk ke CSV) | pemilik |
| Cabut headset / ganti default (verifikasi langkah 16) | pemilik |
| Auto-stop 10 menit hening (sementara set `auto_stop_silence_min = 1` di `providers.json`) | pemilik |
| Matikan Wi-Fi saat `transcribing` | pemilik |
| Key salah → dijeda → key benar | pemilik |
| Windows 10 22H2 bersih (VM) | pemilik |

Lalu bump versi ke `0.2.0` (`package.json`, `Cargo.toml`, `tauri.conf.json`) dan `npm run tauri build`. `tauri-driver` (A5 opsional) **ditunda** — tidak sejalan dengan keputusan tanpa test otomatis.

---

## 3. v0.3 — Seamless (langkah 21–25)

| Langkah | Isi | Catatan teknis |
|---|---|---|
| **21** | C1.2 shortcut global + C1.3 consent cepat + C1.4 autostart | `tauri-plugin-global-shortcut`, `tauri-plugin-autostart` (arg `--minimized` → jangan buat jendela main). Shortcut saat idle → popup consent (F5 tetap); saat merekam → Stop. Consent: salin pesan ke clipboard saat popup dibuka, fokus ke checkbox, Enter = Mulai jika dicentang. Setting baru: `globalShortcut`, `autostart`. |
| **22** | C1.1 deteksi meeting (F12) + C2.1 tawaran Stop | Modul `windows_integration/meeting_detect.rs` sesuai PRD §14.8 (polling 10 dtk ConsentStore; *verifikasi struktur registry di Win10 & 11 dulu*). Event: `MicInUse{app}` → notifikasi "Meeting terdeteksi"; `MicReleased{app}` saat merekam → tunggu 30 dtk → notifikasi + widget "Meeting selesai? Stop dalam 60 dtk" dengan tombol Batal. Hanya sekali per sesi pemakaian mic. Setting on/off. |
| **23** | C3.4 edit ringkasan & action item (butuh K2) + C3.5 rename "Peserta lain" (F10) | Migrasi `002_edits.sql`: kolom `summaries.edited INTEGER`. Command `update_summary`. "Buat ulang ringkasan" minta konfirmasi jika `edited`. Rename memakai tabel `meeting_speaker_names` yang sudah ada; label dipakai di transkrip, salin & ekspor. |
| **24** | C2.2 indikator kesehatan audio + C5.1 friksi API key | Monitor `recording.rs`: system < −90 dBFS selama 2 menit sementara mic aktif → event `recording://warning{kind:"system_silent"}` → teks di widget. Onboarding: deteksi `gsk_` dari clipboard saat fokus input (minta konfirmasi, jangan tempel diam-diam) + tautan/GIF panduan. |
| **25** | C5.3 auto-update (butuh K6) | `tauri-plugin-updater` + `latest.json` di GitHub Releases; cek saat start + tiap 24 jam; tawarkan pasang hanya saat tidak merekam/memproses. |

## 4. v0.4 — Bernilai harian (langkah 26–29)

| Langkah | Isi | Catatan teknis |
|---|---|---|
| **26** | C3.3 ekspor MD/TXT/PDF (F11) + C3.6 klik timestamp putar audio (K3) | Ekspor lewat `tauri-plugin-dialog` save + formatter langkah 18; PDF via `window.print()` + CSS `@media print`. Audio: gabung part per channel → protokol `asset:` (perlu `assetProtocol` scope + CSP langkah 17), `<audio>` seek ke `start_ms`. |
| **27** | A16 unit test (K1) + A6 cancellation + A8 split 413 | Test: `Aligner::align`, `map_time`, `plan_chunks`, `parse_final`. A6: `tokio_util::sync::CancellationToken` (atau `AtomicBool` + `select!`) per job di `Worker.current`; `delete_meeting` membatalkan dulu. A8: pecah chunk di batas region VAD tengah, bangun ulang offset map, transkrip dua bagian. |
| **28** | C4.1 pencarian FTS5 + C4.2 "Tugas saya" + A9 `queuePaused` | Migrasi `002_fts.sql` (rancangan PRD) — gabungkan nomor migrasi dengan langkah 23. A9: tambah `queuePaused` ke `get_onboarding_status` dari `Worker::is_paused()`. C5.4 code signing dikerjakan di sini jika sertifikat siap. |
| **29** | C3.7 template ringkasan (F14) + C5.2 sisa kuota + C5.5 laporan masalah | Prompt per template di `llm/prompts.rs`; tebak otomatis dari 2 menit pertama transkrip. Kuota dari `usage_log` + header `x-ratelimit-remaining-*` (simpan terakhir di memori). Laporan: zip folder log + versi (tanpa DB/audio) ke lokasi pilihan pengguna. |

## 5. v1.0 — Pembeda (belum dirinci)
C3.8 diarization, C4.3 tanya AI, C1.5 kalender, C3.9 integrasi tugas, C2.3–C2.5, C4.4–C4.5, C5.6–C5.7. Masing-masing butuh evaluasi/spike sendiri (ukuran app, kuota, OAuth) sebelum dijadwalkan — buat PRD addendum terpisah.

---

## 6. Urutan & ketergantungan

```
16 audio ─┐
17 CSP/widget ─┼─> 20 uji beta + build v0.2 ─> 21 ─> 22 ─> 23 ─> 24 ─> 25 (K6)
18 notif/salin ┤                                              │
19 akurasi ────┘                    18 formatter ──> 26 ekspor │
                                     17 CSP ───────> 26 audio  │
                                     23 migrasi 002 ─> 28 FTS (nomor migrasi berikutnya)
```

Langkah 16–19 independen satu sama lain dan bisa dikerjakan dalam urutan apa pun; 16 paling berisiko sehingga didahulukan.

## 7. Pengukuran (bagian E feedback)
Tambahkan di langkah 20: log `StopReason` sudah ada → hitung rasio auto-stop dari log. Langkah 18: catat timestamp `Done` dan `stopped_at` di log `info` untuk metrik "Stop → Notulen siap". Penghitung lokal "dibagikan" (salin/ekspor) ditunda ke langkah 26 (tanpa telemetri, tampil di Pengaturan).

---

## 8. Status pengerjaan (2026-10-08)

Koding langkah 16–19 dan 21–29 selesai, satu commit per langkah. Gerbang yang dijalankan: `cargo clippy --all-targets -- -D warnings`, `npm run check`, `npm run build`. **Belum ada uji manual dan `cargo test` belum dijalankan** (atas permintaan pemilik). Langkah 20 (uji beta + bump versi + build NSIS) menunggu checklist di bawah.

Keputusan default yang dipakai (bisa diubah pemilik):
- K1: unit test hanya untuk fungsi sensitif (+ uji FTS5 & `split_point_ms`).
- K2: edit ringkasan masuk Beta (PRD §2 & §20 diperbarui).
- K3: retensi audio default **7 hari**.
- K4: shortcut `Ctrl+Alt+R`.
- K5: ikon final belum (ada placeholder `tray-processing.png`).
- K6: updater nonaktif sampai env `MPA_UPDATER_PUBKEY`/`MPA_UPDATER_ENDPOINT` diberikan saat build; code signing belum.

## 9. Checklist uji manual (langkah 20)

Jalankan `cargo test --lib` (dari `src-tauri/`), lalu `npm run tauri build` dan uji lewat installer.

**Rekaman & audio**
- [ ] Rekam, putar video, ganti output speaker → headset lewat ikon volume Windows: audio sistem tetap terekam (celah ≤ 2 dtk), log `default device system berubah`.
- [ ] Ganti mic default saat merekam: mic pindah.
- [ ] Merekam sambil meeting lain diproses (transkrip panjang): tidak ada glitch/celah di WAV.
- [ ] Mute output (volume 0) 2 menit sambil bicara → widget "Audio sistem tidak terdengar"; nyalakan lagi → hilang.
- [ ] Monitor kedua dilepas → widget muncul di pojok kanan atas monitor utama.
- [ ] Ikon widget (pause/play/mic/stop) tampil rapi.

**Mulai/stop tanpa berpikir**
- [ ] `Ctrl+Alt+R` dari Zoom/browser → langsung merekam (widget muncul); saat merekam → Stop. Ganti shortcut di Pengaturan; shortcut bentrok → pesan error, yang lama tetap aktif.
- [ ] Tombol Mulai rekam dan menu tray "Mulai rekam" langsung merekam tanpa popup.
- [ ] Onboarding langkah akhir centang autostart → restart Windows → app jalan di tray tanpa jendela.
- [ ] Buka Zoom/Teams/Meet (browser) dan mulai pakai mic → notifikasi "Meeting terdeteksi"; klik → banner "Mulai rekam?" di jendela main; klik Mulai → `source_app` tersimpan. Tidak muncul lagi di sesi mic yang sama. **Verifikasi di Windows 10.**
- [ ] Saat merekam, tutup meeting (mic dilepas) → ±30 dtk → widget "Meeting sepertinya sudah selesai" + hitung mundur 60 dtk → Stop otomatis; "Lanjut" membatalkan.

**Setelah meeting**
- [ ] Jendela main tertutup → job selesai → notifikasi "Notulen siap" → klik → detail meeting terbuka.
- [ ] Salin notulen (teks/WhatsApp/action items) → tempel ke WhatsApp & Notepad.
- [ ] Ekspor .md/.txt (nama file `{judul}_{tanggal}`), Cetak → Simpan PDF hanya berisi notulen.
- [ ] Klik timestamp transkrip → audio diputar dari titik itu (retensi 7 hari).
- [ ] Edit ringkasan/keputusan/topik/action item → badge "Diedit"; Buat ulang ringkasan → konfirmasi.
- [ ] Ganti nama "Peserta lain" → transkrip, salin, dan ringkasan ulang memakai nama baru.
- [ ] Ganti template (mis. Standup) → ringkasan dibuat ulang dengan fokus berbeda; mode Otomatis menampilkan jenis yang dikenali.
- [ ] Simpan judul dengan Enter → satu toast saja.

**Mengelola meeting**
- [ ] Cari kata dari transkrip/ringkasan di Beranda → hasil dengan sorotan → klik membuka tab yang tepat.
- [ ] Halaman Tugas: filter "untuk saya", centang selesai tersinkron dengan detail.
- [ ] Hapus meeting saat `transcribing` → request berhenti, tidak ada error palsu di log.
- [ ] Key salah → banner "Pemrosesan dijeda" muncul walau meeting gagal ada di halaman 2.
- [ ] Meeting terputus (kill proses) → Proses → detail menampilkan jam selesai.

**Lainnya**
- [ ] Tes audio onboarding: diam saat tes → mic ✖ walau nada terdengar.
- [ ] Segment "Terima kasih." pendek di hening tidak muncul di transkrip.
- [ ] Pengaturan: kuota hari ini tampil; "Kirim laporan masalah" menyimpan .txt tanpa nama/transkrip.
- [ ] Cek pembaruan di build tanpa env updater → pesan "Pembaruan otomatis tidak aktif".
- [ ] Tidak ada pelanggaran CSP di devtools (main, recorder, onboarding, settings, tasks).
- [ ] Ukur ulang NFR: RAM idle, ukuran installer, RAM/CPU saat merekam 60 menit.

Setelah lolos: bump versi (`package.json`, `Cargo.toml`, `tauri.conf.json`) dan build NSIS.
