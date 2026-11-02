//! Catálogo embutido no binário. O executor só aplica o que está aqui, então
//! nenhum arquivo externo consegue mandar o app escrever conteúdo arbitrário.

use crate::model::{ActionDef, Platform, Privilege, StepDef, Tier};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

const LINUX: &str = include_str!("../../../catalog/linux.toml");
const WINDOWS: &str = include_str!("../../../catalog/windows.toml");

#[derive(Deserialize)]
struct File {
    action: Vec<ActionDef>,
}

pub fn load() -> Result<Vec<ActionDef>> {
    let mut all = vec![];
    for (name, text) in [("linux.toml", LINUX), ("windows.toml", WINDOWS)] {
        let f: File = toml::from_str(text).with_context(|| format!("catálogo {name} inválido"))?;
        all.extend(f.action);
    }
    validate(&all)?;
    Ok(all)
}

pub fn for_platform(all: &[ActionDef], p: Platform) -> Vec<ActionDef> {
    all.iter().filter(|a| a.platform == p).cloned().collect()
}

pub fn current_platform() -> Platform {
    if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

pub fn validate(all: &[ActionDef]) -> Result<()> {
    let mut ids = HashSet::new();
    for a in all {
        if !ids.insert(a.id.as_str()) {
            bail!("id duplicado: {}", a.id);
        }
    }
    for a in all {
        for r in a.depends.iter().chain(a.conflicts.iter()) {
            if !ids.contains(r.as_str()) {
                bail!("{}: referência desconhecida {r}", a.id);
            }
        }
        if matches!(a.tier, Tier::Rejected | Tier::Manual) && !a.steps.is_empty() {
            bail!("{}: ações rejeitadas/manuais não podem ter passos", a.id);
        }
        if a.sources.is_empty() && a.tier != Tier::Rejected {
            bail!("{}: sem fontes", a.id);
        }
        for (k, p) in &a.params {
            if !p.default.starts_with("fact:") {
                check_param(&p.default, &p.allowed).with_context(|| format!("{}: padrão inválido de {k}", a.id))?;
            }
        }
        if a.privilege == Privilege::Admin {
            for s in &a.steps {
                let path = match s {
                    StepDef::WriteFile { path, .. } | StepDef::IniSet { path, .. } | StepDef::CopyIfMissing { to: path, .. } => path.as_str(),
                    _ => continue,
                };
                if path.contains("{{home}}") || path.contains("{{config}}") {
                    bail!("{}: ação administrativa não pode escrever na pasta do usuário", a.id);
                }
            }
        }
    }
    Ok(())
}

/// Valida um valor de parâmetro: "int:MIN:MAX" ou "enum:a|b".
pub fn check_param(value: &str, allowed: &str) -> Result<()> {
    if let Some(r) = allowed.strip_prefix("int:") {
        let (lo, hi) = r.split_once(':').context("faixa inválida")?;
        let v: i64 = value.parse().context("não é inteiro")?;
        if v < lo.parse()? || v > hi.parse()? {
            bail!("{v} fora da faixa {lo}..{hi}");
        }
        return Ok(());
    }
    if let Some(r) = allowed.strip_prefix("enum:") {
        if r.split('|').any(|o| o == value) {
            return Ok(());
        }
        bail!("'{value}' não está em {r}");
    }
    bail!("regra de validação desconhecida: {allowed}")
}

/// Substitui {{nome}} por parâmetros e variáveis de ambiente conhecidas.
pub fn expand(template: &str, params: &BTreeMap<String, String>) -> String {
    let mut s = template.to_string();
    for (k, v) in params {
        s = s.replace(&format!("{{{{{k}}}}}"), v);
    }
    s
}

/// Avalia condições "chave=valor", "chave!=valor", "chave>=n", "chave<=n",
/// "chave~trecho" e "chave?" (existe). Devolve a primeira que falhar.
pub fn first_failed_condition(conds: &[String], facts: &BTreeMap<String, String>) -> Option<String> {
    for c in conds {
        let ok = if let Some(k) = c.strip_suffix('?') {
            facts.contains_key(k)
        } else if let Some((k, v)) = c.split_once("!=") {
            facts.get(k).map(|x| x != v).unwrap_or(true)
        } else if let Some((k, v)) = c.split_once(">=") {
            num(facts.get(k)).zip(v.parse::<f64>().ok()).is_some_and(|(a, b)| a >= b)
        } else if let Some((k, v)) = c.split_once("<=") {
            num(facts.get(k)).zip(v.parse::<f64>().ok()).is_some_and(|(a, b)| a <= b)
        } else if let Some((k, v)) = c.split_once('~') {
            facts.get(k).is_some_and(|x| x.to_lowercase().contains(&v.to_lowercase()))
        } else if let Some((k, v)) = c.split_once('=') {
            facts.get(k).is_some_and(|x| x == v)
        } else {
            false
        };
        if !ok {
            return Some(c.clone());
        }
    }
    None
}

fn num(v: Option<&String>) -> Option<f64> {
    v.and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_valid() {
        let all = load().expect("catálogo deve carregar");
        assert!(all.iter().any(|a| a.platform == Platform::Linux));
        assert!(all.iter().any(|a| a.platform == Platform::Windows));
        for a in &all {
            assert!(!a.problem.is_empty() && !a.undo.is_empty() && !a.validation.is_empty(), "{}", a.id);
        }
    }

    #[test]
    fn conditions() {
        let f: BTreeMap<String, String> =
            [("gpu.vendor", "nvidia"), ("mem.total_mb", "32000"), ("session.desktop", "niri")]
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect();
        assert_eq!(first_failed_condition(&["gpu.vendor=nvidia".into(), "mem.total_mb>=16000".into()], &f), None);
        assert_eq!(first_failed_condition(&["session.desktop~NIRI".into()], &f), None);
        assert_eq!(first_failed_condition(&["gpu.vendor=amd".into()], &f).as_deref(), Some("gpu.vendor=amd"));
        assert!(first_failed_condition(&["x?".into()], &f).is_some());
    }

    #[test]
    fn params_validation() {
        assert!(check_param("6144", "int:2048:16384").is_ok());
        assert!(check_param("99999", "int:2048:16384").is_err());
        assert!(check_param("a", "enum:a|b").is_ok());
        assert!(check_param("rm -rf", "enum:a|b").is_err());
    }
}
