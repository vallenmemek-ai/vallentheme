# FASE 4 — Halaman Library

**Baca dulu:** `panduan/03-gui-layar-library.md` (seluruhnya), `panduan/02-gui-design-system.md` (6.1–6.5), `panduan/05` (dialog & toast).

## Tujuan
Library fungsional dan cepat walau ribuan item: tampil, cari, sortir, tambah (file/folder/drag-drop), favorit, hapus dari library, mode Pilih. Terhubung ke `core`.

## Ruang lingkup
**Termasuk:** seluruh layar Library kecuali tombol "Dari tautan" (Fase 8) dan tombol "Jadikan wallpaper" yang masih mengarah ke Fase 5 (untuk sekarang menu itu **nonaktif** dengan tooltip).
**Tidak termasuk:** daemon, layar Edit penuh (halaman Edit tetap placeholder yang menampilkan nama item).

## Langkah rinci

### 4.0 Prototipe dan keputusan (batas 1 hari kerja)
Bangun prototipe kecil teknik **baris-terpotong** (`panduan/03` bagian 5). Ukur dengan 2.000 kartu data palsu: gulir mulus? memori? resize? Bila tidak memenuhi target → pakai **cadangan** (GridView + chip filter). Tulis putusan di `docs/ARCH.md`.

### 4.1 Lapisan data GUI
- `WallpaperObject` (GObject) membungkus `core::Wallpaper` + properti reaktif (favorit, hilang, dipakai).
- `LibraryModel`: memegang `gio::ListStore`, memuat dari DB lewat thread kerja (halaman 200), mengirim batch ke main thread.
- Filter (pencarian + folder + favorit) dan sortir dihitung di thread kerja; hasil berupa daftar id terurut. Jangan menyaring di main thread untuk 2.000 item.
- Sumber kebenaran tetap DB. Setelah mutasi (favorit, hapus), perbarui model secara lokal **dan** DB; bila DB gagal, kembalikan state UI dan tampilkan toast error.

### 4.2 Header, pencarian, kontrol
Implementasi sesuai `panduan/03` bagian 3–4: StatusChip (status statis "Tidak berjalan" sampai Fase 5), pencarian debounce 150 ms, popover folder (dengan hitungan), popover sortir. Simpan pilihan di config.

### 4.3 Daftar & kartu
- Model baris (`Header`/`Cards`/`Spacer`) dan `ListView` sesuai putusan 4.0.
- Muat thumbnail lazy: permintaan ke core (prioritas item terlihat), dekode di thread kerja, tekstur di main thread, cache LRU 64, pembatalan saat daur ulang.
- Pembaruan bertahap tanpa loncat posisi gulir.
- Kartu: lencana, meta, favorit, info, menu, penanda "Dipakai" (sementara selalu tersembunyi), label `HILANG`.

### 4.4 Tambah & impor
- `+ Tambah` (file, multi-pilih), `Tambah folder` (dialog opsi), drag & drop (overlay). Semua lewat `core::scan` di thread kerja dengan channel berbatas.
- Banner progres + tombol **Batal**; hasil bertahap (batch 20 item / 200 ms).
- Ringkasan akhir (toast + dialog bila ada yang dilewati). Duplikat diperbarui, bukan digandakan.
- Thumbnail dibuat di latar setelah entri muncul (kartu menampilkan skeleton lalu gambar).

### 4.5 Aksi item
- Favorit (toggle, ikon hover + menu + shortcut `F`).
- Info (dialog tabel metadata, salin lokasi, buka folder lewat `gio::AppInfo::launch_default_for_uri` pada folder induk).
- Hapus dari library: soft-delete + toast **Urungkan** (3–4 detik). Hapus nyata setelah toast hilang/aplikasi ditutup (`purge_deleted`).
- Item hilang: dialog opsi (Cari ulang… / Hapus dari library).
- Menu klik kanan sesuai daftar di `panduan/03` bagian 6.

### 4.6 Mode Pilih
Header/BottomBar berubah (`panduan/03` bagian 8). Aksi massal: Favorit, Hapus dari library (konfirmasi), "Pilih semua". `Esc` keluar.

### 4.7 Keadaan khusus
Library kosong, hasil kosong, memuat awal (skeleton), DB gagal dibuka (dialog dengan opsi "Buat ulang library"), file sistem tidak terbaca.

### 4.8 Keyboard
Semua shortcut `panduan/03` bagian 11. Navigasi panah antarkartu mengikuti urutan visual; `Enter` membuka halaman Edit placeholder.

### 4.9 Pembersihan berkala
Saat start: `verify_files` di latar (batasi 200 item per detik agar tidak membebani disk), `purge_deleted` entri lama, pembersihan cache thumbnail yatim (di latar, prioritas rendah).

## Pengukuran (catat)
- 2.000 item: RSS GUI, FPS gulir (perkiraan visual + `GTK_DEBUG`/profiler bila ada), waktu rebuild saat resize, waktu pencarian.
- Impor 200 file: waktu total, UI tetap responsif (tidak ada freeze > 100 ms).

## Kasus tepi
- Nama sangat panjang / emoji / RTL.
- Impor file yang sama dua kali berturut-turut.
- Menghapus item saat thumbnail sedang dibuat.
- Mengganti sortir/filter saat impor berjalan.
- Folder dengan 5.000 file (tampil bertahap, UI tidak beku).
- Ubah ukuran jendela terus-menerus (tidak ada lonjakan memori).
- Disk penuh saat membuat thumbnail (kartu tetap tampil dengan ikon).
- DB terkunci sementara (retry dengan backoff, toast bila gagal).

## Gate
- [ ] 2.000 item: gulir mulus, RSS GUI < 250 MB, rebuild resize < 16 ms
- [ ] Pencarian tampil < 100 ms setelah debounce (2.000 item)
- [ ] Impor 200 file di latar: UI tetap responsif, bisa dibatalkan, hasil benar
- [ ] Hapus dari library **tidak** menghapus file asli (diverifikasi test + manual)
- [ ] **Urungkan** bekerja; setelah toast hilang entri benar-benar dibersihkan
- [ ] Mode Pilih (favorit/hapus massal) benar
- [ ] Tidak ada warning GTK di terminal; tidak ada kebocoran memori yang terlihat setelah 30 menit pemakaian bebas
- [ ] `scripts/check.sh` hijau

## Artefak
Layar Library lengkap, putusan teknik daftar di `docs/ARCH.md`, angka pengukuran di `docs/PROGRESS.md`.

## Jangan lakukan
Memuat semua thumbnail sekaligus. Menjalankan I/O/DB di main thread. Menambah fitur Edit di fase ini.
