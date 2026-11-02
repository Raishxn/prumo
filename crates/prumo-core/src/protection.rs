//! Proteção obrigatória antes de qualquer sessão que altere o sistema.
//!
//! - Windows: ponto de restauração criado E verificado (número de sequência
//!   novo com a descrição da sessão). Sem verificação, a sessão é abortada.
//! - Linux: backups específicos de cada arquivo (sempre). Snapshots Btrfs
//!   ficam para ações administrativas (etapa 2).

use crate::engine::{PlanStatus, PlannedAction};
use crate::journal::ProtectionRecord;
use crate::model::Privilege;
use anyhow::Result;

#[derive(Debug, Default, Clone)]
pub struct ProtectionOptions {
    /// Windows: aceitar um ponto de restauração já existente com menos de
    /// 24 h (o Windows bloqueia criar dois em 24 h por padrão).
    pub accept_recent_restore_point: bool,
}

pub fn prepare(plan: &[PlannedAction], session_id: &str, opts: &ProtectionOptions) -> Result<Vec<ProtectionRecord>> {
    let pending: Vec<&PlannedAction> = plan.iter().filter(|p| p.status == PlanStatus::Pending).collect();
    let mut recs = vec![ProtectionRecord {
        kind: "backup-especifico".into(),
        verified: true,
        detail: "Cada arquivo/chave é copiado para a pasta da sessão antes de mudar; o hash é conferido ao restaurar."
            .into(),
        at: crate::now_iso(),
    }];
    if pending.is_empty() {
        return Ok(recs);
    }
    #[cfg(windows)]
    {
        recs.push(windows_restore_point(session_id, opts)?);
    }
    #[cfg(not(windows))]
    {
        let _ = (session_id, opts);
        if pending.iter().any(|p| p.def.privilege == Privilege::Admin) {
            recs.push(ProtectionRecord {
                kind: "snapshot".into(),
                verified: false,
                detail: "Snapshot Btrfs ainda não implementado (etapa 2); só backups específicos cobrem esta sessão.".into(),
                at: crate::now_iso(),
            });
        }
    }
    Ok(recs)
}

#[cfg(windows)]
fn windows_restore_point(session_id: &str, opts: &ProtectionOptions) -> Result<ProtectionRecord> {
    use anyhow::bail;
    let ps = |c: &str| crate::diag::run("powershell", &["-NoProfile", "-NonInteractive", "-Command", c]).map(|s| s.trim().to_string());
    let last = || ps("(Get-ComputerRestorePoint | Sort-Object SequenceNumber | Select-Object -Last 1 | ForEach-Object { \"$($_.SequenceNumber)|$($_.Description)|$($_.CreationTime)\" })");
    let before = last();
    let desc = format!("Prumo {session_id}");
    let created = ps(&format!(
        "try {{ Checkpoint-Computer -Description '{desc}' -RestorePointType MODIFY_SETTINGS -ErrorAction Stop -WarningVariable w; if ($w) {{ 'AVISO: ' + $w }} else {{ 'ok' }} }} catch {{ 'ERRO: ' + $_.Exception.Message }}"
    ))
    .unwrap_or_else(|| "ERRO: PowerShell indisponível".into());
    let after = last();
    let seq = |s: &Option<String>| s.as_deref().and_then(|v| v.split('|').next()).and_then(|n| n.parse::<u64>().ok());
    if seq(&after) > seq(&before) && after.as_deref().is_some_and(|a| a.contains(&desc)) {
        return Ok(ProtectionRecord {
            kind: "ponto-de-restauracao".into(),
            verified: true,
            detail: format!("Criado e confirmado: {}", after.unwrap_or_default()),
            at: crate::now_iso(),
        });
    }
    if opts.accept_recent_restore_point {
        if let Some(b) = before {
            return Ok(ProtectionRecord {
                kind: "ponto-de-restauracao-existente".into(),
                verified: true,
                detail: format!("Novo ponto não criado ({created}); usando o existente por escolha do usuário: {b}"),
                at: crate::now_iso(),
            });
        }
    }
    bail!(
        "Ponto de restauração NÃO confirmado ({created}). Nada foi alterado.\n\
         Opções: 1) ative a Proteção do Sistema no disco C: (sysdm.cpl → Proteção do Sistema);\n\
         2) se já existe um ponto criado nas últimas 24 h, rode de novo com --usar-ponto-existente;\n\
         3) crie um ponto manualmente e repita."
    )
}
