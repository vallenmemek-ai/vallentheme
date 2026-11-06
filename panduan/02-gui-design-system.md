# 02 — GUI Design System

Tujuan visual: **mirip Fresco secara susunan dan alur, tetapi lebih gelap, lebih modern, lebih rapi, dan lebih ringan.**
Orang yang pernah memakai Fresco harus langsung paham, namun terasa seperti produk baru yang lebih matang.

## 1. Prinsip

1. **Hitam pekat** dengan permukaan bertingkat tipis (bukan abu-abu datar). Kontras tinggi, tepi halus.
2. **Satu warna aksen** (indigo). Warna lain hanya untuk status (sukses, peringatan, bahaya).
3. **Gambar adalah pahlawan.** Thumbnail menempati ruang terbesar, UI di sekitarnya tenang.
4. **Ringan di GPU lemah.** Tanpa blur, tanpa `backdrop-filter`, tanpa bayangan besar, animasi singkat.
5. **Konsisten.** Semua jarak kelipatan 4. Semua radius dari daftar token. Semua warna dari token.
6. **Bahasa Indonesia** sebagai default, nada ramah dan singkat.

## 2. Token warna

| Token | Hex | Dipakai untuk |
|---|---|---|
| `bg` | `#0A0A0D` | Latar jendela |
| `surface` | `#131318` | Kartu, panel, kartu pengaturan |
| `surface-2` | `#1B1B22` | Hover, input, baris terpilih |
| `surface-3` | `#24242D` | Tekan (active), chip nonaktif |
| `border` | `rgba(255,255,255,0.06)` | Garis tipis kartu/panel |
| `border-strong` | `rgba(255,255,255,0.12)` | Fokus input, pemisah penting |
| `text` | `#ECECF1` | Teks utama |
| `text-muted` | `#8B8B99` | Teks sekunder (kontras ≥ 4,5:1 di atas `bg` dan `surface`) |
| `text-faint` | `#5E5E6B` | Placeholder, hanya untuk teks non-kritis |
| `accent` | `#6C63FF` | Tombol utama, sakelar aktif, fokus, penanda terpilih |
| `accent-hover` | `#7F78FF` | Hover aksen |
| `accent-press` | `#5A52E6` | Tekan aksen |
| `accent-soft` | `rgba(108,99,255,0.16)` | Latar chip/penanda aksen halus |
| `success` | `#3DDC97` | Status Berjalan |
| `warning` | `#FFB74D` | Status Dijeda/Degraded |
| `danger` | `#FF5C6C` | Error, hapus |

Pemetaan ke nama warna libadwaita (didefinisikan lewat `@define-color`):

| Nama libadwaita | Token |
|---|---|
| `window_bg_color`, `view_bg_color`, `dialog_bg_color`, `popover_bg_color` (sedikit lebih terang: `surface`) | `bg` / `surface` |
| `window_fg_color`, `view_fg_color`, `card_fg_color`, `headerbar_fg_color` | `text` |
| `card_bg_color` | `surface` |
| `headerbar_bg_color`, `headerbar_backdrop_color` | `bg` |
| `headerbar_border_color`, `card_shade_color` | `border` |
| `accent_bg_color`, `accent_color` | `accent` |
| `accent_fg_color` | `#FFFFFF` |
| `destructive_bg_color` | `danger` |
| `success_bg_color` | `success` |
| `warning_bg_color` | `warning` |

## 3. Tipografi

Pakai font sistem (jangan membundel font). Urutan preferensi: Inter, Cantarell, sans-serif.

| Peran | Ukuran | Bobot | Catatan |
|---|---|---|---|
| Judul header | 15 px | 700 | Tengah header bar |
| Subjudul header | 11 px | 400 | `text-muted`, satu baris, elipsis tengah |
| Judul bagian (TERBARU, VIDEO) | 11 px | 700 | HURUF KAPITAL ditulis langsung di string, `letter-spacing: 0.08em`, `text-muted` |
| Nama kartu | 13 px | 600 | Satu baris, elipsis akhir |
| Meta kartu | 11 px | 400 | `text-muted`, contoh `4K · 24fps · 15 MB` |
| Lencana | 10 px | 700 | `VIDEO`, `4K`, `FOTO`, `HILANG` |
| Label baris pengaturan | 14 px | 500 | |
| Subteks baris | 12 px | 400 | `text-muted` |
| Tombol utama | 14 px | 600 | |
| Teks isi | 14 px | 400 | |

Hindari `text-transform` (dukungan GTK bervariasi); tulis huruf kapital langsung di string terjemahan.

## 4. Spasi, radius, ukuran

- Skala spasi: `4, 8, 12, 16, 20, 24, 32, 40`.
- Radius: kartu **14**, panel/kartu pengaturan **14**, tombol/input **10**, chip/lencana **999** (pil) atau **6** (lencana persegi kecil), pratinjau **12**.
- Tinggi kontrol: input pencarian **40**, tombol bilah bawah **38**, tombol utama **48**, chip status **30**.
- Garis tepi kartu: 1 px `border`. Tanpa bayangan. (Bila terasa datar, naikkan `surface` sedikit, bukan menambah bayangan.)
- Jendela: default **960×660**, minimum **420×520**. Titik patah sempit: lebar **< 640 px**.

## 5. Ikon

Gunakan ikon simbolik (`*-symbolic`) dari tema Adwaita:

| Fungsi | Nama ikon |
|---|---|
| Cari | `system-search-symbolic` (atau `edit-find-symbolic`) |
| Folder | `folder-symbolic`, `folder-new-symbolic` |
| Sortir | `view-sort-descending-symbolic` |
| Favorit (kosong/isi) | `non-starred-symbolic` / `starred-symbolic` |
| Info | `dialog-information-symbolic` |
| Lainnya | `view-more-symbolic` |
| Tambah | `list-add-symbolic` |
| Tautan | `insert-link-symbolic` |
| Pilih | `object-select-symbolic` |
| Putar 90° | `object-rotate-right-symbolic` |
| Kembali | `go-previous-symbolic` |
| Hapus | `user-trash-symbolic` |
| Peringatan | `dialog-warning-symbolic` |
| Video | `video-x-generic-symbolic` |
| Foto | `image-x-generic-symbolic` |

**Penting:** tema ikon MATE bisa tidak punya semua ikon di atas. Solusi: bundel ikon yang dipakai ke dalam **GResource** (folder `data/icons/hicolor/symbolic/actions/`) dan daftarkan lewat `IconTheme::add_resource_path`. Catat lisensi ikon Adwaita (LGPL-3.0 / CC-BY-SA-3.0) di `docs/`.

## 6. Komponen

### 6.1 Chip status (`StatusChip`)
Pil tinggi 30 px, latar `surface-2`, titik 8 px di kiri + teks 12 px.
| Status | Titik | Teks |
|---|---|---|
| Berjalan | `success` | "Berjalan" |
| Tidak berjalan | `text-faint` | "Tidak berjalan" |
| Dijeda | `warning` | "Dijeda" (tooltip alasan) |
| Terbatas (Degraded) | `warning` | "Terbatas" (tooltip level dan alasan) |
| Gagal | `danger` | "Gagal" (tooltip kode error) |
Klik: popover kecil berisi status rinci, tombol **Mulai/Hentikan/Mulai ulang**.

### 6.2 Kartu wallpaper (`WallpaperCard`)
- Rasio **16:9**. Lebar minimum **232 px**, melebar mengisi baris. Radius 14, `overflow: hidden`.
- Lapisan (dari bawah): (1) thumbnail `gtk::Picture` dengan `content_fit = Cover`; (2) gradien gelap bawah (tinggi ±45%) `linear-gradient(to top, rgba(0,0,0,.78), rgba(0,0,0,0))`; (3) teks nama + meta di kiri bawah; (4) lencana di kiri atas; (5) aksi hover di kanan bawah; (6) penanda "Dipakai" di kanan atas.
- Keadaan: normal, hover/fokus (aksi muncul, tepi `border-strong`), terpilih mode Pilih (cincin aksen 2 px + centang di kiri atas), sedang dipakai (cincin aksen 2 px + chip "Dipakai"), hilang (thumbnail digelapkan 60% + lencana `HILANG` merah), memuat (blok `surface-2` berdenyut halus 1,2 detik, hanya `opacity`).
- Aksi hover: ♥ favorit (toggle), ⓘ info, `⋯` menu. Tombol bulat 30 px, latar `rgba(0,0,0,.55)`, ikon putih.
- Seluruh kartu bisa diklik (buka layar Edit), bisa difokus keyboard, `Enter` membuka.

### 6.3 Lencana
Persegi kecil radius 6, tinggi 18, padding 0 6, latar `rgba(0,0,0,.55)`, teks 10 px bold putih. Lencana `4K`/`HD`: latar `accent`. `HILANG`: latar `danger`.
Aturan resolusi: ≥ 3840 lebar → `4K`; ≥ 2560 → `2K`; ≥ 1920 → `FHD`; lainnya tanpa lencana resolusi.

### 6.4 Tombol
| Jenis | Gaya |
|---|---|
| Utama (`.vt-primary`) | Latar `accent`, teks putih, radius 10 (besar: 14), hover `accent-hover`, tekan `accent-press` |
| Sekunder (`.vt-secondary`) | Latar `surface-2`, teks `text`, tepi `border`, hover `surface-3` |
| Teks (`.vt-flat`) | Tanpa latar, teks `text`, hover latar `surface-2` |
| Bahaya (`.vt-danger`) | Latar `danger` (hanya di dialog konfirmasi) |
Fokus keyboard: cincin 2 px `accent` dengan offset 2 px.

### 6.5 Input pencarian
Tinggi 40, radius 12, latar `surface`, tepi `border`, ikon cari di kiri, tombol hapus (×) di kanan saat ada teks. Fokus: tepi `accent`. Lebar maks 560 dan di tengah.

### 6.6 Kartu pengaturan (grup baris)
`adw::PreferencesGroup` dengan baris `ComboRow`, `SwitchRow`, `ActionRow` + `Scale`. Latar `surface`, radius 14, pemisah antar baris `border`.

## 7. Gerak (animasi)

- Hanya `opacity` dan `transform`. Durasi **≤ 150 ms**, easing `ease-out`.
- Hover kartu: aksi muncul (opacity 0→1, 120 ms).
- Perpindahan halaman: animasi bawaan `adw::NavigationView`.
- Hormati pengaturan sistem "kurangi animasi" (`gtk-enable-animations`): bila mati, matikan semua transisi kustom.
- Tidak ada animasi yang berjalan terus-menerus kecuali indikator progres dan placeholder memuat.

## 8. Cara menerapkan tema

1. Di `app.rs` saat `startup`: `adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark)`.
2. Muat `style.css` dari GResource ke `gtk::CssProvider`, daftarkan dengan prioritas `gtk::STYLE_PROVIDER_PRIORITY_APPLICATION` ke `gdk::Display::default()`.
3. Warna libadwaita ditimpa dengan `@define-color` di bagian atas CSS. (Bila versi libadwaita ≥ 1.6 boleh memakai variabel `:root { --window-bg-color: ... }`; pakai `@define-color` agar kompatibel luas.)
4. Kelas CSS kustom diawali `vt-` / `wp-` agar tidak bentrok.

## 9. CSS awal (titik mulai, boleh disempurnakan, token tetap)

```css
/* ===== Warna dasar ===== */
@define-color window_bg_color #0A0A0D;
@define-color window_fg_color #ECECF1;
@define-color view_bg_color #0A0A0D;
@define-color view_fg_color #ECECF1;
@define-color headerbar_bg_color #0A0A0D;
@define-color headerbar_backdrop_color #0A0A0D;
@define-color headerbar_fg_color #ECECF1;
@define-color headerbar_border_color rgba(255,255,255,0.06);
@define-color card_bg_color #131318;
@define-color card_fg_color #ECECF1;
@define-color popover_bg_color #131318;
@define-color popover_fg_color #ECECF1;
@define-color dialog_bg_color #131318;
@define-color dialog_fg_color #ECECF1;
@define-color accent_bg_color #6C63FF;
@define-color accent_color #7F78FF;
@define-color accent_fg_color #FFFFFF;
@define-color destructive_bg_color #FF5C6C;
@define-color success_bg_color #3DDC97;
@define-color warning_bg_color #FFB74D;

/* ===== Header ===== */
headerbar { box-shadow: none; border-bottom: 1px solid rgba(255,255,255,0.06); }
.vt-header-title { font-size: 15px; font-weight: 700; }
.vt-header-subtitle { font-size: 11px; color: #8B8B99; }

/* ===== Status chip ===== */
.vt-chip { background: #1B1B22; border-radius: 999px; padding: 0 12px; min-height: 30px; font-size: 12px; }
.vt-chip .dot { min-width: 8px; min-height: 8px; border-radius: 999px; background: #5E5E6B; }
.vt-chip.running .dot { background: #3DDC97; }
.vt-chip.paused .dot, .vt-chip.degraded .dot { background: #FFB74D; }
.vt-chip.failed .dot { background: #FF5C6C; }

/* ===== Pencarian ===== */
.vt-search { min-height: 40px; border-radius: 12px; background: #131318; border: 1px solid rgba(255,255,255,0.06); padding: 0 12px; }
.vt-search:focus-within { border-color: #6C63FF; }

/* ===== Judul bagian ===== */
.vt-section-title { font-size: 11px; font-weight: 700; letter-spacing: 0.08em; color: #8B8B99; }

/* ===== Kartu wallpaper ===== */
.wp-card { background: #131318; border-radius: 14px; border: 1px solid rgba(255,255,255,0.06); overflow: hidden; }
.wp-card:hover, .wp-card:focus-within { border-color: rgba(255,255,255,0.12); }
.wp-card.in-use { border: 2px solid #6C63FF; }
.wp-card.selected { border: 2px solid #6C63FF; }
.wp-card.missing .wp-thumb { opacity: 0.4; }
.wp-card .wp-shade { background-image: linear-gradient(to top, rgba(0,0,0,0.78), rgba(0,0,0,0)); }
.wp-card .wp-name { font-size: 13px; font-weight: 600; color: #FFFFFF; }
.wp-card .wp-meta { font-size: 11px; color: rgba(255,255,255,0.72); }
.wp-card .wp-actions { opacity: 0; transition: opacity 120ms ease-out; }
.wp-card:hover .wp-actions, .wp-card:focus-within .wp-actions { opacity: 1; }
.wp-actions button { min-width: 30px; min-height: 30px; padding: 0; border-radius: 999px; background: rgba(0,0,0,0.55); color: #FFFFFF; }
.wp-actions button:hover { background: rgba(0,0,0,0.75); }
.wp-badge { background: rgba(0,0,0,0.55); color: #FFFFFF; border-radius: 6px; padding: 0 6px; min-height: 18px; font-size: 10px; font-weight: 700; }
.wp-badge.res { background: #6C63FF; }
.wp-badge.missing { background: #FF5C6C; }
.wp-skeleton { background: #1B1B22; }

/* ===== Bilah bawah ===== */
.vt-bottombar { background: #0A0A0D; border-top: 1px solid rgba(255,255,255,0.06); padding: 8px 16px; }
.vt-count { font-size: 12px; color: #8B8B99; }

/* ===== Tombol ===== */
button.vt-primary { background: #6C63FF; color: #FFFFFF; border-radius: 10px; font-weight: 600; box-shadow: none; }
button.vt-primary:hover { background: #7F78FF; }
button.vt-primary:active { background: #5A52E6; }
button.vt-primary.large { min-height: 48px; border-radius: 14px; font-size: 14px; }
button.vt-secondary { background: #1B1B22; border: 1px solid rgba(255,255,255,0.06); border-radius: 10px; box-shadow: none; }
button.vt-secondary:hover { background: #24242D; }
button.vt-flat { background: transparent; box-shadow: none; border-radius: 10px; }
button.vt-flat:hover { background: #1B1B22; }
button:focus-visible { outline: 2px solid #6C63FF; outline-offset: 2px; }

/* ===== Pratinjau di layar Edit ===== */
.vt-preview { border-radius: 12px; background: #000000; border: 1px solid rgba(255,255,255,0.06); overflow: hidden; }
```

## 10. Hal yang dilarang di GUI
- `backdrop-filter`, blur, bayangan tebal, gradien rumit di latar jendela.
- Animasi > 150 ms atau yang memakai properti selain `opacity`/`transform`.
- Warna di luar token.
- Teks di bawah 10 px. Kontras teks di bawah 4,5:1.
- Menampilkan error mentah (stack trace, kode `Debug`) ke pengguna.
