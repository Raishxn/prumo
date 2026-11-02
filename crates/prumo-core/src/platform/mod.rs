//! Operações específicas de plataforma.

use crate::ops::Op;
use anyhow::Result;

#[cfg(windows)]
mod windows_reg;

/// Lê (dados, tipo) de um valor de registro. `None` = valor ausente.
pub fn reg_read(op: &Op) -> Result<Option<(String, String)>> {
    #[cfg(windows)]
    {
        windows_reg::read(op)
    }
    #[cfg(not(windows))]
    {
        let _ = op;
        anyhow::bail!("registro do Windows indisponível neste sistema")
    }
}

/// Escreve o valor da operação (`restore = None`) ou restaura um valor
/// anterior (`Some((None, _))` = apagar o valor).
pub fn reg_write(op: &Op, restore: Option<(Option<&str>, Option<&str>)>) -> Result<()> {
    #[cfg(windows)]
    {
        windows_reg::write(op, restore)
    }
    #[cfg(not(windows))]
    {
        let _ = (op, restore);
        anyhow::bail!("registro do Windows indisponível neste sistema")
    }
}

/// O processo atual tem privilégio administrativo?
pub fn is_elevated() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid não tem pré-condições.
        unsafe { libc_geteuid() == 0 }
    }
    #[cfg(windows)]
    {
        // `net session` só funciona elevado; evita depender de crates de FFI.
        std::process::Command::new("net")
            .arg("session")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[cfg(unix)]
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}
