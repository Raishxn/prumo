//! Diário de sessão com escrita antecipada (write-ahead): o estado anterior de
//! cada operação é gravado em disco ANTES da alteração. Se o processo cair no
//! meio, o diário diz exatamente o que pode ter mudado e como voltar.

use crate::ops::{write_atomic, Op, PreState};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    /// Estado anterior salvo; alteração pode ou não ter ocorrido (queda).
    Intent,
    Applied,
    /// Desfeita automaticamente porque outra operação da mesma ação falhou.
    RolledBack,
    /// Desfeita a pedido do usuário.
    Undone,
    /// Não desfeita: o valor foi alterado depois por outro programa/pessoa.
    UndoConflict,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u32,
    pub action_id: String,
    pub action_version: u32,
    pub op: Op,
    pub pre: PreState,
    pub applied_fingerprint: String,
    pub status: EntryStatus,
    #[serde(default)]
    pub error: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Applied,
    AlreadyConfigured,
    Incompatible,
    Skipped,
    Failed,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Applied => "aplicado",
            Outcome::AlreadyConfigured => "já configurado",
            Outcome::Incompatible => "incompatível",
            Outcome::Skipped => "ignorado",
            Outcome::Failed => "falhou",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    pub action_id: String,
    pub action_version: u32,
    pub outcome: Outcome,
    pub detail: String,
    pub at: String,
}

/// Registro da proteção criada antes da sessão (ponto de restauração,
/// snapshot) e de sua verificação.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionRecord {
    pub kind: String,
    pub verified: bool,
    pub detail: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created: String,
    pub tool_version: String,
    pub platform: String,
    /// "app" ou "playbook": quem aplicou.
    pub origin: String,
    #[serde(default)]
    pub protection: Vec<ProtectionRecord>,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub actions: Vec<ActionRecord>,
    #[serde(default)]
    pub finished: bool,
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Session {
    pub fn create(state_dir: &Path, origin: &str) -> Result<Session> {
        let id = format!("{}-{}", crate::now_compact(), std::process::id());
        let dir = state_dir.join("sessions").join(&id);
        fs::create_dir_all(dir.join("backups")).with_context(|| format!("criar {}", dir.display()))?;
        let s = Session {
            id,
            created: crate::now_iso(),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            platform: std::env::consts::OS.into(),
            origin: origin.into(),
            protection: vec![],
            entries: vec![],
            actions: vec![],
            finished: false,
            dir,
        };
        s.save()?;
        Ok(s)
    }

    pub fn load(dir: &Path) -> Result<Session> {
        let text = fs::read_to_string(dir.join("journal.json"))
            .with_context(|| format!("ler diário em {}", dir.display()))?;
        let mut s: Session = serde_json::from_str(&text).context("diário inválido")?;
        s.dir = dir.to_path_buf();
        Ok(s)
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&self.dir.join("journal.json"), &json)
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    pub fn next_seq(&self) -> u32 {
        self.entries.len() as u32 + 1
    }

    /// Entradas que ficaram em `Intent`: indicam queda no meio de uma alteração.
    pub fn interrupted(&self) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.status == EntryStatus::Intent).collect()
    }
}

/// Lista sessões, mais recentes primeiro.
pub fn list_sessions(state_dir: &Path) -> Vec<Session> {
    let mut v: Vec<Session> = fs::read_dir(state_dir.join("sessions"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Session::load(&e.path()).ok())
        .collect();
    v.sort_by(|a, b| b.id.cmp(&a.id));
    v
}
