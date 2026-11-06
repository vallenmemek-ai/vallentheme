# VALLENTHEME — Paket Rencana untuk AI Agent

Paket ini menggantikan `vallentheme-plan.md` (versi ringkas sebelumnya). Isinya lebih rinci, dipecah per topik dan per fase.

**vallentheme** = aplikasi Linux untuk menjadikan **foto** atau **video (live wallpaper)** sebagai wallpaper desktop.
Bahasa **Rust**, GUI **GTK4 + libadwaita**, tampilan **mirip Fresco tapi hitam pekat, modern, ringan**.
Target pertama: **Debian 13 + MATE + X11**. Aplikasi akan **dipublikasikan**, jadi jangan asumsikan spek perangkat pengguna.
Prioritas: **stabilitas > kinerja > fitur > kecantikan**.

---

## Cara memakai paket ini (untuk agent)

1. Baca **semua** file di `panduan/` satu kali sebelum menulis kode apa pun.
2. Kerjakan fase di `fase/` **berurutan**, satu fase dalam satu waktu.
3. Di awal tiap fase, baca ulang file `panduan/` yang disebut di bagian "Baca dulu".
4. Jangan lanjut ke fase berikutnya sebelum **semua Gate** fase sekarang lulus.
5. Bila ada yang ambigu, bertentangan, atau terblokir: **berhenti dan tulis pertanyaan** dengan format di bawah. Jangan menebak.

## Peta file

| File | Isi |
|---|---|
| `panduan/00-aturan-agent.md` | Aturan kerja, kode, keamanan, daftar crate yang diizinkan |
| `panduan/01-arsitektur.md` | Struktur repo, proses, thread, state machine, alur data |
| `panduan/02-gui-design-system.md` | Warna, tipografi, spasi, komponen, CSS awal, ikon |
| `panduan/03-gui-layar-library.md` | Layar utama (daftar wallpaper) sangat rinci |
| `panduan/04-gui-layar-edit.md` | Layar edit wallpaper sangat rinci |
| `panduan/05-gui-pengaturan-dialog-status.md` | Pengaturan, dialog, kondisi kosong/error, shortcut, aksesibilitas |
| `panduan/06-referensi-teknis.md` | D-Bus, opsi mpv, X11, DB, config, perintah ffmpeg/yt-dlp |
| `fase/FASE-0` s.d. `FASE-9` | Satu file per fase: tujuan, langkah, kasus tepi, tes, Gate |

## Ringkasan fase

| Fase | Nama | Hasil utama |
|---|---|---|
| 0 | Persiapan & fondasi repo | Workspace bersih, lint, logging, config |
| 1 | Spike render X11 | Bukti video bisa jadi wallpaper di MATE |
| 2 | Core (data, scanner, thumbnail) | Library inti teruji tanpa GUI |
| 3 | Kerangka GUI & tema | Jendela hitam modern, navigasi |
| 4 | Halaman Library | Grid cepat, cari, sortir, tambah, hapus |
| 5 | Daemon produksi | Terapkan wallpaper, pulih sendiri, restore saat login |
| 6 | Halaman Edit | Semua pengaturan per wallpaper bekerja |
| 7 | Sistem kinerja | Otomatis + manual, jeda cerdas |
| 8 | Fitur tambahan | Dari tautan, Pengaturan lengkap |
| 9 | Pengerasan & rilis | Soak 24 jam, paket .deb/Flatpak, v0.1.0 |

## Lima aturan emas
1. Stabilitas di atas segalanya: daemon **tidak boleh crash**.
2. Tidak ada `unwrap()`/`panic!` di kode non-test.
3. File asli pengguna **tidak pernah** diubah atau dihapus.
4. Operasi berat **tidak boleh** di thread UI.
5. Satu fase, satu waktu. Gate wajib lulus.

## Format pertanyaan (bila terblokir)

```
BLOKIR — Fase N
Konteks: (apa yang sedang dikerjakan)
Masalah: (apa yang tidak jelas / gagal, sertakan log singkat)
Yang sudah dicoba: ...
Opsi: A) ...  B) ...
Rekomendasi agent: ...
```

Pemilik proyek akan membawa pertanyaan itu ke perencana (Claude) untuk merevisi dokumen.

## Format laporan akhir fase (tulis di `docs/PROGRESS.md`)

```
## Fase N — nama (tanggal)
Status: SELESAI / SEBAGIAN
Gate: [x] ... [x] ... [ ] ... (jelaskan yang belum)
Yang dibuat: ...
Keputusan teknis yang diambil (dan alasannya): ...
Bug/utang teknis yang diketahui: ...
Hasil ukur (RAM/CPU/dll): ...
```

## Catatan koreksi dari versi sebelumnya
- Properti `cpu` dari mpv **tidak ada**. Beban CPU dibaca dari `/proc` (lihat Fase 7).
- Mode pengisian **Ubin** hanya untuk **foto** (lewat setter desktop), bukan video.
- Daemon menyimpan state sendiri (bukan membaca DB) agar bisa restore saat login tanpa GUI.
