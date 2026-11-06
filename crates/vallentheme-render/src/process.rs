//! Menjalankan proses anak media (mpv) dengan pengamanan yang seragam.
//!
//! Aturan dari `panduan/00-aturan-agent.md`: tanpa shell, argumen berupa array,
//! anak harus ikut mati bila induk mati, dan punya grup proses sendiri agar
//! seluruh keturunannya bisa dibunuh bersama.

use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

use crate::error::{RenderError, Result};

/// Jalankan program tanpa shell.
///
/// Anak diberi `PR_SET_PDEATHSIG = SIGKILL` sehingga tidak pernah menjadi
/// proses yatim bila induk mati mendadak, dan memakai grup proses sendiri.
pub fn spawn_detached(program: &str, args: &[OsString]) -> Result<Child> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    cmd.process_group(0);
    arm_parent_death(&mut cmd);

    cmd.spawn().map_err(|sumber| RenderError::Spawn {
        program: program.to_string(),
        sumber,
    })
}

/// Pasang `PR_SET_PDEATHSIG` pada proses anak.
///
/// # Keselamatan
///
/// Penutup `pre_exec` berjalan di antara `fork` dan `exec`, sehingga hanya boleh
/// memanggil fungsi *async-signal-safe*. `prctl` memenuhi syarat itu. Nilai
/// baliknya diperiksa dan diteruskan sebagai `io::Error`.
#[allow(unsafe_code)]
fn arm_parent_death(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;

    // SAFETY: `prctl` async-signal-safe dan tidak menyentuh state Rust.
    unsafe {
        cmd.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Kirim `SIGTERM` ke seluruh grup proses anak, lalu `SIGKILL` bila perlu.
pub fn terminate_group(child: &mut Child, tunggu: std::time::Duration) -> Result<()> {
    let pid = match i32::try_from(child.id()) {
        Ok(pid) => pid,
        Err(_) => return Ok(()),
    };

    // Grup proses dibuat dengan `process_group(0)`, jadi pgid == pid.
    // SAFETY: `kill` hanya menyentuh tabel proses kernel.
    #[allow(unsafe_code)]
    unsafe {
        libc::kill(-pid, libc::SIGTERM);
    }

    let batas = std::time::Instant::now() + tunggu;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {
                if std::time::Instant::now() >= batas {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => return Ok(()),
        }
    }

    // SAFETY: sama seperti di atas.
    #[allow(unsafe_code)]
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let _ = child.wait();
    Ok(())
}

/// Turunkan prioritas proses anak sesuai level `nice` yang diminta.
pub fn set_nice(pid: u32, increment: i32) -> Result<()> {
    let Ok(pid) = i32::try_from(pid) else {
        return Ok(());
    };
    // SAFETY: `setpriority` hanya mengubah prioritas proses yang kita miliki.
    #[allow(unsafe_code)]
    let hasil = unsafe { libc::setpriority(libc::PRIO_PROCESS, pid as libc::id_t, increment) };

    if hasil == -1 {
        return Err(RenderError::Spawn {
            program: format!("nice(pid={pid})"),
            sumber: std::io::Error::last_os_error(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn program_tidak_ada_menghasilkan_error_berkode() {
        let hasil = spawn_detached("program-yang-pasti-tidak-ada-xyz", &[]);
        match hasil {
            Err(err) => assert_eq!(err.code(), "VT-MPV-001"),
            Ok(_) => panic!("seharusnya gagal"),
        }
    }

    #[test]
    fn anak_ikut_mati_saat_induk_mati() {
        // Jalankan skrip kecil yang mencatat PID-nya sendiri, lalu kita bunuh
        // induknya dan periksa anak ikut hilang.
        let args: Vec<OsString> = vec!["-c".into(), "sleep 30".into()];
        let mut child = spawn_detached("/bin/sh", &args).unwrap();
        let pid = child.id();

        terminate_group(&mut child, std::time::Duration::from_millis(300)).unwrap();

        let masih_hidup = std::path::Path::new(&format!("/proc/{pid}")).exists();
        assert!(!masih_hidup, "proses {pid} seharusnya sudah berhenti");
    }

    #[test]
    fn set_nice_berhasil_untuk_proses_sendiri() {
        let hasil = set_nice(std::process::id(), 1);
        assert!(hasil.is_ok(), "setpriority gagal: {hasil:?}");
    }
}
