//! Jendela utama dan navigasi antarhalaman.

use gtk::prelude::*;

use crate::pages;

/// Bangun jendela utama berisi `NavigationView` dengan halaman Library.
pub fn bangun(app: &adw::Application) -> adw::ApplicationWindow {
    let navigasi = adw::NavigationView::new();
    navigasi.push(&pages::library());

    let lapis_toast = adw::ToastOverlay::new();
    lapis_toast.set_child(Some(&navigasi));

    let jendela = adw::ApplicationWindow::builder()
        .application(app)
        .title("vallentheme")
        .default_width(960)
        .default_height(660)
        .content(&lapis_toast)
        .build();

    // Ukuran minimum dari panduan desain.
    jendela.set_size_request(420, 520);
    jendela
}
