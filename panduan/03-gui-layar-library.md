# 03 — GUI: Layar Library (halaman utama)

Referensi: layar "Fresco" dengan bagian TERBARU dan VIDEO. Susunan sama, tampilan lebih gelap dan rapi.

## 1. Gambaran

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [● Berjalan]                    vallentheme                     [⋯] [–][□][×]│  ← HeaderBar
├──────────────────────────────────────────────────────────────────────────┤
│                    ┌───────────────────────────────┐                       │
│                    │ 🔍 Cari wallpaper atau folder… │                       │  ← Pencarian
│                    └───────────────────────────────┘                       │
│  [📁 Semua wallpaper ▾]                              [⇅ Urutan saya ▾]     │  ← Kontrol
│                                                                            │
│  TERBARU                                                                   │
│  [kartu][kartu][kartu]                                                     │
│  VIDEO                                                                     │
│  [kartu][kartu][kartu]                                                     │
│  FOTO                                                                      │
│  [kartu][kartu][kartu]                                                     │
├──────────────────────────────────────────────────────────────────────────┤
│ 24 wallpaper        [Tambah folder] [Dari tautan] [+ Tambah] [✓ Pilih]     │  ← Bilah bawah
└──────────────────────────────────────────────────────────────────────────┘
```

## 2. Pohon widget (acuan)

```
adw::ApplicationWindow
└─ adw::ToastOverlay
   └─ adw::NavigationView
      └─ adw::NavigationPage "library"
         └─ adw::ToolbarView
            ├─ top bar:    adw::HeaderBar
            │                 ├─ start: StatusChip
            │                 ├─ title: Label "vallentheme" (kelas vt-header-title)
            │                 └─ end:   MenuButton (⋯)
            ├─ content:    gtk::Box (vertical)
            │                 ├─ SearchRow   (gtk::SearchEntry di dalam clamp 560)
            │                 ├─ ControlRow  (FolderButton kiri, SortButton kanan)
            │                 └─ gtk::ScrolledWindow
            │                       └─ LibraryList (lihat bagian 5)
            └─ bottom bar: BottomBar (hitungan + 4 tombol)
```

Mode Pilih mengganti isi HeaderBar dan BottomBar (bagian 8).

## 3. Header

- Kiri: **StatusChip** (komponen 6.1). Klik → popover status + tombol Mulai/Hentikan/Mulai ulang.
- Tengah: judul "vallentheme".
- Kanan: tombol menu `⋯` → `Pengaturan`, `Tentang vallentheme`, `Hentikan wallpaper`, `Keluar`.
  - "Hentikan wallpaper" memanggil `Stop` di daemon. "Keluar" menutup GUI saja (daemon tetap jalan). Tambahkan juga "Keluar dan hentikan daemon".

## 4. Pencarian dan kontrol

### Pencarian
- `gtk::SearchEntry` dalam `adw::Clamp` (maks 560). Placeholder: "Cari wallpaper atau folder…".
- Debounce **150 ms**. Mencocokkan: nama, nama folder, jenis (`video`, `foto`), resolusi (`4k`), tanda `fav`/`favorit`.
- `Ctrl+F` memfokuskan kolom. `Esc` mengosongkan lalu melepas fokus.
- Hasil kosong → keadaan kosong "Tidak ada hasil untuk 'xxx'" dengan tombol "Hapus pencarian".

### Pemilih folder ("Semua wallpaper")
- Tombol dengan ikon folder + teks + panah. Popover berisi daftar: **Semua wallpaper**, **Favorit**, lalu folder terdaftar (nama + jumlah item).
- Pilihan tersimpan di config (`ui.last_folder`).

### Sortir ("Urutan saya")
Popover opsi (radio): **Urutan saya** (urutan manual/tambah), **Terbaru ditambahkan**, **Terakhir dipakai**, **Nama (A–Z)**, **Ukuran file**, **Resolusi**, **Favorit dulu**.
- "Urutan saya" = urutan stabil berdasarkan `sort_index` (default: urutan ditambahkan). Seret-untuk-urutkan tidak ada di v0.1.
- Pilihan tersimpan di config (`ui.sort`).

## 5. Daftar wallpaper (inti performa)

### Struktur bagian
Urutan: **TERBARU** (maks 1 baris, 6 item terbaru yang ditambahkan/dipakai), **VIDEO**, **FOTO**. Bagian kosong disembunyikan. Saat pencarian aktif, bagian TERBARU disembunyikan dan hasil ditampilkan satu daftar bagian-bagian tetap (VIDEO/FOTO).

### Teknik virtualisasi: "baris-terpotong" (row chunking)
`GtkGridView` tidak mendukung judul bagian. Karena itu:

1. Model sumber: daftar `WallpaperObject` yang sudah difilter dan diurutkan.
2. Bentuk **model baris** (`RowObject`) dengan tiga tipe: `Header(judul)`, `Cards(Vec<WallpaperObject>)`, `Spacer`.
3. Jumlah kolom `N = max(1, floor((lebar_konten + gap) / (232 + gap)))`, `gap = 16`. Hitung ulang saat lebar berubah (debounce 50 ms) dan bangun ulang model baris.
4. Tampilkan dengan satu `gtk::ListView` + `SignalListItemFactory`. Setiap item `Cards` berisi `gtk::Box` horizontal homogen berisi `N` `WallpaperCard` (kartu kosong disembunyikan bila baris terakhir kurang dari N, lebar tetap sama).
5. Karena `ListView` melakukan daur ulang widget, jumlah widget di memori tetap kecil walau ribuan item.
6. Jaga posisi gulir saat rebuild: simpan id item paling atas yang terlihat, kembalikan setelah rebuild.

**Cadangan** (bila teknik di atas bermasalah dalam batas waktu 1 hari kerja): satu `GtkGridView` dengan chip filter **Semua | Video | Foto** di atas (tanpa judul bagian) dan baris TERBARU sebagai `FlowBox` tetap di atas area gulir. Putuskan di awal Fase 4 setelah prototipe kecil, catat di `docs/ARCH.md`.

### Pemuatan thumbnail
- Dimuat **lazy** saat kartu terlihat (di `bind` pada factory), dibatalkan bila kartu didaur ulang (`gio::Cancellable` per item).
- Decode di thread kerja (gdk-pixbuf aman antar-thread), buat `gdk::MemoryTexture` di main thread.
- Cache memori LRU maksimal **64 tekstur**. Thumbnail 480 px lebar (disk) diskalakan ke ukuran kartu.
- Selama memuat: `wp-skeleton` berdenyut.
- Bila thumbnail belum ada di disk: minta core membuatnya (antrean dengan prioritas: item yang terlihat lebih dulu).

## 6. Kartu wallpaper (detail perilaku)

| Elemen | Isi |
|---|---|
| Thumbnail | `gtk::Picture`, `Cover`, `can_shrink` |
| Lencana kiri atas | `VIDEO` (untuk video), lencana resolusi (`4K`/`2K`/`FHD`), `HILANG` bila file tidak ada |
| Penanda kanan atas | "Dipakai" (chip aksen kecil) bila sedang jadi wallpaper di monitor mana pun |
| Teks kiri bawah | Nama (tanpa ekstensi), baris meta `4K · 24fps · 15 MB` (foto: `3840×2160 · 2,1 MB`) |
| Aksi kanan bawah (hover/fokus) | ♥ favorit, ⓘ info, `⋯` menu |
| Centang (mode Pilih) | Lingkaran 22 px kiri atas, terisi aksen bila terpilih |

Interaksi:
- **Klik/Enter**: buka layar Edit. Pada mode Pilih: toggle pilih.
- **Klik kanan / tombol `⋯`**: `PopoverMenu` berisi: *Jadikan wallpaper*, *Edit*, *Favorit/Hapus favorit*, *Info*, *Buka folder*, *Salin lokasi file*, *Hapus dari library*.
- **Double-click** (opsional): langsung "Jadikan wallpaper" dengan pengaturan tersimpan.
- Item `missing`: klik menampilkan dialog "File tidak ditemukan" dengan opsi *Cari ulang file…*, *Hapus dari library*, *Batal*.
- Tooltip pada nama panjang menampilkan nama penuh.

## 7. Bilah bawah

Kiri: hitungan "N wallpaper" (saat filter aktif: "N dari M wallpaper").
Kanan (berurutan): **Tambah folder** (ikon + teks, sekunder), **Dari tautan** (sekunder), **+ Tambah** (utama), **Pilih** (sekunder, toggle).

| Tombol | Aksi |
|---|---|
| Tambah folder | `gtk::FileDialog::select_folder` → dialog opsi (sertakan subfolder, jenis file) → impor dengan progres |
| Dari tautan | Dialog "Tambah dari tautan" (Fase 8). Sebelum Fase 8: tombol dinonaktifkan dengan tooltip "Segera hadir" |
| + Tambah | `gtk::FileDialog::open_multiple` dengan filter media → impor |
| Pilih | Masuk mode Pilih |

Responsif (< 640 px): tombol hanya berupa ikon dengan tooltip; hitungan disembunyikan.

## 8. Mode Pilih

- Header: tombol "Batal" di kiri, judul "N dipilih" di tengah, tombol "Pilih semua" di kanan.
- Bilah bawah: **Favorit**, **Hapus dari library** (merah, dengan konfirmasi), **Jadikan wallpaper** (aktif bila tepat 1 terpilih).
- Kartu menampilkan centang; klik kartu = toggle. `Ctrl+A` pilih semua, `Esc` keluar.
- Konfirmasi hapus: `AlertDialog` "Hapus N wallpaper dari library?" + keterangan "File asli di disk tidak akan dihapus."

## 9. Impor (file/folder/drag & drop)

1. Terima `gio::File` dari dialog atau drop (`gtk::DropTarget` pada jendela, tipe `gdk::FileList`).
2. Saring ekstensi yang didukung dan tolak sisanya dengan ringkasan ("3 file dilewati: format tidak didukung").
3. Kirim ke core (di thread kerja). Tampilkan **banner progres** tipis di bawah kontrol: "Menambahkan 12 dari 87…" + tombol **Batal**.
4. Item muncul bertahap di daftar (batch 20 item atau 200 ms, mana lebih cepat) tanpa membuat daftar loncat.
5. Selesai: toast "87 wallpaper ditambahkan" (+ "N dilewati" bila ada).
6. Duplikat (path sama): perbarui entri, jangan gandakan.
7. Drag & drop: saat file melayang di jendela, tampilkan overlay "Lepas untuk menambahkan" (border putus-putus aksen, tanpa blur).

## 10. Keadaan khusus

| Keadaan | Tampilan |
|---|---|
| Library kosong | `adw::StatusPage`: ikon besar, judul "Belum ada wallpaper", teks "Tambahkan foto atau video untuk memulai", tombol utama **+ Tambah**, tombol sekunder **Tambah folder** |
| Hasil cari kosong | `StatusPage` kecil "Tidak ada hasil" + "Hapus pencarian" |
| Sedang memuat library awal | Skeleton kartu (6 buah) |
| Daemon tidak berjalan | Chip "Tidak berjalan"; menerapkan wallpaper akan menyalakannya otomatis |
| DB gagal dibuka | Dialog error dengan kode, tombol "Salin detail", "Buat ulang library" (dengan konfirmasi), "Keluar" |

## 11. Keyboard

`Ctrl+F` cari · `Ctrl+O` tambah file · `Ctrl+Shift+O` tambah folder · `Ctrl+L` dari tautan · `Ctrl+A` (mode Pilih) · `Delete` hapus dari library (dengan konfirmasi) · `F` favorit item terfokus · `Enter` buka Edit · `Ctrl+,` Pengaturan · `Ctrl+Q` keluar · `Esc` batal/keluar mode/mengosongkan pencarian · Panah: navigasi antar kartu (urutan visual).

## 12. Target kinerja layar ini

- 2.000 item: gulir tanpa patah-patah terlihat, memori GUI < 250 MB.
- Pencarian mulai menampilkan hasil < 100 ms setelah debounce di 2.000 item.
- Membangun ulang model baris saat resize < 16 ms untuk 2.000 item (profilkan, optimalkan bila perlu).
