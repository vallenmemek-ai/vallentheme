//! Komponen antarmuka yang dipakai berulang.
//!
//! Dibangun dengan komposisi widget + kelas CSS (bukan subclass GObject) agar
//! kode lebih mudah diaudit dan tidak menambah boilerplate. Lihat catatan
//! penyimpangan di `docs/PROGRESS.md` Fase 3.

use gtk::prelude::*;

use crate::i18n::tr;

/// Status yang bisa ditampilkan chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Wallpaper berjalan.
    Berjalan,
    /// Tidak ada wallpaper aktif.
    TidakBerjalan,
    /// Dijeda karena aturan.
    Dijeda,
    /// Berjalan dengan batasan kinerja.
    Terbatas,
    /// Gagal dan menunggu tindakan.
    Gagal,
}

impl Status {
    fn kelas(self) -> &'static str {
        match self {
            Self::Berjalan => "running",
            Self::TidakBerjalan => "idle",
            Self::Dijeda => "paused",
            Self::Terbatas => "degraded",
            Self::Gagal => "failed",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Berjalan => "Running",
            Self::TidakBerjalan => "Not running",
            Self::Dijeda => "Paused",
            Self::Terbatas => "Degraded",
            Self::Gagal => "Failed",
        }
    }
}

/// Chip status untuk header bar yang bisa diperbarui tanpa dibangun ulang.
#[derive(Clone)]
pub struct StatusChip {
    kotak: gtk::Box,
    label: gtk::Label,
}

impl StatusChip {
    /// Widget yang dipasang ke header bar.
    pub fn widget(&self) -> &gtk::Box {
        &self.kotak
    }

    /// Ganti status tanpa mengganti widget.
    pub fn set(&self, status: Status) {
        for s in [
            Status::Berjalan,
            Status::TidakBerjalan,
            Status::Dijeda,
            Status::Terbatas,
            Status::Gagal,
        ] {
            self.kotak.remove_css_class(s.kelas());
        }
        self.kotak.add_css_class(status.kelas());
        let teks = tr(status.label());
        self.label.set_text(&teks);
        self.kotak.set_tooltip_text(Some(&teks));
    }
}

/// Chip status untuk header bar.
pub fn status_chip(status: Status) -> StatusChip {
    let kotak = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .valign(gtk::Align::Center)
        .build();
    kotak.add_css_class("vt-chip");

    let titik = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    titik.add_css_class("dot");
    titik.set_valign(gtk::Align::Center);

    let label = gtk::Label::new(Some(&tr(status.label())));
    label.add_css_class("vt-chip-label");

    kotak.append(&titik);
    kotak.append(&label);
    let chip = StatusChip { kotak, label };
    chip.set(status);
    chip
}

/// Urai `status_json` daemon menjadi status chip dan daftar wallpaper yang dipakai.
pub fn urai_status(json: &str) -> (Status, Vec<i64>) {
    let Ok(nilai) = serde_json::from_str::<serde_json::Value>(json) else {
        return (Status::TidakBerjalan, Vec::new());
    };
    let status = match nilai.get("state").and_then(|v| v.as_str()) {
        Some("running") => Status::Berjalan,
        Some("paused") => Status::Dijeda,
        Some("failed") => Status::Gagal,
        Some("degraded") => Status::Terbatas,
        _ => Status::TidakBerjalan,
    };
    let dipakai = nilai
        .get("monitors")
        .and_then(|v| v.as_array())
        .map(|monitor| {
            monitor
                .iter()
                .filter_map(|m| m.get("wallpaper_id").and_then(|i| i.as_i64()))
                .collect()
        })
        .unwrap_or_default();
    (status, dipakai)
}

/// Data kartu contoh untuk Fase 3 (belum tersambung ke basis data).
#[derive(Debug, Clone)]
pub struct KartuContoh {
    /// Nama tampilan.
    pub nama: String,
    /// Baris meta, misalnya `4K · 24fps · 15 MB`.
    pub meta: String,
    /// Jenis media.
    pub video: bool,
    /// Lencana resolusi.
    pub resolusi: Option<String>,
    /// Sedang dipakai sebagai wallpaper.
    pub dipakai: bool,
    /// Berkas hilang.
    pub hilang: bool,
    /// Favorit.
    pub favorit: bool,
}

impl KartuContoh {
    /// Buat data contoh.
    pub fn baru(nama: &str, meta: &str, video: bool) -> Self {
        Self {
            nama: nama.to_string(),
            meta: meta.to_string(),
            video,
            resolusi: Some("4K".to_string()),
            dipakai: false,
            hilang: false,
            favorit: false,
        }
    }
}

/// Kartu wallpaper 16:9.
pub fn wallpaper_card(data: &KartuContoh) -> gtk::Widget {
    let kartu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    kartu.add_css_class("wp-card");
    kartu.set_size_request(232, 160);
    if data.dipakai {
        kartu.add_css_class("in-use");
    }
    if data.hilang {
        kartu.add_css_class("missing");
    }

    // Lapisan gambar: untuk Fase 3 memakai blok warna sebagai placeholder.
    let thumb = gtk::Box::new(gtk::Orientation::Vertical, 0);
    thumb.add_css_class("wp-thumb");
    thumb.add_css_class("wp-skeleton");
    thumb.set_size_request(-1, 118);
    thumb.set_vexpand(true);

    // Lencana kiri atas.
    let lencana = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_start(8)
        .margin_top(8)
        .build();

    if data.video {
        lencana.append(&badge(&tr("Video"), ""));
    }
    if let Some(res) = &data.resolusi {
        lencana.append(&badge(res, "res"));
    }
    if data.hilang {
        lencana.append(&badge(&tr("Missing"), "missing"));
    }

    let lapis = gtk::Overlay::new();
    lapis.set_child(Some(&thumb));
    lapis.add_overlay(&lencana);
    lapis.set_vexpand(true);

    // Teks nama + meta.
    let teks = gtk::Box::new(gtk::Orientation::Vertical, 2);
    teks.set_margin_start(10);
    teks.set_margin_end(10);
    teks.set_margin_top(8);
    teks.set_margin_bottom(10);

    let nama = gtk::Label::new(Some(&data.nama));
    nama.add_css_class("wp-name");
    nama.set_xalign(0.0);
    nama.set_ellipsize(gtk::pango::EllipsizeMode::End);
    nama.set_tooltip_text(Some(&data.nama));

    let meta = gtk::Label::new(Some(&data.meta));
    meta.add_css_class("wp-meta");
    meta.set_xalign(0.0);
    meta.set_ellipsize(gtk::pango::EllipsizeMode::End);

    teks.append(&nama);
    teks.append(&meta);

    kartu.append(&lapis);
    kartu.append(&teks);
    kartu.upcast()
}

/// Lencana kecil (persegi radius 6).
fn badge(teks: &str, kelas: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(teks));
    label.add_css_class("wp-badge");
    if !kelas.is_empty() {
        label.add_css_class(kelas);
    }
    label.upcast()
}

/// Bilah bawah: hitungan di kiri, tombol di kanan.
pub fn bottom_bar(jumlah: u32) -> gtk::Widget {
    let bilah = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bilah.add_css_class("vt-bottombar");

    let hitung = gtk::Label::new(Some(&format!("{jumlah} {}", tr("wallpapers"))));
    hitung.add_css_class("vt-count");
    hitung.set_xalign(0.0);
    hitung.set_hexpand(true);

    let tambah_folder = tombol(&tr("Add folder"), "folder-symbolic", "vt-secondary");
    let dari_tautan = tombol(&tr("From link"), "insert-link-symbolic", "vt-secondary");
    dari_tautan.set_sensitive(false);
    dari_tautan.set_tooltip_text(Some(&tr("Coming soon")));
    let tambah = tombol(&tr("Add"), "list-add-symbolic", "vt-primary");
    let pilih = tombol(&tr("Select"), "object-select-symbolic", "vt-secondary");

    bilah.append(&hitung);
    bilah.append(&tambah_folder);
    bilah.append(&dari_tautan);
    bilah.append(&tambah);
    bilah.append(&pilih);
    bilah.upcast()
}

/// Tombol berikon dengan teks.
fn tombol(teks: &str, ikon: &str, kelas: &str) -> gtk::Button {
    let isi = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    isi.append(&gtk::Image::from_icon_name(ikon));
    isi.append(&gtk::Label::new(Some(teks)));

    let b = gtk::Button::builder().child(&isi).build();
    b.add_css_class(kelas);
    b.set_tooltip_text(Some(teks));
    b.set_accessible_role(gtk::AccessibleRole::Button);
    b.update_property(&[gtk::accessible::Property::Label(teks)]);
    b
}

/// Halaman kosong dengan ajakan bertindak.
pub fn empty_state() -> gtk::Widget {
    let halaman = adw::StatusPage::builder()
        .icon_name("image-x-generic-symbolic")
        .title(tr("No wallpapers yet"))
        .description(tr("Add a photo or video to get started"))
        .build();

    let aksi = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    aksi.set_halign(gtk::Align::Center);
    aksi.append(&tombol(&tr("Add"), "list-add-symbolic", "vt-primary"));
    aksi.append(&tombol(
        &tr("Add folder"),
        "folder-symbolic",
        "vt-secondary",
    ));
    halaman.set_child(Some(&aksi));
    halaman.upcast()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn status_punya_kelas_css_unik() {
        let semua = [
            Status::Berjalan,
            Status::TidakBerjalan,
            Status::Dijeda,
            Status::Terbatas,
            Status::Gagal,
        ];
        let mut kelas: Vec<&str> = semua.iter().map(|s| s.kelas()).collect();
        kelas.sort_unstable();
        let n = kelas.len();
        kelas.dedup();
        assert_eq!(kelas.len(), n);
    }

    #[test]
    fn urai_status_memahami_keadaan_dan_monitor() {
        let (s, ids) = urai_status(
            r#"{"v":1,"state":"running","monitors":[{"wallpaper_id":2},{"wallpaper_id":3}]}"#,
        );
        assert_eq!(s, Status::Berjalan);
        assert_eq!(ids, vec![2, 3]);

        let (s, ids) = urai_status(r#"{"v":1,"state":"idle","monitors":[]}"#);
        assert_eq!(s, Status::TidakBerjalan);
        assert!(ids.is_empty());

        // JSON rusak tidak boleh panic dan dianggap tidak berjalan.
        let (s, ids) = urai_status("bukan json");
        assert_eq!(s, Status::TidakBerjalan);
        assert!(ids.is_empty());
    }

    #[test]
    fn status_label_punya_terjemahan() {
        for s in [
            Status::Berjalan,
            Status::TidakBerjalan,
            Status::Dijeda,
            Status::Terbatas,
            Status::Gagal,
        ] {
            assert!(
                crate::i18n::punya_terjemahan(s.label()),
                "label {} belum diterjemahkan",
                s.label()
            );
        }
    }
}
