# FASE 3 — Kerangka GUI & Tema

**Baca dulu:** `panduan/02-gui-design-system.md` (seluruhnya), `panduan/03-gui-layar-library.md` (bagian 1–3), `panduan/05-gui-pengaturan-dialog-status.md` (bagian 5–7).

## Tujuan
Jendela jadi dengan tema hitam modern, navigasi antarhalaman, komponen dasar, dan sistem terjemahan. Isi halaman masih **placeholder**.

## Ruang lingkup
**Termasuk:** aplikasi `adw::Application`, jendela, header, NavigationView, tema/CSS, resource, ikon, i18n, komponen kustom (`StatusChip`, `WallpaperCard`, `BottomBar`, `EmptyState`), breakpoint responsif.
**Tidak termasuk:** data nyata, DB, D-Bus (kartu memakai data palsu).

## Langkah rinci

### 3.1 Aplikasi
- `adw::Application` dengan `application_id = "id.vallentheme.App"`, flag `DEFAULT_FLAGS` (instance tunggal otomatis lewat GApplication).
- `startup`: `adw::StyleManager::default().set_color_scheme(ForceDark)`, muat CSS, daftarkan ikon, siapkan aksi global (`app.quit`, `app.preferences`, `app.about`) + shortcut.
- `activate`: bila jendela ada → `present()`, jika tidak buat baru.
- Simpan ukuran/posisi jendela di config (`ui.window`), pulihkan saat start (klem ke layar yang ada).

### 3.2 Resource
- `data/vallentheme.gresource.xml` memuat `style.css`, ikon simbolik yang dibundel, dan (nanti) UI blueprint bila dipakai.
- Compile lewat `glib-build-tools` di `build.rs`. Tanpa blueprint-compiler/Cargo tambahan: bangun widget **dengan kode Rust** (lebih sedikit dependensi, lebih mudah diaudit agent).
- Ikon: daftarkan `IconTheme::for_display().add_resource_path("/id/vallentheme/icons")`.

### 3.3 Tema
- Terapkan CSS dari `panduan/02-gui-design-system.md` bagian 9 (pindahkan ke `data/style.css`).
- Prioritas provider: `STYLE_PROVIDER_PRIORITY_APPLICATION`.
- Verifikasi di dua renderer: `GSK_RENDERER=ngl` dan `GSK_RENDERER=cairo`. Catat bila ada perbedaan tampilan.
- Tes kontras: cek manual pasangan `text/bg`, `text-muted/surface` ≥ 4,5:1.

### 3.4 Jendela & navigasi
- `adw::ApplicationWindow` default 960×660, minimum 420×520.
- Struktur: `ToastOverlay` → `NavigationView` → halaman `library` (placeholder), `edit` (placeholder). Pengaturan memakai `adw::PreferencesDialog` (kerangka kosong dengan tiga halaman).
- `adw::Breakpoint` untuk lebar < 640 px: tandai jendela dengan kelas `narrow` (komponen menyesuaikan lewat CSS/properti).

### 3.5 Komponen kustom
Tiap komponen dalam modul sendiri, subclass GObject (`glib::wrapper!`) bila perlu properti/sinyal:
- **`StatusChip`**: properti `state` (enum), memperbarui kelas CSS `running/paused/degraded/failed`, tooltip. Sinyal `clicked`.
- **`WallpaperCard`**: lapisan seperti `panduan/02` 6.2; properti `title`, `meta`, `kind`, `resolution_badge`, `favorite`, `missing`, `in_use`, `selected`, `thumbnail` (`gdk::Paintable`). Sinyal `activated`, `favorite-toggled`, `info-requested`, `menu-requested`.
- **`BottomBar`**: hitungan + 4 tombol, mode normal/pilih.
- **`EmptyState`**: pembungkus `adw::StatusPage` dengan gaya ringan.
Untuk sekarang, isi halaman Library dengan 12 kartu data palsu agar tampilan bisa dinilai.

### 3.6 i18n
- `gettext-rs`: `bindtextdomain`, `textdomain`, fungsi/makro `tr!`. Berkas `po/id.po` berisi semua string UI saat ini. `po/POTFILES` dan skrip `scripts/update-po.sh` (xgettext).
- Tes otomatis (`cargo test` di crate gui, tanpa display) yang memindai kode untuk `tr!("...")` dan memastikan tiap string ada di `id.po`.
- Bahasa dipilih dari config (`system` → locale lingkungan).

### 3.7 Shortcut & aksi
Daftarkan akselerator: `Ctrl+Q`, `Ctrl+,`, `Ctrl+F` (placeholder). Gunakan `gio::SimpleAction` + `app.set_accels_for_action`.

### 3.8 Tentang
`adw::AboutDialog` dengan nama, versi (`env!("CARGO_PKG_VERSION")`), lisensi (sementara "TBD"), tautan kosong, tombol "Salin info debug" (versi GTK/libadwaita, OS, DE, sesi).

## Pengukuran
```bash
# RAM idle GUI
./target/release/vallentheme-gui & sleep 10; ps -o rss= -p $!
# CPU idle
pidstat -p $! 5 3
```
Bandingkan `ngl` vs `cairo`. Catat di `docs/ARCH.md`.

## Kasus tepi
- Layar kecil (1024×600) dan jendela diperkecil sampai minimum.
- Skala tampilan 1x dan 2x (HiDPI), ukuran font sistem besar.
- Ikon tidak ditemukan di tema sistem (harus tetap tampil dari resource).
- Dijalankan di terminal tanpa `DISPLAY` → pesan jelas dan keluar kode ≠ 0, bukan panic.
- Instance kedua: hanya mengangkat jendela pertama.

## Gate
- [ ] Aplikasi terbuka dengan tema gelap benar di `GSK_RENDERER=ngl` **dan** `cairo`
- [ ] RAM idle < 150 MB dan CPU ~0% (diukur, tercatat)
- [ ] Instance kedua hanya mengangkat jendela pertama
- [ ] Tidak ada warning GTK/GLib/Adwaita di terminal pada pemakaian normal
- [ ] Semua ikon tampil walau tema sistem tidak punya
- [ ] Test i18n lulus (tidak ada string tanpa terjemahan Indonesia)
- [ ] Responsif: tata letak benar di 420 px, 640 px, dan 960 px
- [ ] `scripts/check.sh` hijau

## Artefak
Crate `gui` dengan kerangka, `style.css`, resource, komponen, `po/id.po`.

## Jangan lakukan
Menyentuh database atau daemon. Menambah animasi/efek di luar `panduan/02` bagian 7.
