//! Registro do Windows via reg.exe (ferramenta do sistema), sem FFI própria.
//! NÃO TESTADO em Windows real nesta etapa: ver docs/ETAPAS.md.

use crate::ops::Op;
use anyhow::{bail, Context, Result};
use std::process::Command;

fn parts(op: &Op) -> Result<(&str, &str, &str, &str)> {
    match op {
        Op::RegSet { path, name, kind, data } => Ok((path, name, kind, data)),
        _ => bail!("operação não é de registro"),
    }
}

pub fn read(op: &Op) -> Result<Option<(String, String)>> {
    let (path, name, _, _) = parts(op)?;
    let out = Command::new("reg")
        .args(["query", path, "/v", name])
        .output()
        .context("executar reg query")?;
    if !out.status.success() {
        return Ok(None); // chave ou valor ausente
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 3 && cols[0].eq_ignore_ascii_case(name) && cols[1].starts_with("REG_") {
            let kind = cols[1].to_string();
            let raw = cols[2..].join(" ");
            let data = if kind == "REG_DWORD" {
                let hex = raw.trim_start_matches("0x");
                u32::from_str_radix(hex, 16).map(|v| v.to_string()).unwrap_or(raw)
            } else {
                raw
            };
            return Ok(Some((data, kind)));
        }
    }
    Ok(None)
}

pub fn write(op: &Op, restore: Option<(Option<&str>, Option<&str>)>) -> Result<()> {
    let (path, name, kind, data) = parts(op)?;
    let (data, kind) = match restore {
        None => (Some(data), kind),
        Some((prev, prev_kind)) => (prev, prev_kind.unwrap_or(kind)),
    };
    let status = match data {
        Some(d) => Command::new("reg").args(["add", path, "/v", name, "/t", kind, "/d", d, "/f"]).status(),
        None => Command::new("reg").args(["delete", path, "/v", name, "/f"]).status(),
    }
    .context("executar reg.exe")?;
    if !status.success() {
        bail!("reg.exe falhou ({status})");
    }
    Ok(())
}
