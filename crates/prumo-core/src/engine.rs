//! Planejamento, aplicação e reversão.

use crate::catalog::{check_param, expand, first_failed_condition};
use crate::diag::Diagnosis;
use crate::journal::{ActionRecord, Entry, EntryStatus, Outcome, Session};
use crate::model::{ActionDef, Privilege, StepDef, Tier};
use crate::ops::Op;
use crate::prism::Instance;
use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum PlanStatus {
    Pending,
    AlreadyConfigured,
    Incompatible(String),
    /// Compatível, mas não pode ser aplicada agora (ex.: jogo aberto).
    Blocked(String),
    ManualOnly,
    Rejected,
    NotSelected,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedOp {
    pub op: Op,
    pub current: String,
    pub satisfied: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedAction {
    pub def: ActionDef,
    pub status: PlanStatus,
    pub params: BTreeMap<String, String>,
    pub ops: Vec<PlannedOp>,
    /// Alvos concretos (ex.: nomes das instâncias do Minecraft).
    pub targets: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Selection {
    /// None = plano recomendado; Some = exatamente estes ids.
    pub ids: Option<Vec<String>>,
    /// Sobrescritas "id.param" → valor (validadas contra o catálogo).
    pub params: BTreeMap<String, String>,
}

pub fn base_vars() -> BTreeMap<String, String> {
    let mut v = BTreeMap::new();
    if let Ok(h) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let cfg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{h}/.config"));
        v.insert("home".into(), h);
        v.insert("config".into(), cfg);
    }
    v
}

pub fn plan(catalog: &[ActionDef], diag: &Diagnosis, sel: &Selection) -> Result<Vec<PlannedAction>> {
    let facts = diag.fact_map();
    let elevated = crate::platform::is_elevated();
    let selected: HashSet<&str> = match &sel.ids {
        Some(ids) => {
            for id in ids {
                if !catalog.iter().any(|a| &a.id == id) {
                    bail!("ação desconhecida: {id}");
                }
            }
            ids.iter().map(String::as_str).collect()
        }
        None => catalog.iter().filter(|a| a.tier == Tier::Recommended).map(|a| a.id.as_str()).collect(),
    };
    let mut out = vec![];
    for def in catalog {
        let mut params = base_vars();
        for (k, p) in &def.params {
            let v = sel.params.get(&format!("{}.{k}", def.id)).unwrap_or(&p.default).clone();
            check_param(&v, &p.allowed).map_err(|e| anyhow::anyhow!("{}.{k}: {e}", def.id))?;
            params.insert(k.clone(), v);
        }
        let mut pa = PlannedAction { def: def.clone(), status: PlanStatus::Pending, params, ops: vec![], targets: vec![] };
        pa.status = if def.tier == Tier::Rejected {
            PlanStatus::Rejected
        } else if def.tier == Tier::Manual {
            PlanStatus::ManualOnly
        } else if !selected.contains(def.id.as_str()) {
            PlanStatus::NotSelected
        } else if let Some(c) = first_failed_condition(&def.conditions, &facts) {
            PlanStatus::Incompatible(format!("condição não atendida: {c}"))
        } else {
            expand_steps(&mut pa, &diag.minecraft)
        };
        if pa.status == PlanStatus::Pending && def.privilege == Privilege::Admin && !elevated {
            pa.status = PlanStatus::Blocked("requer administrador: rode novamente com privilégio elevado".into());
        }
        out.push(pa);
    }
    resolve_relations(&mut out);
    Ok(out)
}

fn expand_steps(pa: &mut PlannedAction, instances: &[Instance]) -> PlanStatus {
    let mut ops = vec![];
    let mut blocked = vec![];
    for step in &pa.def.steps {
        match step {
            StepDef::WriteFile { path, content, mode } => ops.push(Op::WriteFile {
                path: PathBuf::from(expand(path, &pa.params)),
                content: expand(content, &pa.params),
                mode: *mode,
            }),
            StepDef::IniSet { path, section, key, value } => ops.push(Op::IniSet {
                path: PathBuf::from(expand(path, &pa.params)),
                section: section.clone(),
                key: key.clone(),
                value: expand(value, &pa.params),
            }),
            StepDef::RegSet { path, name, value_type, data } => ops.push(Op::RegSet {
                path: path.clone(),
                name: name.clone(),
                kind: value_type.clone(),
                data: expand(data, &pa.params),
            }),
            StepDef::PrismInstance { selector, keys } => {
                let matches: Vec<&Instance> = instances
                    .iter()
                    .filter(|i| i.minecraft.as_deref() == Some(selector.minecraft.as_str()))
                    .filter(|i| selector.requires_component.as_ref().is_none_or(|c| i.components.iter().any(|x| x.contains(c.as_str()))))
                    .filter(|i| {
                        selector.name_contains_any.is_empty()
                            || selector.name_contains_any.iter().any(|n| {
                                let n = n.to_lowercase();
                                i.name.to_lowercase().contains(&n) || i.folder.to_lowercase().contains(&n)
                            })
                    })
                    .filter(|i| selector.min_java.is_none_or(|m| i.java_major.is_some_and(|j| j >= m)))
                    .collect();
                for i in matches {
                    if i.running {
                        blocked.push(format!("'{}' está aberta", i.name));
                        continue;
                    }
                    pa.targets.push(i.name.clone());
                    for (k, v) in keys {
                        ops.push(Op::IniSet {
                            path: i.cfg_path(),
                            section: Some("General".into()),
                            key: k.clone(),
                            value: expand(v, &pa.params),
                        });
                    }
                }
            }
        }
    }
    pa.ops = ops
        .into_iter()
        .map(|op| {
            let satisfied = op.is_satisfied().unwrap_or(false);
            PlannedOp { current: op.current_value(), satisfied, op }
        })
        .collect();
    if pa.ops.is_empty() {
        return if blocked.is_empty() {
            PlanStatus::Incompatible("nenhum alvo compatível encontrado".into())
        } else {
            PlanStatus::Blocked(format!("feche o jogo: {}", blocked.join(", ")))
        };
    }
    if pa.ops.iter().all(|o| o.satisfied) {
        PlanStatus::AlreadyConfigured
    } else {
        PlanStatus::Pending
    }
}

/// Conflitos e dependências entre ações selecionadas.
fn resolve_relations(plan: &mut [PlannedAction]) {
    let active: HashSet<String> = plan
        .iter()
        .filter(|p| matches!(p.status, PlanStatus::Pending | PlanStatus::AlreadyConfigured))
        .map(|p| p.def.id.clone())
        .collect();
    let mut taken: HashSet<String> = HashSet::new();
    for p in plan.iter_mut() {
        if !active.contains(&p.def.id) {
            continue;
        }
        if let Some(c) = p.def.conflicts.iter().find(|c| taken.contains(*c)) {
            p.status = PlanStatus::Incompatible(format!("conflita com {c}, já selecionada"));
            continue;
        }
        if let Some(d) = p.def.depends.iter().find(|d| !active.contains(*d)) {
            p.status = PlanStatus::Incompatible(format!("depende de {d}, que não está no plano"));
            continue;
        }
        taken.insert(p.def.id.clone());
    }
}

/// Aplica as ações pendentes. Cada ação é atômica: se uma operação falhar,
/// as anteriores da mesma ação são revertidas na hora.
pub fn apply(plan: &[PlannedAction], session: &mut Session) -> Result<Vec<ActionRecord>> {
    let mut records = vec![];
    let mut failed: HashSet<String> = HashSet::new();
    for pa in plan {
        let (outcome, detail) = match &pa.status {
            PlanStatus::NotSelected | PlanStatus::ManualOnly | PlanStatus::Rejected => continue,
            PlanStatus::AlreadyConfigured => (Outcome::AlreadyConfigured, "estado atual já é o desejado".into()),
            PlanStatus::Incompatible(r) | PlanStatus::Blocked(r) => (Outcome::Incompatible, r.clone()),
            PlanStatus::Pending => {
                if let Some(d) = pa.def.depends.iter().find(|d| failed.contains(*d)) {
                    (Outcome::Skipped, format!("dependência {d} falhou"))
                } else {
                    match apply_action(pa, session) {
                        Ok(n) => (Outcome::Applied, format!("{n} alteração(ões)")),
                        Err(e) => {
                            failed.insert(pa.def.id.clone());
                            (Outcome::Failed, format!("{e:#}"))
                        }
                    }
                }
            }
        };
        let rec = ActionRecord {
            action_id: pa.def.id.clone(),
            action_version: pa.def.version,
            outcome,
            detail,
            at: crate::now_iso(),
        };
        session.actions.push(rec.clone());
        session.save()?;
        records.push(rec);
    }
    session.finished = true;
    session.save()?;
    Ok(records)
}

fn apply_action(pa: &PlannedAction, session: &mut Session) -> Result<usize> {
    let mut done: Vec<usize> = vec![];
    for po in pa.ops.iter().filter(|o| !o.satisfied) {
        let seq = session.next_seq();
        let pre = po.op.capture(&session.backup_dir(), &format!("{seq:04}"))?;
        session.entries.push(Entry {
            seq,
            action_id: pa.def.id.clone(),
            action_version: pa.def.version,
            op: po.op.clone(),
            pre,
            applied_fingerprint: po.op.applied_fingerprint(),
            status: EntryStatus::Intent,
            error: None,
            at: crate::now_iso(),
        });
        session.save()?; // estado anterior em disco ANTES de alterar
        let idx = session.entries.len() - 1;
        let result = po.op.apply().and_then(|_| {
            if po.op.is_satisfied()? {
                Ok(())
            } else {
                bail!("validação falhou: valor não ficou como esperado")
            }
        });
        match result {
            Ok(()) => {
                session.entries[idx].status = EntryStatus::Applied;
                session.save()?;
                done.push(idx);
            }
            Err(e) => {
                let _ = session.entries[idx].op.restore(&session.entries[idx].pre.clone(), &session.backup_dir());
                session.entries[idx].status = EntryStatus::Failed;
                session.entries[idx].error = Some(format!("{e:#}"));
                for &j in done.iter().rev() {
                    let (op, pre) = (session.entries[j].op.clone(), session.entries[j].pre.clone());
                    if op.restore(&pre, &session.backup_dir()).is_ok() {
                        session.entries[j].status = EntryStatus::RolledBack;
                    }
                }
                session.save()?;
                bail!("{e:#} (alterações desta ação foram revertidas)");
            }
        }
    }
    Ok(done.len())
}

#[derive(Debug, Default, Serialize)]
pub struct UndoReport {
    pub restored: Vec<String>,
    pub conflicts: Vec<String>,
    pub errors: Vec<String>,
}

/// Desfaz uma sessão inteira (ou só uma ação), na ordem inversa. Se o valor
/// atual não for mais o que o app escreveu, não sobrescreve sem `force`.
pub fn undo(session: &mut Session, only_action: Option<&str>, force: bool) -> Result<UndoReport> {
    let mut rep = UndoReport::default();
    let backup = session.backup_dir();
    for i in (0..session.entries.len()).rev() {
        let e = &session.entries[i];
        if only_action.is_some_and(|a| a != e.action_id) {
            continue;
        }
        if !matches!(e.status, EntryStatus::Applied | EntryStatus::Intent | EntryStatus::UndoConflict) {
            continue;
        }
        let label = format!("{} · {}", e.action_id, e.op.describe());
        let changed_since = e.op.current_fingerprint().as_deref() != Some(e.applied_fingerprint.as_str());
        // Em `Intent` não sabemos se a alteração ocorreu; restaurar o estado
        // anterior é seguro nos dois casos.
        if changed_since && e.status != EntryStatus::Intent && !force {
            session.entries[i].status = EntryStatus::UndoConflict;
            rep.conflicts.push(format!("{label}: valor mudou depois da aplicação; use --forcar para restaurar mesmo assim"));
            continue;
        }
        match e.op.restore(&e.pre, &backup) {
            Ok(()) => {
                session.entries[i].status = EntryStatus::Undone;
                rep.restored.push(label);
            }
            Err(err) => rep.errors.push(format!("{label}: {err:#}")),
        }
    }
    session.save()?;
    Ok(rep)
}
