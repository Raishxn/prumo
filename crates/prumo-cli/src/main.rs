//! `prumo` — linha de comando. Funciona sem interface gráfica e é também o
//! caminho de recuperação quando a interface não abre.

use anyhow::{bail, Context, Result};
use prumo_core::engine::{self, PlanStatus, PlannedAction, Selection};
use prumo_core::journal::{list_sessions, Session};
use prumo_core::{catalog, diag, protection, state_dir};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};

const AJUDA: &str = "\
prumo — otimizador pessoal com reversão

USO
  prumo diagnostico [--json]          Inventário da máquina (somente leitura)
  prumo catalogo [--json]             Todas as ações, com nível e evidência
  prumo explicar <id>                 Documentação completa de uma ação
  prumo plano [opções] [--json]       Prévia: estado atual → alteração proposta
  prumo aplicar [opções] [--sim]      Aplica o plano (pede confirmação sem --sim)
  prumo sessoes                       Sessões registradas
  prumo desfazer <sessão|ultima> [--acao <id>] [--forcar]
  prumo recuperar                     Procura sessões interrompidas

OPÇÕES DE PLANO
  --acoes a,b,c        Exatamente estas ações (padrão: plano recomendado)
  --param id.nome=v    Ajusta um parâmetro validado da ação
  --usar-ponto-existente   (Windows) aceita ponto de restauração com menos de 24 h

Estado e backups ficam em: ";

fn main() {
    if let Err(e) = run() {
        eprintln!("erro: {e:#}");
        std::process::exit(1);
    }
}

struct Args {
    cmd: String,
    pos: Vec<String>,
    flags: BTreeMap<String, String>,
    params: BTreeMap<String, String>,
}

fn parse() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_else(|| "ajuda".into());
    let (mut pos, mut flags, mut params) = (vec![], BTreeMap::new(), BTreeMap::new());
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" | "--sim" | "--forcar" | "--usar-ponto-existente" => {
                flags.insert(a, String::new());
            }
            "--acoes" | "--acao" => {
                flags.insert(a, it.next().context("faltou o valor")?);
            }
            "--param" => {
                let v = it.next().context("faltou id.nome=valor")?;
                let (k, val) = v.split_once('=').context("use id.nome=valor")?;
                params.insert(k.to_string(), val.to_string());
            }
            s if s.starts_with("--") => bail!("opção desconhecida: {s}"),
            _ => pos.push(a),
        }
    }
    Ok(Args { cmd, pos, flags, params })
}

fn run() -> Result<()> {
    let a = parse()?;
    let json = a.flags.contains_key("--json");
    let all = catalog::load()?;
    let cat = catalog::for_platform(&all, catalog::current_platform());
    match a.cmd.as_str() {
        "diagnostico" => {
            let d = diag::collect();
            if json {
                println!("{}", serde_json::to_string_pretty(&d)?);
            } else {
                print_diag(&d);
            }
        }
        "catalogo" => {
            if json {
                println!("{}", serde_json::to_string_pretty(&cat)?);
            } else {
                for x in &cat {
                    println!("{:<44} {:<12} {}", x.id, format!("{:?}", x.tier).to_lowercase(), x.evidence.label());
                }
            }
        }
        "explicar" => {
            let id = a.pos.first().context("informe o id")?;
            let x = all.iter().find(|x| &x.id == id).context("ação desconhecida")?;
            print_action_doc(x);
        }
        "plano" => {
            let (_, plan) = build_plan(&cat, &a)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                print_plan(&plan);
            }
        }
        "aplicar" => apply(&cat, &a)?,
        "sessoes" => {
            for s in list_sessions(&state_dir()) {
                let applied = s.actions.iter().filter(|r| r.outcome == prumo_core::journal::Outcome::Applied).count();
                let flag = if s.interrupted().is_empty() { "" } else { "  ⚠ interrompida" };
                println!("{}  origem={}  ações aplicadas={}  alterações={}{}", s.id, s.origin, applied, s.entries.len(), flag);
            }
        }
        "desfazer" => undo(&a)?,
        "recuperar" => {
            let mut any = false;
            for s in list_sessions(&state_dir()) {
                let i = s.interrupted();
                if !i.is_empty() {
                    any = true;
                    println!("Sessão {} parou no meio de {} alteração(ões):", s.id, i.len());
                    for e in i {
                        println!("  - {} · {}", e.action_id, e.op.describe());
                    }
                    println!("  Para voltar ao estado anterior: prumo desfazer {}", s.id);
                }
            }
            if !any {
                println!("Nenhuma sessão interrompida.");
            }
        }
        _ => println!("{AJUDA}{}", state_dir().display()),
    }
    Ok(())
}

fn build_plan(cat: &[prumo_core::model::ActionDef], a: &Args) -> Result<(diag::Diagnosis, Vec<PlannedAction>)> {
    let d = diag::collect();
    let sel = Selection {
        ids: a.flags.get("--acoes").map(|s| s.split(',').map(|x| x.trim().to_string()).collect()),
        params: a.params.clone(),
    };
    let plan = engine::plan(cat, &d, &sel)?;
    Ok((d, plan))
}

fn apply(cat: &[prumo_core::model::ActionDef], a: &Args) -> Result<()> {
    let (_, plan) = build_plan(cat, a)?;
    print_plan(&plan);
    let pending = plan.iter().filter(|p| p.status == PlanStatus::Pending).count();
    if pending == 0 {
        println!("\nNada a aplicar.");
        return Ok(());
    }
    if !a.flags.contains_key("--sim") {
        print!("\nAplicar {pending} ação(ões)? Digite 's' para confirmar: ");
        std::io::stdout().flush()?;
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        if line.trim().to_lowercase() != "s" {
            println!("Cancelado. Nada foi alterado.");
            return Ok(());
        }
    }
    let mut session = Session::create(&state_dir(), "app")?;
    let opts = protection::ProtectionOptions { accept_recent_restore_point: a.flags.contains_key("--usar-ponto-existente") };
    match protection::prepare(&plan, &session.id, &opts) {
        Ok(recs) => {
            session.protection = recs;
            session.save()?;
        }
        Err(e) => {
            session.finished = true;
            session.save()?;
            bail!("proteção obrigatória falhou; sessão interrompida antes de qualquer alteração.\n{e:#}");
        }
    }
    let recs = engine::apply(&plan, &mut session)?;
    println!("\nSessão {} ({}):", session.id, session.dir.display());
    for r in &recs {
        println!("  [{}] {} — {}", r.outcome.label(), r.action_id, r.detail);
    }
    let restart: Vec<_> = plan
        .iter()
        .filter(|p| recs.iter().any(|r| r.action_id == p.def.id && r.outcome == prumo_core::journal::Outcome::Applied))
        .map(|p| format!("{}: {}", p.def.id, p.def.restart.label()))
        .collect();
    if !restart.is_empty() {
        println!("\nPróximo passo:");
        for r in restart {
            println!("  - {r}");
        }
    }
    println!("\nPara desfazer tudo: prumo desfazer {}", session.id);
    Ok(())
}

fn undo(a: &Args) -> Result<()> {
    let which = a.pos.first().context("informe a sessão ou 'ultima'")?;
    let sessions = list_sessions(&state_dir());
    let s = if which == "ultima" {
        sessions.into_iter().find(|s| !s.entries.is_empty()).context("nenhuma sessão com alterações")?
    } else {
        sessions.into_iter().find(|s| &s.id == which).context("sessão não encontrada")?
    };
    let mut s = Session::load(&s.dir)?;
    let rep = engine::undo(&mut s, a.flags.get("--acao").map(String::as_str), a.flags.contains_key("--forcar"))?;
    for r in &rep.restored {
        println!("restaurado: {r}");
    }
    for c in &rep.conflicts {
        println!("NÃO restaurado: {c}");
    }
    for e in &rep.errors {
        println!("erro: {e}");
    }
    if rep.restored.is_empty() && rep.conflicts.is_empty() && rep.errors.is_empty() {
        println!("Nada a desfazer nesta sessão.");
    }
    Ok(())
}

fn print_diag(d: &diag::Diagnosis) {
    println!("Diagnóstico coletado em {}\n", d.collected_at);
    for (k, f) in &d.facts {
        match &f.value {
            Some(v) => println!("  {k:<28} {v}"),
            None => println!("  {k:<28} desconhecido — {}", f.source),
        }
    }
    if !d.minecraft.is_empty() {
        println!("\nInstâncias do Prism ({}):", d.minecraft.len());
        for i in &d.minecraft {
            println!(
                "  {:<46} MC {:<7} Java {:<4} mem {}–{} MB{}",
                truncate(&i.name, 46),
                i.minecraft.as_deref().unwrap_or("?"),
                i.java_major.map(|j| j.to_string()).unwrap_or_else(|| "?".into()),
                i.min_mem_mb.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                i.max_mem_mb.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
                if i.running { "  [aberta]" } else { "" }
            );
        }
    }
    if !d.findings.is_empty() {
        println!("\nAchados:");
        for f in &d.findings {
            println!("  [{}] {}\n      baseado em: {}", f.severity, f.text, f.based_on.join(", "));
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

fn status_label(s: &PlanStatus) -> String {
    match s {
        PlanStatus::Pending => "SERÁ APLICADA".into(),
        PlanStatus::AlreadyConfigured => "já configurada".into(),
        PlanStatus::Incompatible(r) => format!("incompatível: {r}"),
        PlanStatus::Blocked(r) => format!("bloqueada: {r}"),
        PlanStatus::ManualOnly => "orientação manual".into(),
        PlanStatus::Rejected => "rejeitada".into(),
        PlanStatus::NotSelected => "não selecionada".into(),
    }
}

fn print_plan(plan: &[PlannedAction]) {
    for p in plan {
        println!("\n● {} [{}]", p.def.title, p.def.id);
        println!("  estado: {}", status_label(&p.status));
        println!("  evidência: {} · reinício: {}", p.def.evidence.label(), p.def.restart.label());
        if matches!(p.status, PlanStatus::Pending | PlanStatus::AlreadyConfigured) {
            if !p.targets.is_empty() {
                println!("  alvos: {}", p.targets.join(", "));
            }
            for o in &p.ops {
                let mark = if o.satisfied { "=" } else { "→" };
                println!("    {mark} {}  (atual: {})", o.op.describe(), o.current);
            }
        }
    }
}

fn print_action_doc(x: &prumo_core::model::ActionDef) {
    println!("{} [{} v{}]\n", x.title, x.id, x.version);
    let rows = [
        ("1. Problema", &x.problem),
        ("2. Compatibilidade", &x.compat),
        ("3. Quando faz sentido", &x.when),
        ("4. Alteração exata", &x.change),
        ("5. Benefício esperado", &x.benefit),
        ("6. Efeitos negativos", &x.side_effects),
        ("8. Como desfazer", &x.undo),
        ("9. Validação", &x.validation),
    ];
    for (t, v) in rows {
        println!("{t}\n   {v}\n");
    }
    println!("7. Privilégio/reinício\n   {:?} · {}\n", x.privilege, x.restart.label());
    println!("Evidência: {} · Nível: {:?}\n", x.evidence.label(), x.tier);
    println!("10. Fontes");
    for s in &x.sources {
        println!("   - {} ({}), consultado em {}\n     {}", s.title, s.url, s.consulted, s.says);
    }
}
