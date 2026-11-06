# 05 — GUI: Pengaturan, Dialog, Status, Aksesibilitas

## 1. Pengaturan (`adw::PreferencesDialog`)

Dibuka dari menu `⋯` → Pengaturan atau `Ctrl+,`. Dialog ukuran 640×560 (adaptif). Halaman:

### Halaman "Umum"
| Grup | Baris | Tipe | Keterangan |
|---|---|---|---|
| Aplikasi | Bahasa | ComboRow | Sistem, Indonesia, English. Perubahan butuh mulai ulang GUI (tampilkan toast) |
| Aplikasi | Jalankan saat login | SwitchRow | Menulis/menghapus file autostart daemon |
| Aplikasi | Tetap berjalan di latar belakang saat GUI ditutup | SwitchRow | Bawaan: nyala |
| Tampilan | Kurangi animasi | SwitchRow | Bawaan: ikuti sistem |

### Halaman "Kinerja" (diisi penuh di Fase 7)
| Grup | Baris | Tipe |
|---|---|---|
| Mode | Mode kinerja | ComboRow: Otomatis / Kualitas asli / Seimbang / Hemat / Kustom |
| Mode | Keterangan mode terpilih | Subteks dinamis |
| Decoding | Hardware decode | ComboRow: Otomatis / VA-API / NVDEC / Perangkat lunak (hanya opsi yang terdeteksi tersedia yang aktif) |
| Batas | Batas fps | ComboRow: Tanpa batas / 60 / 30 / 24 |
| Batas | Resolusi render maksimal | ComboRow: Asli / 1440p / 1080p / 720p |
| Jeda otomatis | Saat ada jendela fullscreen | SwitchRow |
| Jeda otomatis | Saat ada jendela maximize | SwitchRow |
| Jeda otomatis | Saat baterai di bawah N% | SwitchRow + SpinRow (5–90, bawaan 30) |
| Jeda otomatis | Saat layar terkunci atau mati | SwitchRow |
| Jeda otomatis | Saat mode hemat daya sistem | SwitchRow |
| Lanjutan | Prioritas proses (nice) | ComboRow: Normal / Rendah / Sangat rendah |
| Lanjutan | Optimalkan video saat impor | SwitchRow (bawaan mati) + ComboRow target (720p/1080p) |
| Diagnostik | Riwayat keputusan kinerja | ActionRow → buka dialog log keputusan |
| Diagnostik | Profil perangkat | ActionRow → dialog menampilkan hasil deteksi |
Baris yang hanya berlaku pada mode Kustom dinonaktifkan (abu-abu) di mode lain, dengan subteks "Diatur otomatis oleh mode ini".

### Halaman "Penyimpanan"
| Baris | Keterangan |
|---|---|
| Lokasi unduhan | Folder (pemilih) |
| Cache thumbnail | Menampilkan ukuran + tombol **Bersihkan** |
| Cache video yang dioptimalkan | Menampilkan ukuran + **Bersihkan** |
| Batas ukuran cache | SpinRow (MB) |
| Setel ulang pengaturan | Tombol bahaya → konfirmasi (tidak menghapus library) |

### Halaman "Tentang" (atau `adw::AboutDialog` terpisah)
Nama, versi, lisensi, tautan situs/sumber, kredit, versi mpv/ffmpeg terdeteksi, tombol "Salin info debug" (versi, OS, DE, sesi, GPU, hwdec).

## 2. Dialog

Semua dialog memakai `adw::AlertDialog` atau `adw::Dialog`. Tombol utama di kanan, tombol batal di kiri. Tombol bahaya pakai `vt-danger`.

| Dialog | Judul | Isi | Tombol |
|---|---|---|---|
| Hapus dari library | "Hapus N wallpaper dari library?" | "File asli di disk tidak akan dihapus." | Batal · **Hapus** (bahaya) |
| File hilang | "File tidak ditemukan" | Path lengkap | Batal · Hapus dari library · **Cari ulang…** |
| Impor selesai sebagian | "N file dilewati" | Daftar ringkas alasan (scroll) | **Tutup** |
| Tambah folder | "Tambah folder" | Sertakan subfolder (switch), jenis (Foto/Video, centang) | Batal · **Tambah** |
| Dari tautan (Fase 8) | "Tambah dari tautan" | Kolom URL, kualitas maks (ComboRow), folder tujuan, progres | Batal · **Unduh** |
| Info wallpaper | Nama berkas | Tabel: Lokasi, Jenis, Resolusi, FPS, Codec, Durasi, Ukuran, Ditambahkan, Terakhir dipakai | Salin lokasi · Buka folder · **Tutup** |
| Daemon gagal | "Wallpaper gagal dijalankan" | Pesan ramah + kode | Salin detail · Coba lagi · Tutup |
| Mpv tidak ada | "mpv belum terpasang" | Cara pasang: `sudo apt install mpv` (dengan tombol salin) | Salin perintah · Tutup |
| Sesi bukan X11 | "Sesi ini belum didukung" | Penjelasan jujur: Wayland belum didukung penuh | Mengerti |
| Setel ulang pengaturan | "Setel ulang semua pengaturan?" | Library tidak ikut terhapus | Batal · **Setel ulang** (bahaya) |

Pesan error selalu: **apa yang terjadi** (1 kalimat) + **apa yang bisa dilakukan** (1 kalimat) + kode kecil di bawah (`VT-MPV-002`) + tombol "Salin detail".

## 3. Toast

Singkat (maks 60 karakter), hilang setelah 3–4 detik, boleh punya satu aksi. Contoh: "Wallpaper diterapkan [Urungkan]", "Ditambahkan ke favorit", "3 wallpaper dihapus dari library [Urungkan]".
**Urungkan** untuk hapus dari library: tahan penghapusan permanen entri selama durasi toast (soft-delete, hapus nyata setelah toast hilang atau saat GUI ditutup).

## 4. Status daemon di GUI

- GUI berlangganan sinyal D-Bus `StateChanged` dan memperbarui `StatusChip` + indikator "Dipakai" di kartu.
- Bila daemon hilang tiba-tiba (name owner berubah): chip → "Tidak berjalan", tidak ada dialog yang mengganggu. Dialog hanya muncul bila pengguna sedang menerapkan wallpaper.
- Polling dilarang kecuali sebagai cadangan (maks tiap 10 detik) bila sinyal tidak diterima.

## 5. Aksesibilitas

- Semua tombol ikon punya `tooltip-text` dan label aksesibel (`accessible-label`).
- Urutan Tab logis. Fokus selalu terlihat (cincin aksen).
- Kontras teks ≥ 4,5:1, ikon ≥ 3:1.
- Seluruh fungsi bisa dioperasikan tanpa mouse.
- Hormati `gtk-enable-animations` dan ukuran font sistem (jangan memaku ukuran font di container yang tidak melar).
- Teks kartu tidak boleh terpotong tanpa tooltip.

## 6. Internasionalisasi

- Semua string lewat `tr!("...")` (gettext). Kunci = string Inggris, terjemahan Indonesia di `po/id.po`. Karena default Indonesia, pastikan **tidak ada** string yang lolos tanpa terjemahan: tes otomatis yang memeriksa semua string `tr!` punya entri `id`.
- Pluralisasi lewat `ngettext` ("1 wallpaper" / "N wallpaper" — bahasa Indonesia tidak membedakan, tetap sediakan struktur).
- Format angka/ukuran: ukuran file dengan koma desimal di Indonesia (`1,5 MB`).
- Tanggal: relatif ("Hari ini", "Kemarin") untuk < 7 hari, selain itu `dd MMM yyyy`.

## 7. Daftar string kunci (Indonesia) agar konsisten

| Konsep | String |
|---|---|
| Set as wallpaper | Jadikan wallpaper |
| Fill method | Metode pengisian |
| Fill / Fit / Center / Stretch / Tile | Penuhi / Pas / Tengah / Regang / Ubin |
| Transition / None / Fade | Transisi / Tidak ada / Pudar |
| Mute | Bisu |
| Power saving | Hemat daya |
| Default | Bawaan |
| Running / Not running | Berjalan / Tidak berjalan |
| Recent | Terbaru |
| All wallpapers | Semua wallpaper |
| My order | Urutan saya |
| Add folder / From link / Add / Select | Tambah folder / Dari tautan / Tambah / Pilih |
| Rotate 90° / Reset crop | Putar 90° / Setel ulang pangkas |
| Missing | HILANG |
