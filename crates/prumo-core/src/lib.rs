//! Prumo — núcleo do otimizador pessoal (diagnóstico, catálogo, motor de
//! ações reversíveis e diário de sessões).

pub mod catalog;
pub mod diag;
pub mod engine;
pub mod ini;
pub mod journal;
pub mod measure;
pub mod model;
pub mod net;
pub mod ops;
pub mod platform;
pub mod prism;
pub mod protection;

use std::path::PathBuf;
use time::OffsetDateTime;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

pub fn now_iso() -> String {
    now().format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
}

pub fn now_compact() -> String {
    let f = time::macros::format_description!("[year][month][day]-[hour][minute][second]");
    now().format(&f).unwrap_or_default()
}

/// Pasta de estado (sessões, backups). `PRUMO_STATE_DIR` sobrescreve.
pub fn state_dir() -> PathBuf {
    if let Ok(p) = std::env::var("PRUMO_STATE_DIR") {
        return PathBuf::from(p);
    }
    #[cfg(windows)]
    {
        let base = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into());
        PathBuf::from(base).join("Prumo")
    }
    #[cfg(not(windows))]
    {
        if platform::is_elevated() {
            return PathBuf::from("/var/lib/prumo");
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(home).join(".local/state"))
            .join("prumo")
    }
}
