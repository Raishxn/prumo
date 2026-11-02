//! Leitura (somente leitura) das instâncias do Prism Launcher.

use crate::ini;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Instance {
    pub dir: PathBuf,
    pub folder: String,
    pub name: String,
    pub minecraft: Option<String>,
    pub loader: Option<String>,
    pub components: Vec<String>,
    /// Versão de Java registrada pelo Prism (JavaVersion=), se houver.
    pub java_version: Option<String>,
    pub java_major: Option<u32>,
    pub java_path: Option<String>,
    pub override_memory: bool,
    pub min_mem_mb: Option<u32>,
    pub max_mem_mb: Option<u32>,
    pub override_java_args: bool,
    pub jvm_args: Option<String>,
    /// Há um processo Java rodando a partir desta instância agora?
    pub running: bool,
}

impl Instance {
    pub fn cfg_path(&self) -> PathBuf {
        self.dir.join("instance.cfg")
    }
}

pub fn default_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PRUMO_PRISM_ROOT") {
        return Some(PathBuf::from(p));
    }
    #[cfg(unix)]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .ok()?;
        let p = base.join("PrismLauncher");
        p.exists().then_some(p)
    }
    #[cfg(windows)]
    {
        let p = PathBuf::from(std::env::var("APPDATA").ok()?).join("PrismLauncher");
        p.exists().then_some(p)
    }
}

/// Java major a partir de "17.0.15", "21.0.7", "1.8.0_202", "26".
pub fn java_major(v: &str) -> Option<u32> {
    let mut it = v.split(['.', '_', '+', '-']);
    let first: u32 = it.next()?.parse().ok()?;
    if first == 1 {
        it.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// Normaliza separadores (e caixa, no Windows) para comparar caminhos.
fn norm(s: &str) -> String {
    let s = s.replace('\\', "/");
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// Interpreta a lista de linhas de comando dos processos (uma por linha).
/// Lista vazia significa que a consulta falhou, pois o próprio processo que
/// consulta sempre aparece nela: devolve `None` em vez de "nada rodando".
pub fn parse_process_list(out: &str) -> Option<Vec<String>> {
    let v: Vec<String> = out.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    (!v.is_empty()).then_some(v)
}

pub fn scan(root: &Path, running_cmdlines: &[String]) -> Vec<Instance> {
    let global = fs::read_to_string(root.join("prismlauncher.cfg")).unwrap_or_default();
    let mut out = vec![];
    let Ok(rd) = fs::read_dir(root.join("instances")) else { return out };
    for e in rd.flatten() {
        let dir = e.path();
        let Ok(cfg) = fs::read_to_string(dir.join("instance.cfg")) else { continue };
        let folder = e.file_name().to_string_lossy().to_string();
        let g = |k: &str| ini::get(&cfg, Some("General"), k).or_else(|| ini::get(&cfg, None, k));
        let gb = |k: &str| g(k).is_some_and(|v| v == "true");
        let pack: serde_json::Value = fs::read_to_string(dir.join("mmc-pack.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let comps: Vec<(String, Option<String>)> = pack["components"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|c| (c["uid"].as_str().unwrap_or("").to_string(), c["version"].as_str().map(String::from)))
                    .collect()
            })
            .unwrap_or_default();
        let minecraft = comps.iter().find(|c| c.0 == "net.minecraft").and_then(|c| c.1.clone());
        let loader = comps
            .iter()
            .find(|c| matches!(c.0.as_str(), "net.minecraftforge" | "net.neoforged" | "net.fabricmc.fabric-loader" | "org.quiltmc.quilt-loader"))
            .map(|c| format!("{} {}", c.0, c.1.clone().unwrap_or_default()));
        let override_memory = gb("OverrideMemory");
        let override_java_args = gb("OverrideJavaArgs");
        let mem = |k: &str| {
            if override_memory { g(k) } else { ini::get(&global, Some("General"), k).or_else(|| ini::get(&global, None, k)) }
                .and_then(|v| v.parse().ok())
        };
        let jvm_args = if override_java_args {
            g("JvmArgs")
        } else {
            ini::get(&global, Some("General"), "JvmArgs").or_else(|| ini::get(&global, None, "JvmArgs"))
        };
        let java_version = g("JavaVersion");
        let dir_s = norm(&dir.to_string_lossy());
        out.push(Instance {
            name: g("name").unwrap_or_else(|| folder.clone()),
            folder,
            minecraft,
            loader,
            components: comps.iter().map(|c| c.0.clone()).collect(),
            java_major: java_version.as_deref().and_then(java_major),
            java_version,
            java_path: g("JavaPath"),
            override_memory,
            min_mem_mb: mem("MinMemAlloc"),
            max_mem_mb: mem("MaxMemAlloc"),
            override_java_args,
            jvm_args,
            running: running_cmdlines.iter().any(|c| norm(c).contains(&dir_s)),
            dir,
        });
    }
    out.sort_by(|a, b| a.folder.cmp(&b.folder));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_major_parses_known_formats() {
        assert_eq!(java_major("1.8.0_202"), Some(8));
        assert_eq!(java_major("17.0.15"), Some(17));
        assert_eq!(java_major("21.0.12.1"), Some(21));
        assert_eq!(java_major("26"), Some(26));
        assert_eq!(java_major("x"), None);
    }

    #[test]
    fn empty_process_list_is_failure_not_nothing_running() {
        assert_eq!(parse_process_list(""), None);
        assert_eq!(parse_process_list(" \r\n\r\n"), None);
        assert_eq!(parse_process_list("a b\r\n\r\nc\r\n").unwrap(), vec!["a b", "c"]);
    }

    #[test]
    fn scan_reads_instance_and_detects_running() {
        let t = tempfile::tempdir().unwrap();
        let inst = t.path().join("instances/GTNH");
        fs::create_dir_all(&inst).unwrap();
        fs::write(t.path().join("prismlauncher.cfg"), "[General]\nMaxMemAlloc=14096\nMinMemAlloc=512\n").unwrap();
        fs::write(inst.join("instance.cfg"), "[General]\nname=GTNH 2.9\nJavaVersion=25.0.4\n").unwrap();
        fs::write(
            inst.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.7.10"},{"uid":"me.eigenraven.lwjgl3ify.forgepatches"}]}"#,
        )
        .unwrap();
        let running = vec![format!("java -Djava.library.path={}/natives", inst.display())];
        let v = scan(t.path(), &running);
        assert_eq!(v.len(), 1);
        let i = &v[0];
        assert_eq!(i.minecraft.as_deref(), Some("1.7.10"));
        assert_eq!(i.java_major, Some(25));
        assert_eq!(i.max_mem_mb, Some(14096), "herda o global quando OverrideMemory=false");
        assert!(i.running);
    }
}
