# 04 — GUI: Layar Edit Wallpaper

Referensi: layar "Edit wallpaper" Fresco: pratinjau di kiri, kartu pengaturan di kanan, tombol besar "Jadikan wallpaper".

## 1. Gambaran

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [‹]            Edit wallpaper                                [–][□][×]   │
│          raiden-shogun-underwater-genshin-impact                          │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌──────────────────────┐    ┌──────────────────────────────────────┐   │
│   │                      │    │ Metode pengisian         [ Penuhi ▾ ]│   │
│   │      PRATINJAU       │    │ Transisi                 [ Tidak ada▾]│   │
│   │       (16:9)         │    │ Bisu                           (●──) │   │
│   │                      │    │ Volume          ───────●───────      │   │
│   └──────────────────────┘    │ Hemat daya               [ Bawaan ▾ ]│   │
│    Putar 90°  Setel ulang     └──────────────────────────────────────┘   │
│    pangkas                    ┌──────────────────────────────────────┐   │
│                               │          Jadikan wallpaper            │   │
│                               └──────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
```

## 2. Pohon widget

```
adw::NavigationPage "edit"
└─ adw::ToolbarView
   ├─ top: adw::HeaderBar
   │        ├─ start: tombol Kembali (otomatis dari NavigationView)
   │        └─ title: adw::WindowTitle(title="Edit wallpaper", subtitle=nama_file)
   └─ content: adw::Clamp(max=960) → gtk::Box horizontal (spacing 32, margin 24)
               ├─ PreviewColumn (lebar 320..360, tidak melebar)
               │    ├─ PreviewFrame (kelas vt-preview, rasio 16:9)
               │    └─ gtk::Box horizontal: [Putar 90°] [Setel ulang pangkas]  (vt-flat)
               └─ SettingsColumn (melebar, maks 520)
                    ├─ adw::PreferencesGroup
                    │    ├─ adw::ComboRow  "Metode pengisian"
                    │    ├─ adw::ComboRow  "Transisi"
                    │    ├─ adw::SwitchRow "Bisu"            (hanya video)
                    │    ├─ adw::ActionRow "Volume" + gtk::Scale (hanya video)
                    │    └─ adw::ComboRow  "Hemat daya"
                    ├─ (opsional) adw::ComboRow "Monitor" (muncul bila monitor > 1)
                    └─ gtk::Button.vt-primary.large "Jadikan wallpaper"
```

Titik patah sempit (< 640 px): `PreviewColumn` dan `SettingsColumn` ditumpuk vertikal, pratinjau selebar konten.

## 3. Isi pengaturan (label, subteks, opsi)

| Baris | Subteks | Opsi | Berlaku untuk |
|---|---|---|---|
| **Metode pengisian** | "Cara media mengisi layar" | **Penuhi** (isi penuh, bisa terpangkas), **Pas** (seluruh gambar terlihat, ada bilah hitam), **Tengah** (ukuran asli di tengah), **Regang** (tarik memenuhi layar, rasio berubah), **Ubin** (diulang, **hanya foto**) | Foto dan video |
| **Transisi** | "Wallpaper berganti tanpa efek apa pun." (berubah sesuai pilihan) | **Tidak ada**, **Pudar** (butuh compositor aktif; bila tidak, otomatis "Tidak ada" dengan keterangan) | Foto dan video |
| **Bisu** | — | Sakelar | Video |
| **Volume** | — | Slider 0–100 (tidak aktif bila Bisu menyala) | Video |
| **Hemat daya** | "Bawaan mengikuti Pengaturan; pilihan lain hanya berlaku untuk wallpaper ini" | **Bawaan**, **Selalu jalan**, **Jeda saat fullscreen/maximize**, **Jeda saat baterai**, **Selalu hemat** | Video (untuk foto: baris disembunyikan) |

Aturan tampil:
- Wallpaper **foto**: sembunyikan Bisu, Volume, Hemat daya. "Ubin" tersedia.
- Wallpaper **video**: "Ubin" disembunyikan dari opsi Metode pengisian.
- Bila video **tanpa trek audio** (hasil ffprobe): Bisu dan Volume dinonaktifkan dengan subteks "Video ini tidak memiliki suara".
- Subteks "Transisi" mengikuti pilihan: Tidak ada → "Wallpaper berganti tanpa efek apa pun."; Pudar → "Wallpaper berganti dengan efek memudar."

## 4. Pratinjau

- Menampilkan **frame/thumbnail** (bukan video live) agar ringan. Resolusi sumber: thumbnail 480 px, atau frame 960 px yang dibuat saat layar Edit dibuka (di thread kerja, di-cache).
- Pratinjau **mensimulasikan** pengaturan:
  - Metode pengisian → `gtk::Picture::content_fit`: Penuhi=`Cover`, Pas=`Contain`, Regang=`Fill`, Tengah=`ScaleDown` (perkiraan), Ubin=gambar diulang (cukup tampilkan `Cover`; beri keterangan "Pratinjau perkiraan").
  - Putar 90° → pratinjau memakai pixbuf yang diputar (`gdk_pixbuf::Pixbuf::rotate_simple`).
  - Pangkas (pan/zoom) → geser dan perbesar gambar di dalam frame (lihat bagian 5).
- Rasio frame pratinjau mengikuti **rasio monitor target** (bukan selalu 16:9). Latar `#000` untuk bilah hitam mode Pas.
- Label kecil di sudut pratinjau: resolusi sumber dan jenis (contoh `3840×2160 · Video`).

## 5. Pangkas (pan & zoom) dan Putar

Fresco punya "Putar 90°" dan "Setel ulang pangkas". Di vallentheme:

- **Putar 90°**: tiap klik menambah rotasi 90° searah jarum jam (0 → 90 → 180 → 270 → 0). Disimpan sebagai `rotation` (derajat).
- **Pangkas**: pengguna **menggeser** (drag) pratinjau untuk memindahkan titik fokus dan **menggulir/pinch** untuk memperbesar. Hanya aktif saat mode **Penuhi** (Cover); pada mode lain, geser dinonaktifkan dengan kursor biasa.
  - Disimpan sebagai `zoom` (0.0 = tanpa zoom; skala log2 seperti mpv, rentang 0.0–2.0) dan `pan_x`, `pan_y` (rentang −0.5..0.5, pecahan dari ukuran video, 0 = tengah).
  - Terapkan batas agar tepi gambar tidak pernah menampakkan area kosong (klem pan sesuai zoom).
- **Setel ulang pangkas**: `zoom = 0`, `pan_x = 0`, `pan_y = 0` (rotasi **tidak** direset). Tombol dinonaktifkan bila nilai sudah bawaan.
- Terjemahan ke mpv: `--video-rotate`, `--video-zoom`, `--video-pan-x`, `--video-pan-y` (lihat `06-referensi-teknis.md`). Untuk foto lewat setter desktop yang tidak mendukung pan/zoom: render hasil pangkas menjadi file gambar di cache (ffmpeg `crop`/`scale`) lalu setel file itu sebagai wallpaper, tanpa mengubah file asli.

## 6. Tombol "Jadikan wallpaper"

- Lebar penuh kolom pengaturan, tinggi 48, radius 14, kelas `vt-primary large`.
- Saat diklik: tombol menampilkan spinner dan dinonaktifkan sampai daemon membalas.
- Sukses: toast "Wallpaper diterapkan" (aksi: "Urungkan" mengembalikan wallpaper sebelumnya bila ada). Gagal: toast/dialog dengan pesan ramah + kode (lihat katalog error Fase 9).
- Multi-monitor: baris **Monitor** (ComboRow) berisi "Semua monitor", "Monitor 1 (HDMI-1, 1920×1080)", dst. Default "Semua monitor". Pilihan terakhir diingat per wallpaper.
- Bila wallpaper ini sudah aktif dengan pengaturan sama: tombol tetap aktif ("Terapkan ulang").

## 7. Penyimpanan otomatis & penerapan langsung

- Setiap perubahan pengaturan disimpan otomatis ke `settings_json` (debounce **400 ms**). Tidak ada tombol "Simpan".
- Bila wallpaper ini sedang aktif, perubahan **langsung diterapkan** ke daemon lewat `Apply` ringan (hanya properti yang berubah, melalui `set_property` mpv bila memungkinkan, tanpa memulai ulang).
- Nilai tidak valid dikoreksi (klem) sebelum disimpan.
- Meninggalkan layar tidak perlu konfirmasi.

## 8. Keadaan dan kasus tepi

| Kasus | Perilaku |
|---|---|
| File hilang saat layar dibuka | Banner di atas "File tidak ditemukan" + tombol "Cari ulang…"; tombol Jadikan wallpaper nonaktif |
| Thumbnail gagal dibuat | Pratinjau menampilkan ikon jenis media + pesan kecil; pengaturan tetap berfungsi |
| Video tanpa audio | Bisu/Volume nonaktif (bagian 3) |
| Video sangat panjang/besar | Tidak ada efek di layar ini (pratinjau hanya frame) |
| Daemon tidak bisa dihubungi | Toast error + tombol "Coba lagi"; pengaturan tersimpan tetap aman |
| Monitor dicabut saat layar terbuka | Perbarui daftar Monitor, kembalikan pilihan ke "Semua monitor" |
| Mode Penuhi + zoom lalu ganti ke Pas | Simpan nilai zoom/pan, nonaktifkan interaksi; kembali aktif bila pilih Penuhi lagi |

## 9. Keyboard

`Esc`/`Alt+←` kembali · `R` putar 90° · `Ctrl+Enter` Jadikan wallpaper · `Ctrl+0` setel ulang pangkas · Tab berpindah antar baris pengaturan.

## 10. Target kinerja
- Layar terbuka < 150 ms (thumbnail langsung dari cache), frame pratinjau resolusi tinggi muncul bertahap.
- Menggeser pratinjau tetap 60 fps pada renderer cairo (gunakan gambar kecil, jangan render ulang tiap gerakan penuh).
