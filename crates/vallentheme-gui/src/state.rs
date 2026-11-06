//! Keadaan bersama aplikasi yang dipakai banyak halaman.

use std::cell::RefCell;
use std::rc::Rc;

use vallentheme_core::config::Config;
use vallentheme_core::db_actor::DbActor;
use vallentheme_core::paths::Paths;

/// Keadaan yang dibagi ke seluruh jendela.
pub struct AppState {
    /// Lokasi berkas XDG.
    pub paths: Paths,
    /// Pegangan thread basis data.
    pub db: std::sync::Arc<DbActor>,
    /// Konfigurasi pengguna (bisa berubah saat berjalan).
    pub config: RefCell<Config>,
}

/// Alias yang dipakai di seluruh kode GUI.
pub type Shared = Rc<AppState>;
