//! Medição de frame time a partir de CSV do MangoHud ou do PresentMon.
//!
//! Stutter (definição do projeto): quadro com mais que o dobro da mediana dos
//! 60 quadros anteriores e acima de 25 ms. Os primeiros 60 quadros só servem de base.
//! O app só calcula sobre o arquivo; nada roda durante o jogo.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const WINDOW: usize = 60;
const STUTTER_RATIO: f64 = 2.0;
const STUTTER_MIN_MS: f64 = 25.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameStats {
    pub name: String,
    pub frames: usize,
    pub duration_s: f64,
    pub avg_fps: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    /// FPS equivalente à média do pior 1% dos quadros.
    pub low_1pct_fps: f64,
    pub stutters: usize,
    pub stutters_per_min: f64,
}

/// Extrai os frame times (ms). Acha a linha de cabeçalho pelo nome da coluna,
/// o que cobre o preâmbulo de sistema do MangoHud.
pub fn parse_csv(text: &str) -> Result<Vec<f64>> {
    let mut lines = text.lines();
    let mut col = None;
    for l in lines.by_ref() {
        let found = l.split(',').position(|c| {
            let c = c.trim().to_lowercase();
            c == "frametime" || c == "msbetweenpresents"
        });
        if found.is_some() {
            col = found;
            break;
        }
    }
    let Some(col) = col else { bail!("coluna de frame time não encontrada (frametime, FrameTime ou msBetweenPresents)") };
    let v: Vec<f64> = lines
        .filter_map(|l| l.split(',').nth(col)?.trim().parse::<f64>().ok())
        .filter(|x| x.is_finite() && *x > 0.0)
        .collect();
    if v.is_empty() {
        bail!("nenhum quadro no arquivo");
    }
    Ok(v)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn median(w: &[f64]) -> f64 {
    let mut s = w.to_vec();
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}

pub fn count_stutters(ft: &[f64]) -> usize {
    (WINDOW..ft.len())
        .filter(|&i| ft[i] > STUTTER_MIN_MS && ft[i] > STUTTER_RATIO * median(&ft[i - WINDOW..i]))
        .count()
}

pub fn stats(name: &str, ft: &[f64]) -> FrameStats {
    let mut s = ft.to_vec();
    s.sort_by(f64::total_cmp);
    let total_ms: f64 = ft.iter().sum();
    let worst = (s.len() / 100).max(1);
    let worst_avg = s[s.len() - worst..].iter().sum::<f64>() / worst as f64;
    let stutters = count_stutters(ft);
    FrameStats {
        name: name.into(),
        frames: ft.len(),
        duration_s: total_ms / 1000.0,
        avg_fps: ft.len() as f64 * 1000.0 / total_ms,
        p50_ms: percentile(&s, 0.50),
        p95_ms: percentile(&s, 0.95),
        p99_ms: percentile(&s, 0.99),
        max_ms: s[s.len() - 1],
        low_1pct_fps: 1000.0 / worst_avg,
        stutters,
        stutters_per_min: stutters as f64 * 60_000.0 / total_ms,
    }
}

pub fn load(path: &Path) -> Result<FrameStats> {
    let text = std::fs::read_to_string(path).with_context(|| format!("ler {}", path.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Ok(stats(&name, &parse_csv(&text).with_context(|| path.display().to_string())?))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Relatório HTML offline: sem scripts nem recursos externos.
pub fn html(runs: &[FrameStats]) -> String {
    let rows: String = runs
        .iter()
        .map(|r| {
            format!(
                "<tr><th scope=row>{}</th><td>{}</td><td>{:.0} s</td><td>{:.1}</td><td>{:.1}</td><td>{:.1}</td><td>{:.1}</td><td>{:.1}</td><td>{:.1}</td><td>{} ({:.1}/min)</td></tr>",
                esc(&r.name), r.frames, r.duration_s, r.avg_fps, r.low_1pct_fps, r.p50_ms, r.p95_ms, r.p99_ms, r.max_ms, r.stutters, r.stutters_per_min
            )
        })
        .collect();
    format!(
        "<!DOCTYPE html><html lang=pt-BR><meta charset=utf-8><title>Prumo: medição</title>\
<style>body{{font:16px system-ui;margin:2rem;max-width:70rem}}table{{border-collapse:collapse;width:100%}}\
th,td{{border-bottom:1px solid #8884;padding:.5rem;text-align:right;font-variant-numeric:tabular-nums}}\
th:first-child{{text-align:left}}</style><h1>Medição de frame time</h1>\
<table><thead><tr><th>Sessão</th><th>Quadros</th><th>Duração</th><th>FPS médio</th><th>1% low (FPS)</th>\
<th>p50 (ms)</th><th>p95 (ms)</th><th>p99 (ms)</th><th>Máx. (ms)</th><th>Stutters</th></tr></thead><tbody>{rows}</tbody></table>\
<p>Stutter: quadro acima de 25 ms e de 2× a mediana dos 60 anteriores. Uma diferença entre duas sessões só \
indica efeito se superar a variação entre repetições do mesmo trajeto.</p></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mangohud_with_preamble_and_presentmon() {
        let m = "os,cpu,gpu\nLinux,x,y\nfps,frametime,cpu_load\n60,16.6,10\n59,17.0,11\n";
        assert_eq!(parse_csv(m).unwrap(), vec![16.6, 17.0]);
        let p = "Application,ProcessID,msBetweenPresents\na,1,8.5\n";
        assert_eq!(parse_csv(p).unwrap(), vec![8.5]);
        assert!(parse_csv("a,b\n1,2\n").is_err());
    }

    #[test]
    fn stutter_needs_relative_and_absolute_spike() {
        let mut ft = vec![10.0; 100];
        ft[80] = 30.0; // conta
        ft[90] = 18.0; // quase o dobro, abaixo de 25 ms
        assert_eq!(count_stutters(&ft), 1);
        let mut slow = vec![20.0; 100]; // 30 ms não chega a 2× a mediana
        slow[80] = 30.0;
        assert_eq!(count_stutters(&slow), 0);
    }

    #[test]
    fn stats_on_known_series() {
        let mut ft = vec![10.0; 99];
        ft.push(110.0);
        let s = stats("x", &ft);
        assert_eq!((s.frames, s.max_ms, s.p50_ms), (100, 110.0, 10.0));
        assert!((s.avg_fps - 90.909).abs() < 0.01);
        assert!((s.low_1pct_fps - 9.09).abs() < 0.01);
    }

    #[test]
    fn html_escapes_name() {
        let s = stats("<b>", &[10.0]);
        assert!(html(&[s]).contains("&lt;b&gt;"));
    }
}
