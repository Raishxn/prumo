//! Testes do motor: idempotência, falha parcial, reversão, conflitos e
//! recuperação de sessão interrompida. Tudo em diretórios temporários.

use prumo_core::diag::Diagnosis;
use prumo_core::engine::{self, PlanStatus, Selection};
use prumo_core::journal::{EntryStatus, Outcome, Session};
use prumo_core::model::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn action(id: &str, steps: Vec<StepDef>) -> ActionDef {
    ActionDef {
        id: id.into(),
        version: 1,
        title: id.into(),
        platform: Platform::Linux,
        area: "teste".into(),
        tier: Tier::Recommended,
        evidence: Evidence::Hypothesis,
        privilege: Privilege::User,
        restart: Restart::None,
        problem: "p".into(),
        compat: "c".into(),
        when: "w".into(),
        conditions: vec![],
        change: "x".into(),
        benefit: "b".into(),
        side_effects: "s".into(),
        undo: "u".into(),
        validation: "v".into(),
        sources: vec![],
        depends: vec![],
        conflicts: vec![],
        params: BTreeMap::new(),
        steps,
    }
}

fn write(p: &Path, content: &str) -> StepDef {
    StepDef::WriteFile { path: p.to_string_lossy().into(), content: content.into(), mode: None }
}

fn ini(p: &Path, key: &str, value: &str) -> StepDef {
    StepDef::IniSet { path: p.to_string_lossy().into(), section: Some("General".into()), key: key.into(), value: value.into() }
}

fn run(cat: &[ActionDef], state: &Path) -> (Session, Vec<prumo_core::journal::ActionRecord>) {
    let plan = engine::plan(cat, &Diagnosis::default(), &Selection::default()).unwrap();
    let mut s = Session::create(state, "teste").unwrap();
    let r = engine::apply(&plan, &mut s).unwrap();
    (s, r)
}

#[test]
fn apply_then_undo_restores_real_previous_state() {
    let t = tempfile::tempdir().unwrap();
    let cfg = t.path().join("instance.cfg");
    let original = "[General]\nname=GTNH\nMaxMemAlloc=14096\n# manter\n";
    fs::write(&cfg, original).unwrap();
    let newfile = t.path().join("env/90-x.conf");
    let cat = vec![action("a", vec![ini(&cfg, "MaxMemAlloc", "6144"), ini(&cfg, "OverrideMemory", "true"), write(&newfile, "X=1\n")])];

    let (mut s, recs) = run(&cat, &t.path().join("state"));
    assert_eq!(recs[0].outcome, Outcome::Applied);
    let after = fs::read_to_string(&cfg).unwrap();
    assert!(after.contains("MaxMemAlloc=6144") && after.contains("OverrideMemory=true") && after.contains("# manter"));
    assert!(newfile.exists());

    let rep = engine::undo(&mut s, None, false).unwrap();
    assert_eq!(rep.restored.len(), 3, "{rep:?}");
    assert_eq!(fs::read_to_string(&cfg).unwrap(), original, "volta byte a byte ao original");
    assert!(!newfile.exists(), "arquivo criado pelo app é removido");
}

#[test]
fn second_run_is_idempotent() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("a.conf");
    let cat = vec![action("a", vec![write(&f, "A=1\n")])];
    let (_, r1) = run(&cat, &t.path().join("state"));
    assert_eq!(r1[0].outcome, Outcome::Applied);
    let plan = engine::plan(&cat, &Diagnosis::default(), &Selection::default()).unwrap();
    assert_eq!(plan[0].status, PlanStatus::AlreadyConfigured);
    let (s2, r2) = run(&cat, &t.path().join("state"));
    assert_eq!(r2[0].outcome, Outcome::AlreadyConfigured);
    assert!(s2.entries.is_empty(), "nenhuma alteração registrada na segunda vez");
}

#[test]
fn partial_failure_rolls_back_the_action_and_continues_others() {
    let t = tempfile::tempdir().unwrap();
    let good = t.path().join("good.conf");
    fs::write(&good, "ORIGINAL\n").unwrap();
    let blocker = t.path().join("arquivo-no-lugar-de-pasta");
    fs::write(&blocker, "x").unwrap();
    let impossible = blocker.join("filho.conf"); // pai é arquivo: falha garantida
    let other = t.path().join("other.conf");
    let mut dependent = action("c", vec![write(&t.path().join("c.conf"), "C\n")]);
    dependent.depends = vec!["a".into()];
    let cat = vec![
        action("a", vec![write(&good, "NOVO\n"), write(&impossible, "y")]),
        action("b", vec![write(&other, "B\n")]),
        dependent,
    ];
    let (s, recs) = run(&cat, &t.path().join("state"));
    let by = |id: &str| recs.iter().find(|r| r.action_id == id).unwrap().outcome;
    assert_eq!(by("a"), Outcome::Failed);
    assert_eq!(fs::read_to_string(&good).unwrap(), "ORIGINAL\n", "operação já feita foi revertida");
    assert_eq!(by("b"), Outcome::Applied, "ações independentes continuam");
    assert_eq!(by("c"), Outcome::Skipped, "dependente de ação que falhou é ignorada");
    assert!(s.entries.iter().any(|e| e.status == EntryStatus::RolledBack));
    assert!(s.entries.iter().any(|e| e.status == EntryStatus::Failed));
}

#[test]
fn undo_refuses_to_overwrite_later_external_change_without_force() {
    let t = tempfile::tempdir().unwrap();
    let cfg = t.path().join("instance.cfg");
    fs::write(&cfg, "[General]\nMaxMemAlloc=14096\n").unwrap();
    let cat = vec![action("a", vec![ini(&cfg, "MaxMemAlloc", "6144")])];
    let (mut s, _) = run(&cat, &t.path().join("state"));
    // O usuário muda pelo Prism depois da aplicação.
    fs::write(&cfg, "[General]\nMaxMemAlloc=8192\n").unwrap();
    let rep = engine::undo(&mut s, None, false).unwrap();
    assert_eq!(rep.conflicts.len(), 1);
    assert!(fs::read_to_string(&cfg).unwrap().contains("8192"), "não sobrescreve a escolha do usuário");
    let rep = engine::undo(&mut s, None, true).unwrap();
    assert_eq!(rep.restored.len(), 1);
    assert!(fs::read_to_string(&cfg).unwrap().contains("14096"));
}

#[test]
fn interrupted_session_can_be_restored_from_journal() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("a.conf");
    fs::write(&f, "ANTES\n").unwrap();
    let cat = vec![action("a", vec![write(&f, "DEPOIS\n")])];
    let (s, _) = run(&cat, &t.path().join("state"));
    // Simula queda: entrada volta a Intent como se o processo tivesse morrido
    // entre gravar o estado anterior e confirmar a alteração.
    let mut j: serde_json::Value = serde_json::from_str(&fs::read_to_string(s.dir.join("journal.json")).unwrap()).unwrap();
    j["entries"][0]["status"] = "intent".into();
    fs::write(s.dir.join("journal.json"), serde_json::to_vec(&j).unwrap()).unwrap();

    let mut loaded = Session::load(&s.dir).unwrap();
    assert_eq!(loaded.interrupted().len(), 1);
    let rep = engine::undo(&mut loaded, None, false).unwrap();
    assert_eq!(rep.restored.len(), 1);
    assert_eq!(fs::read_to_string(&f).unwrap(), "ANTES\n");
}

#[test]
fn undo_single_action_leaves_others() {
    let t = tempfile::tempdir().unwrap();
    let (fa, fb) = (t.path().join("a"), t.path().join("b"));
    let cat = vec![action("a", vec![write(&fa, "A")]), action("b", vec![write(&fb, "B")])];
    let (mut s, _) = run(&cat, &t.path().join("state"));
    engine::undo(&mut s, Some("a"), false).unwrap();
    assert!(!fa.exists());
    assert!(fb.exists());
}

#[test]
fn conflicts_and_rejected_are_never_applied() {
    let t = tempfile::tempdir().unwrap();
    let mut b = action("b", vec![write(&t.path().join("b"), "B")]);
    b.conflicts = vec!["a".into()];
    let mut r = action("r", vec![]);
    r.tier = Tier::Rejected;
    let cat = vec![action("a", vec![write(&t.path().join("a"), "A")]), b, r];
    let sel = Selection { ids: Some(vec!["a".into(), "b".into(), "r".into()]), ..Default::default() };
    let plan = engine::plan(&cat, &Diagnosis::default(), &sel).unwrap();
    assert_eq!(plan[0].status, PlanStatus::Pending);
    assert!(matches!(plan[1].status, PlanStatus::Incompatible(_)));
    assert_eq!(plan[2].status, PlanStatus::Rejected);
    let mut s = Session::create(&t.path().join("state"), "teste").unwrap();
    engine::apply(&plan, &mut s).unwrap();
    assert!(!t.path().join("b").exists());
}

#[test]
fn backup_tamper_is_detected_on_restore() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("a.conf");
    fs::write(&f, "ORIGINAL").unwrap();
    let cat = vec![action("a", vec![write(&f, "NOVO")])];
    let (mut s, _) = run(&cat, &t.path().join("state"));
    fs::write(s.backup_dir().join("0001.bak"), "ADULTERADO").unwrap();
    let rep = engine::undo(&mut s, None, false).unwrap();
    assert_eq!(rep.errors.len(), 1, "hash do backup não confere: não restaura lixo");
    assert_eq!(fs::read_to_string(&f).unwrap(), "NOVO");
}

#[test]
fn invalid_param_override_is_rejected() {
    let mut a = action("a", vec![]);
    a.params.insert("heap".into(), ParamDef { default: "6144".into(), description: "".into(), allowed: "int:4096:12288".into() });
    let sel = Selection { ids: Some(vec!["a".into()]), params: [("a.heap".to_string(), "999999".to_string())].into() };
    assert!(engine::plan(&[a], &Diagnosis::default(), &sel).is_err());
}
