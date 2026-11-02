//! Operações concretas e reversíveis. Cada operação sabe:
//! - ler o estado atual (`capture`),
//! - dizer se já está no estado desejado (`is_satisfied`),
//! - aplicar (`apply`),
//! - restaurar o estado anterior real capturado (`restore`).

use crate::ini;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    WriteFile { path: PathBuf, content: String, mode: Option<u32> },
    IniSet { path: PathBuf, section: Option<String>, key: String, value: String },
    RegSet { path: String, name: String, kind: String, data: String },
}

/// Estado anterior real, guardado antes de qualquer alteração.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreState {
    /// Arquivo não existia.
    FileAbsent,
    /// Conteúdo completo salvo em `backup` (relativo à pasta da sessão).
    FileContent { backup: String, sha256: String, mode: Option<u32> },
    /// Valor anterior da chave INI (None = ausente). O arquivo inteiro também
    /// é copiado para `backup` como segunda linha de defesa.
    IniValue { previous: Option<String>, file_existed: bool, backup: Option<String> },
    RegValue { previous: Option<String>, previous_kind: Option<String> },
}

impl Op {
    pub fn describe(&self) -> String {
        match self {
            Op::WriteFile { path, .. } => format!("escrever {}", path.display()),
            Op::IniSet { path, section, key, value } => format!(
                "{}: [{}] {key}={value}",
                path.display(),
                section.as_deref().unwrap_or("")
            ),
            Op::RegSet { path, name, kind, data } => format!("{path}\\{name} = {data} ({kind})"),
        }
    }

    /// Descrição do valor atual, para a prévia.
    pub fn current_value(&self) -> String {
        match self {
            Op::WriteFile { path, .. } => match fs::read_to_string(path) {
                Ok(c) => format!("existe ({} bytes)", c.len()),
                Err(_) => "não existe".into(),
            },
            Op::IniSet { path, section, key, .. } => fs::read_to_string(path)
                .ok()
                .and_then(|t| ini::get(&t, section.as_deref(), key))
                .unwrap_or_else(|| "(ausente)".into()),
            Op::RegSet { .. } => crate::platform::reg_read(self)
                .map(|v| v.map(|(d, _)| d).unwrap_or_else(|| "(ausente)".into()))
                .unwrap_or_else(|e| format!("indisponível: {e}")),
        }
    }

    pub fn is_satisfied(&self) -> Result<bool> {
        Ok(match self {
            Op::WriteFile { path, content, .. } => {
                fs::read_to_string(path).map(|c| c == *content).unwrap_or(false)
            }
            Op::IniSet { path, section, key, value } => fs::read_to_string(path)
                .ok()
                .and_then(|t| ini::get(&t, section.as_deref(), key))
                .is_some_and(|v| v == *value),
            Op::RegSet { data, .. } => {
                crate::platform::reg_read(self)?.is_some_and(|(d, _)| d == *data)
            }
        })
    }

    /// Captura o estado anterior. Arquivos são copiados para `backup_dir`.
    pub fn capture(&self, backup_dir: &Path, tag: &str) -> Result<PreState> {
        match self {
            Op::WriteFile { path, .. } => {
                if !path.exists() {
                    return Ok(PreState::FileAbsent);
                }
                let bytes = fs::read(path).with_context(|| format!("ler {}", path.display()))?;
                let name = format!("{tag}.bak");
                write_atomic(&backup_dir.join(&name), &bytes)?;
                Ok(PreState::FileContent { backup: name, sha256: sha256(&bytes), mode: file_mode(path) })
            }
            Op::IniSet { path, section, key, .. } => {
                if !path.exists() {
                    return Ok(PreState::IniValue { previous: None, file_existed: false, backup: None });
                }
                let bytes = fs::read(path)?;
                let text = String::from_utf8_lossy(&bytes);
                let name = format!("{tag}.bak");
                write_atomic(&backup_dir.join(&name), &bytes)?;
                Ok(PreState::IniValue {
                    previous: ini::get(&text, section.as_deref(), key),
                    file_existed: true,
                    backup: Some(name),
                })
            }
            Op::RegSet { .. } => {
                let prev = crate::platform::reg_read(self)?;
                Ok(PreState::RegValue {
                    previous: prev.as_ref().map(|p| p.0.clone()),
                    previous_kind: prev.map(|p| p.1),
                })
            }
        }
    }

    pub fn apply(&self) -> Result<()> {
        match self {
            Op::WriteFile { path, content, mode } => {
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                write_atomic(path, content.as_bytes())?;
                if let Some(m) = mode {
                    set_mode(path, *m)?;
                }
                Ok(())
            }
            Op::IniSet { path, section, key, value } => {
                let text = fs::read_to_string(path).unwrap_or_default();
                let out = ini::set(&text, section.as_deref(), key, Some(value));
                write_atomic(path, out.as_bytes())
            }
            Op::RegSet { .. } => crate::platform::reg_write(self, None),
        }
    }

    /// Hash do estado que esta operação deixou, para detectar alterações
    /// posteriores feitas por outra pessoa/programa antes de desfazer.
    pub fn applied_fingerprint(&self) -> String {
        match self {
            Op::WriteFile { content, .. } => sha256(content.as_bytes()),
            Op::IniSet { value, .. } => sha256(value.as_bytes()),
            Op::RegSet { data, .. } => sha256(data.as_bytes()),
        }
    }

    pub fn current_fingerprint(&self) -> Option<String> {
        match self {
            Op::WriteFile { path, .. } => fs::read(path).ok().map(|b| sha256(&b)),
            Op::IniSet { path, section, key, .. } => fs::read_to_string(path)
                .ok()
                .and_then(|t| ini::get(&t, section.as_deref(), key))
                .map(|v| sha256(v.as_bytes())),
            Op::RegSet { .. } => crate::platform::reg_read(self)
                .ok()
                .flatten()
                .map(|(d, _)| sha256(d.as_bytes())),
        }
    }

    /// Restaura o estado anterior real.
    pub fn restore(&self, pre: &PreState, backup_dir: &Path) -> Result<()> {
        match (self, pre) {
            (Op::WriteFile { path, .. }, PreState::FileAbsent) => {
                if path.exists() {
                    fs::remove_file(path)?;
                }
                Ok(())
            }
            (Op::WriteFile { path, .. }, PreState::FileContent { backup, sha256: h, mode }) => {
                let bytes = fs::read(backup_dir.join(backup)).context("backup ausente")?;
                if sha256(&bytes) != *h {
                    bail!("backup corrompido: hash não confere");
                }
                write_atomic(path, &bytes)?;
                if let Some(m) = mode {
                    set_mode(path, *m)?;
                }
                Ok(())
            }
            (Op::IniSet { path, section, key, .. }, PreState::IniValue { previous, file_existed, .. }) => {
                if !file_existed {
                    // Só criamos o arquivo para esta chave: remover a chave basta.
                    let text = fs::read_to_string(path).unwrap_or_default();
                    let out = ini::set(&text, section.as_deref(), key, None);
                    if out.trim().is_empty() || out.trim() == format!("[{}]", section.as_deref().unwrap_or("")) {
                        let _ = fs::remove_file(path);
                        return Ok(());
                    }
                    return write_atomic(path, out.as_bytes());
                }
                let text = fs::read_to_string(path).unwrap_or_default();
                let out = ini::set(&text, section.as_deref(), key, previous.as_deref());
                write_atomic(path, out.as_bytes())
            }
            (Op::RegSet { .. }, PreState::RegValue { previous, previous_kind }) => crate::platform::reg_write(
                self,
                Some((previous.as_deref(), previous_kind.as_deref())),
            ),
            _ => bail!("estado anterior incompatível com a operação"),
        }
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Escrita atômica: arquivo temporário no mesmo diretório + fsync + rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.prumo-tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("arquivo")
    ));
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("criar {}", tmp.display()))?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Some(m) = file_mode(path) {
        let _ = set_mode(&tmp, m);
    }
    fs::rename(&tmp, path).with_context(|| format!("substituir {}", path.display()))?;
    Ok(())
}

#[cfg(unix)]
fn file_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).ok().map(|m| m.permissions().mode() & 0o7777)
}
#[cfg(not(unix))]
fn file_mode(_: &Path) -> Option<u32> {
    None
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}
#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> Result<()> {
    Ok(())
}
