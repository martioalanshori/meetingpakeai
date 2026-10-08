# Rencana: halaman Beranda (versi sederhana) sebagai tampilan pertama

**Tujuan:** saat aplikasi dibuka, pengguna langsung bisa:
- bertanya ke meeting-meetingnya,
- melihat notulen terakhir,
- tahu tugas yang mendesak.

Tidak lebih dari itu. Daftar meeting yang sekarang ada di `/` pindah ke halaman **Meeting** (`/meetings`).

**Aturan eksekusi:**
- Satu langkah = satu commit.
- Tanpa pengujian oleh AI.
- Gerbang tiap langkah: `npm run check` + `cargo clippy --all-targets -- -D warnings`.
- Keputusan dicatat di `CLAUDE.md`.

---

## 1. Tampilan

Satu kolom di tengah (`max-w-2xl`), sama di layar lebar maupun sempit. Tanpa kartu bertumpuk, tanpa grafik, tanpa angka statistik.

```
Selamat pagi, Budi
Siap merekam · deteksi meeting aktif

[ Tanya apa saja dari meeting Anda…                         ➤ ]

(satu baris status, hanya jika ada:
 "Merekam 00:12:31 · Lihat transkrip"  /  "Menyusun notulen Sync Mingguan… 60%"
 /  "1 meeting gagal diproses · Lihat")

Notulen terbaru
  Sync Mingguan                                   Sel 14.00
  Rilis dipindah ke 20 Okt; QA mulai Senin.
  Rapat Klien A                                   Sen 10.00
  Klien setuju harga paket B.
  Review Desain                                   Jum 15.30
  Tiga revisi untuk halaman checkout.
  Semua meeting →

Tugas mendesak
  ☐ Kirim penawaran revisi            lewat tenggat
  ☐ Siapkan data QA                   hari ini
  ☐ Konfirmasi jadwal rilis           besok
  Semua tugas (13) →
```

**Aturan isi:**

| Bagian | Aturan |
|---|---|
| Sapaan | "Selamat pagi/siang/sore/malam, {nama}". Baris kecil di bawahnya: "Siap merekam · deteksi meeting aktif", atau peringatan singkat + tombol bila deteksi/autostart mati. |
| Tanya | Satu kolom besar. Jawaban AI tampil tepat di bawahnya, dengan tautan rujukan ke meeting + waktu. Fitur ini pindah dari daftar meeting; halaman Meeting kembali ke pencarian kata biasa. |
| Status | Maks satu baris dan hanya bila ada: sedang merekam, sedang memproses, atau ada yang gagal/terputus/antrean dijeda. |
| Notulen terbaru | 3 meeting selesai terakhir: judul, hari & jam, satu kalimat (intisari poin pertama; jika tidak ada, kalimat pertama ringkasan). |
| Tugas mendesak | Maks 3 tugas terbuka: lewat tenggat → hari ini → tenggat terdekat → (bila kosong) tugas terbaru. Bisa dicentang langsung. Bagian disembunyikan bila tidak ada tugas terbuka. |
| Pengguna baru (belum ada meeting) | Ganti "Notulen terbaru" & "Tugas" dengan 2 baris petunjuk: "Mulai rekam dari tombol di kiri atau Ctrl+Alt+R" dan "Atau impor rekaman yang sudah ada" (tombol Impor). |

**Yang sengaja tidak ada di Beranda:**
- tombol rekam besar (sudah ada di sidebar),
- tombol Impor (ada di halaman Meeting; muncul di Beranda hanya untuk pengguna baru),
- kartu "Minggu lalu" (tetap di halaman Meeting),
- label,
- angka statistik.

---

## 2. Navigasi & rute

| Sekarang | Sesudah |
|---|---|
| `/` = daftar meeting (+ panel detail `/?m=`) | `/` = **Beranda** |
| — | `/meetings` = daftar meeting (+ panel detail `/meetings?m=&tab=&at=`) |
| `/meeting/[id]` → `/?m=` di layar lebar | → `/meetings?m=` |
| Rel kiri: Meeting, Tugas, Pengaturan | Rel kiri: **Beranda**, Meeting, Tugas, Pengaturan (ikon `home` baru) |

Tautan yang disesuaikan:
- `MeetingDetail.svelte`: tombol Kembali dan setelah hapus/gabung → `/meetings`.
- `meeting/[id]/+page.svelte`: alihan layar lebar → `/meetings`.
- Rujukan "Tanya" di Beranda → `/meeting/<id>?tab=transcript&at=`.

Onboarding selesai → Beranda. Tray "Notulen terakhir" dan notifikasi tetap membuka detail meeting.

---

## 3. Langkah

| Langkah | Isi |
|---|---|
| **B1 — Pindah daftar meeting** | Isi `src/routes/+page.svelte` → `src/routes/meetings/+page.svelte` tanpa perubahan perilaku; sesuaikan tautan & alihan (bagian 2); rel kiri + item Beranda & ikon `home`. |
| **B2 — Data Beranda** | Command tambahan `home_overview`: `{ status (merekam/memproses/perlu perhatian), recent[3] {id, title, startedAt, line}, urgentTasks[3], openTasks, hasMeetings }`. |
| **B3 — Tampilan Beranda** | Bagian 1; Tanya dipindah dari daftar meeting ke Beranda; diperbarui lewat event `meeting://updated`, `job://progress`, `recording://state`; kerangka saat memuat. |
| **B4 — Rapikan** | i18n, judul jendela "Beranda", aksesibilitas, catatan `CLAUDE.md`, status rencana. |

**Uji pemilik:**
- Buka aplikasi → Beranda.
- Bertanya → jawaban + rujukan.
- Rekam lalu Stop → baris status lalu notulen muncul di "Notulen terbaru".
- Centang tugas dari Beranda.

---

## 4. Catatan koordinasi

Sesi lain (`meetingpakeai-f9`) masih punya perubahan belum di-commit di:
- `src/routes/+page.svelte`
- `+layout.svelte`
- `AppRail.svelte`
- `RecordButton.svelte`
- `ui.svelte.ts`
- `app.css`
- `MeetingDetail.svelte`

B1 memindahkan `+page.svelte` dan mengubah `AppRail.svelte`, jadi sebaiknya perubahan itu di-commit dulu.

**Status:** B1–B4 selesai (8 Okt 2026), belum diuji pemilik.
