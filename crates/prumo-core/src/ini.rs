//! Edição mínima de arquivos INI que preserva comentários, ordem e todas as
//! linhas que não foram tocadas (instance.cfg do Prism, basic.ini do OBS).

/// Lê o valor de `key` em `section` (None = antes da primeira seção).
pub fn get(text: &str, section: Option<&str>, key: &str) -> Option<String> {
    let mut current: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(name) = section_name(t) {
            current = Some(name.to_string());
            continue;
        }
        if current.as_deref() == section {
            if let Some((k, v)) = split_kv(t) {
                if k == key {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// Define `key=value`, substituindo a primeira ocorrência ou acrescentando
/// ao fim da seção (criando a seção se necessário). `None` remove a chave.
pub fn set(text: &str, section: Option<&str>, key: &str, value: Option<&str>) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 2);
    let mut current: Option<String> = None;
    let mut done = false;
    let mut section_seen = section.is_none();
    let mut insert_at: Option<usize> = if section.is_none() { Some(0) } else { None };

    for line in &lines {
        let t = line.trim();
        if let Some(name) = section_name(t) {
            current = Some(name.to_string());
            if section == Some(name) {
                section_seen = true;
            }
            out.push(line.to_string());
            if current.as_deref() == section {
                insert_at = Some(out.len());
            }
            continue;
        }
        if current.as_deref() == section {
            if let Some((k, _)) = split_kv(t) {
                if k == key && !done {
                    done = true;
                    if let Some(v) = value {
                        out.push(format!("{key}={v}"));
                    }
                    continue;
                }
            }
            if !t.is_empty() {
                insert_at = Some(out.len());
            }
        }
        out.push(line.to_string());
    }

    if !done {
        if let Some(v) = value {
            let kv = format!("{key}={v}");
            match (section_seen, insert_at) {
                (true, Some(i)) => out.insert(i, kv),
                _ => {
                    if let Some(s) = section {
                        if out.last().is_some_and(|l| !l.trim().is_empty()) {
                            out.push(String::new());
                        }
                        out.push(format!("[{s}]"));
                    }
                    out.push(kv);
                }
            }
        }
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') || text.is_empty() {
        s.push('\n');
    }
    s
}

fn section_name(t: &str) -> Option<&str> {
    t.strip_prefix('[')?.strip_suffix(']')
}

fn split_kv(t: &str) -> Option<(&str, &str)> {
    if t.starts_with('#') || t.starts_with(';') {
        return None;
    }
    let (k, v) = t.split_once('=')?;
    Some((k.trim(), v.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "[General]\nname=GTNH\nMaxMemAlloc=14096\n# comentário\nJvmArgs=\"-Dx=1\"\n";

    #[test]
    fn get_reads_section_value() {
        assert_eq!(get(CFG, Some("General"), "MaxMemAlloc").as_deref(), Some("14096"));
        assert_eq!(get(CFG, Some("General"), "Missing"), None);
        assert_eq!(get(CFG, Some("Other"), "name"), None);
    }

    #[test]
    fn set_replaces_and_preserves_rest() {
        let out = set(CFG, Some("General"), "MaxMemAlloc", Some("6144"));
        assert!(out.contains("MaxMemAlloc=6144\n"));
        assert!(out.contains("# comentário"));
        assert!(out.contains("JvmArgs=\"-Dx=1\""));
        assert_eq!(out.lines().count(), CFG.lines().count());
    }

    #[test]
    fn set_appends_inside_section_and_removes() {
        let out = set(CFG, Some("General"), "OverrideMemory", Some("true"));
        assert_eq!(get(&out, Some("General"), "OverrideMemory").as_deref(), Some("true"));
        let back = set(&out, Some("General"), "OverrideMemory", None);
        assert_eq!(back, CFG);
    }

    #[test]
    fn set_creates_missing_section() {
        let out = set("", Some("General"), "a", Some("1"));
        assert_eq!(out, "[General]\na=1\n");
    }
}
