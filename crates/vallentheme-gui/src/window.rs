//! Jendela utama dan navigasi antarhalaman.

use gtk::prelude::*;

use crate::library;
use crate::state::Shared;

/// Bangun jendela utama berisi `NavigationView` dengan halaman Library.
pub fn bangun(app: &adw::Application, state: Shared) -> adw::ApplicationWindow {
    let navigasi = adw::NavigationView::new();
    let lapis_toast = adw::ToastOverlay::new();

    let halaman = library::build(state, navigasi.clone(), lapis_toast.clone());
    navigasi.push(&halaman);
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
