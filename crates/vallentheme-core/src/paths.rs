//! Lokasi berkas dan folder menurut spesifikasi XDG.
//!
//! Semua path yang dipakai aplikasi ditentukan di sini, tidak tersebar di
//! modul lain. Lihat `panduan/01-arsitektur.md` bagian 7.

use std::fs;
use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};

use crate::error::{Error, Result};

/// Kumpulan path yang dipakai vallentheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Folder konfigurasi (`~/.config/vallentheme`).
    pub config_dir: PathBuf,
    /// Berkas konfigurasi (`config.toml`).
    pub config_file: PathBuf,
    /// Folder data (`~/.local/share/vallentheme`).
    pub data_dir: PathBuf,
    /// Berkas basis data library.
    pub db_file: PathBuf,
    /// Folder cache (`~/.cache/vallentheme`).
    pub cache_dir: PathBuf,
    /// Cache thumbnail.
    pub thumbs_dir: PathBuf,
    /// Cache video hasil optimasi.
    pub optimized_dir: PathBuf,
    /// Cache gambar hasil rotasi/pangkas.
    pub rendered_dir: PathBuf,
    /// Folder state (`~/.local/state/vallentheme`).
    pub state_dir: PathBuf,
    /// Berkas state daemon.
    pub daemon_state_file: PathBuf,
    /// Folder log.
    pub logs_dir: PathBuf,
    /// Folder runtime (socket, lock), izin `0700`.
    pub runtime_dir: PathBuf,
    /// Folder unduhan bawaan.
    pub default_download_dir: PathBuf,
}

impl Paths {
    /// Tentukan semua path dari lingkungan saat ini.
    pub fn resolve() -> Result<Self> {
        let proj = ProjectDirs::from("id", "vallentheme", "vallentheme").ok_or(Error::NoDataDir)?;
        let base = BaseDirs::new().ok_or(Error::NoDataDir)?;

        let config_dir = proj.config_dir().to_path_buf();
        let data_dir = proj.data_dir().to_path_buf();
        let cache_dir = proj.cache_dir().to_path_buf();
        let state_dir = proj.state_dir().unwrap_or(&data_dir).to_path_buf();

        // $XDG_RUNTIME_DIR wajib untuk socket; bila tidak ada, pakai state dir
        // supaya aplikasi tetap jalan (dengan peringatan di log).
        let runtime_dir = match base.runtime_dir() {
            Some(dir) => dir.join("vallentheme"),
            None => {
                tracing::warn!(
                    "XDG_RUNTIME_DIR tidak tersedia; memakai folder state untuk runtime"
                );
                state_dir.join("runtime")
            }
        };

        let default_download_dir = base.home_dir().join("Videos").join("vallentheme");

        Ok(Self {
            config_file: config_dir.join("config.toml"),
            config_dir,
            db_file: data_dir.join("library.db"),
            data_dir,
            thumbs_dir: cache_dir.join("thumbs"),
            optimized_dir: cache_dir.join("optimized"),
            rendered_dir: cache_dir.join("rendered"),
            cache_dir,
            daemon_state_file: state_dir.join("daemon-state.json"),
            logs_dir: state_dir.join("logs"),
            state_dir,
            runtime_dir,
            default_download_dir,
        })
    }

    /// Buat semua folder yang dibutuhkan. Folder runtime diberi izin `0700`.
    pub fn ensure(&self) -> Result<()> {
        let folders = [
            &self.config_dir,
            &self.data_dir,
            &self.cache_dir,
            &self.thumbs_dir,
            &self.optimized_dir,
            &self.rendered_dir,
            &self.state_dir,
            &self.logs_dir,
            &self.runtime_dir,
        ];
        for dir in folders {
            fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
        }
        set_private(&self.runtime_dir)?;
        Ok(())
    }
}

/// Batasi izin folder ke pemilik saja (`0700`).
#[cfg(unix)]
fn set_private(dir: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perm = fs::metadata(dir)
        .map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?
        .permissions();
    perm.set_mode(0o700);
    fs::set_permissions(dir, perm).map_err(|source| Error::Io {
        path: dir.to_path_buf(),
        source,
    })
}

/// Di luar Unix tidak ada konsep izin POSIX.
#[cfg(not(unix))]
fn set_private(_dir: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Semua uji yang bergantung variabel lingkungan dijalankan dalam satu tes
    /// agar tidak saling menimpa saat test berjalan paralel.
    #[test]
    fn susunan_xdg() {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();

        std::env::set_var("XDG_CONFIG_HOME", r.join("config"));
        std::env::set_var("XDG_DATA_HOME", r.join("data"));
        std::env::set_var("XDG_CACHE_HOME", r.join("cache"));
        std::env::set_var("XDG_STATE_HOME", r.join("state"));
        std::env::set_var("XDG_RUNTIME_DIR", r.join("run"));

        let paths = Paths::resolve().unwrap();

        assert!(paths.config_file.ends_with("vallentheme/config.toml"));
        assert!(paths.db_file.ends_with("vallentheme/library.db"));
        assert!(paths.thumbs_dir.ends_with("vallentheme/thumbs"));
        assert!(paths.optimized_dir.ends_with("vallentheme/optimized"));
        assert!(paths
            .daemon_state_file
            .ends_with("vallentheme/daemon-state.json"));
        assert!(paths.logs_dir.ends_with("vallentheme/logs"));
        assert!(paths.runtime_dir.ends_with("vallentheme"));
        assert!(paths.default_download_dir.ends_with("Videos/vallentheme"));

        paths.ensure().unwrap();
        for dir in [
            &paths.config_dir,
            &paths.data_dir,
            &paths.cache_dir,
            &paths.thumbs_dir,
            &paths.optimized_dir,
            &paths.state_dir,
            &paths.logs_dir,
            &paths.runtime_dir,
        ] {
            assert!(dir.is_dir(), "folder belum dibuat: {}", dir.display());
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&paths.runtime_dir)
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "izin runtime harus 0700");
        }

        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_DATA_HOME");
        std::env::remove_var("XDG_CACHE_HOME");
        std::env::remove_var("XDG_STATE_HOME");
        std::env::remove_var("XDG_RUNTIME_DIR");
    }
}
