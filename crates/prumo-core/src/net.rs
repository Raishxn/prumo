//! Medição de rede sob demanda (só quando o usuário roda `prumo rede`).
//!
//! - Recusa medir se já houver upload alto em andamento (ex.: live no ar).
//! - Latência/perda até o roteador e até 1.1.1.1 com `ping`.
//! - Upload: envia bytes aleatórios (nenhum dado do usuário) para o endpoint
//!   público de teste da Cloudflare com `curl`, várias vezes; guarda mediana e
//!   mínimo. O valor para live usa o MÍNIMO (upload sustentado no pior caso).

#[cfg(unix)]
use crate::diag::run;
use crate::ops::write_atomic;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const UPLOAD_URL: &str = "https://speed.cloudflare.com/__up";
const UPLOAD_MB: usize = 25;
const RUNS: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetMeasurement {
    pub at: String,
    pub interface: Option<String>,
    pub gateway: Option<String>,
    pub gateway_rtt_ms: Option<f64>,
    pub gateway_loss_pct: Option<f64>,
    pub internet_rtt_ms: Option<f64>,
    pub internet_jitter_ms: Option<f64>,
    pub internet_loss_pct: Option<f64>,
    pub upload_runs_mbps: Vec<f64>,
    pub upload_median_mbps: Option<f64>,
    pub upload_min_mbps: Option<f64>,
    pub method: String,
}

fn latest_path(state: &Path) -> PathBuf {
    state.join("medicoes").join("rede-ultima.json")
}

pub fn load_latest(state: &Path) -> Option<NetMeasurement> {
    serde_json::from_str(&std::fs::read_to_string(latest_path(state)).ok()?).ok()
}

/// Bitrate sugerido (kbps) para H.264 1080p60 no YouTube: no máximo 70% do
/// upload mínimo medido (folga para oscilação; heurística do projeto, não
/// regra do YouTube), limitado à faixa do YouTube (6–17 Mbps), em passos de 500.
pub fn suggested_h264_1080p60_kbps(upload_min_mbps: f64) -> Option<u32> {
    let k = ((upload_min_mbps * 0.7 * 1000.0) / 500.0).floor() as u32 * 500;
    let k = k.min(17_000);
    (k >= 6_000).then_some(k)
}

/// Upload atual da interface em Mb/s (Linux, sysfs), amostrado por 2 s.
#[cfg(unix)]
fn current_tx_mbps(iface: &str) -> Option<f64> {
    let p = format!("/sys/class/net/{iface}/statistics/tx_bytes");
    let a: u64 = std::fs::read_to_string(&p).ok()?.trim().parse().ok()?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    let b: u64 = std::fs::read_to_string(&p).ok()?.trim().parse().ok()?;
    Some((b.saturating_sub(a)) as f64 * 8.0 / 2.0 / 1e6)
}

#[cfg(unix)]
fn default_route() -> (Option<String>, Option<String>) {
    let out = run("ip", &["route", "show", "default"]).unwrap_or_default();
    let w: Vec<&str> = out.split_whitespace().collect();
    let after = |k: &str| w.iter().position(|x| *x == k).and_then(|i| w.get(i + 1)).map(|s| s.to_string());
    (after("dev"), after("via"))
}
#[cfg(not(unix))]
fn default_route() -> (Option<String>, Option<String>) {
    (None, None)
}

/// (rtt médio, jitter = mdev, perda %) a partir da saída do ping do Linux.
pub fn parse_ping(out: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    let loss = out
        .lines()
        .find(|l| l.contains("packet loss") || l.contains("perda"))
        .and_then(|l| l.split(',').find(|p| p.contains('%')))
        .and_then(|p| p.split('%').next())
        .and_then(|p| p.split_whitespace().last())
        .and_then(|v| v.replace(',', ".").parse().ok());
    let stats = out
        .lines()
        .find(|l| l.contains("min/avg/max"))
        .and_then(|l| l.split('=').nth(1))
        .map(|v| v.trim().trim_end_matches(" ms").split('/').filter_map(|x| x.trim().parse::<f64>().ok()).collect::<Vec<_>>())
        .unwrap_or_default();
    (stats.get(1).copied(), stats.get(3).copied(), loss)
}

fn ping(host: &str, n: u32) -> (Option<f64>, Option<f64>, Option<f64>) {
    let out = Command::new("ping").args(["-c", &n.to_string(), "-i", "0.2", "-W", "1", host]).output();
    match out {
        Ok(o) => parse_ping(&String::from_utf8_lossy(&o.stdout)),
        Err(_) => (None, None, None),
    }
}

fn upload_once(file: &Path) -> Result<f64> {
    let out = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "--max-time", "60", "-w", "%{speed_upload}", "--data-binary"])
        .arg(format!("@{}", file.display()))
        .arg(UPLOAD_URL)
        .output()
        .context("executar curl")?;
    if !out.status.success() {
        bail!("curl falhou ({})", out.status);
    }
    let bps: f64 = String::from_utf8_lossy(&out.stdout).trim().replace(',', ".").parse().context("saída do curl")?;
    Ok(bps * 8.0 / 1e6)
}

pub fn measure(state: &Path, progress: &dyn Fn(&str)) -> Result<NetMeasurement> {
    if cfg!(windows) {
        bail!("medição de rede no Windows entra na etapa 3 (ping/curl têm opções diferentes)");
    }
    let (iface, gw) = default_route();
    #[cfg(unix)]
    if let Some(i) = &iface {
        progress("conferindo se já há upload em andamento (2 s)…");
        if let Some(tx) = current_tx_mbps(i) {
            if tx > 1.5 {
                bail!("já há {tx:.1} Mb/s saindo por {i} (live ou envio em andamento?). Medição cancelada para não atrapalhar.");
            }
        }
    }
    let mut m = NetMeasurement {
        at: crate::now_iso(),
        interface: iface,
        gateway: gw.clone(),
        gateway_rtt_ms: None,
        gateway_loss_pct: None,
        internet_rtt_ms: None,
        internet_jitter_ms: None,
        internet_loss_pct: None,
        upload_runs_mbps: vec![],
        upload_median_mbps: None,
        upload_min_mbps: None,
        method: format!("ping (20 pacotes) roteador e 1.1.1.1; upload: curl POST de {UPLOAD_MB} MB aleatórios para {UPLOAD_URL}, {RUNS} vezes"),
    };
    if let Some(g) = &gw {
        progress("latência até o roteador…");
        let (rtt, _, loss) = ping(g, 20);
        m.gateway_rtt_ms = rtt;
        m.gateway_loss_pct = loss;
    }
    progress("latência até 1.1.1.1…");
    let (rtt, jit, loss) = ping("1.1.1.1", 20);
    (m.internet_rtt_ms, m.internet_jitter_ms, m.internet_loss_pct) = (rtt, jit, loss);

    let tmp = std::env::temp_dir().join(format!("prumo-upload-{}.bin", std::process::id()));
    {
        // Bytes pseudoaleatórios (xorshift) para a compressão não inflar o resultado.
        let mut buf = vec![0u8; UPLOAD_MB * 1024 * 1024];
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15 ^ std::process::id() as u64;
        for c in buf.chunks_mut(8) {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            c.copy_from_slice(&x.to_le_bytes()[..c.len()]);
        }
        std::fs::write(&tmp, &buf)?;
    }
    for i in 1..=RUNS {
        progress(&format!("upload {i}/{RUNS}…"));
        match upload_once(&tmp) {
            Ok(v) => m.upload_runs_mbps.push((v * 10.0).round() / 10.0),
            Err(e) => progress(&format!("  falhou: {e:#}")),
        }
    }
    let _ = std::fs::remove_file(&tmp);
    let mut v = m.upload_runs_mbps.clone();
    v.sort_by(|a, b| a.total_cmp(b));
    m.upload_min_mbps = v.first().copied();
    m.upload_median_mbps = v.get(v.len() / 2).copied();

    let dir = state.join("medicoes");
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_vec_pretty(&m)?;
    write_atomic(&dir.join(format!("rede-{}.json", crate::now_compact())), &json)?;
    write_atomic(&latest_path(state), &json)?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_parsing_linux() {
        let out = "20 packets transmitted, 20 received, 0% packet loss, time 3810ms\nrtt min/avg/max/mdev = 3.101/4.220/9.004/1.310 ms\n";
        assert_eq!(parse_ping(out), (Some(4.22), Some(1.31), Some(0.0)));
        let lossy = "20 packets transmitted, 19 received, 5% packet loss, time 3810ms\n";
        assert_eq!(parse_ping(lossy).2, Some(5.0));
    }

    #[test]
    fn suggested_bitrate_respects_youtube_range_and_headroom() {
        assert_eq!(suggested_h264_1080p60_kbps(100.0), Some(17_000), "teto do YouTube");
        assert_eq!(suggested_h264_1080p60_kbps(15.0), Some(10_500), "70% do upload");
        assert_eq!(suggested_h264_1080p60_kbps(8.0), None, "abaixo do mínimo de 6 Mbps");
    }
}
