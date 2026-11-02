//! Diagnóstico: só registra o que foi lido de fato, com a fonte. O que não
//! pôde ser lido aparece como desconhecido, com o motivo.

use serde::Serialize;
use std::collections::BTreeMap;
use std::process::Command;

#[cfg(unix)]
pub mod linux;
#[cfg(windows)]
pub mod windows;

#[derive(Debug, Clone, Serialize)]
pub struct Fact {
    pub value: Option<String>,
    /// De onde veio (arquivo, comando) ou por que é desconhecido.
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Diagnosis {
    pub collected_at: String,
    pub facts: BTreeMap<String, Fact>,
    /// Observações derivadas de fatos (ex.: pressão de memória), cada uma
    /// citando os fatos em que se baseia.
    pub findings: Vec<Finding>,
    pub minecraft: Vec<crate::prism::Instance>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub id: String,
    pub severity: String,
    pub text: String,
    pub based_on: Vec<String>,
}

impl Diagnosis {
    pub fn known(&mut self, key: &str, value: impl Into<String>, source: &str) {
        let v: String = value.into();
        let v = v.trim().to_string();
        if v.is_empty() {
            self.unknown(key, &format!("{source}: vazio"));
        } else {
            self.facts.insert(key.into(), Fact { value: Some(v), source: source.into() });
        }
    }

    pub fn unknown(&mut self, key: &str, why: &str) {
        self.facts.insert(key.into(), Fact { value: None, source: why.into() });
    }

    pub fn opt(&mut self, key: &str, value: Option<String>, source: &str, why_missing: &str) {
        match value {
            Some(v) if !v.trim().is_empty() => self.known(key, v, source),
            _ => self.unknown(key, why_missing),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.facts.get(key).and_then(|f| f.value.as_deref())
    }

    pub fn num(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Mapa simples chave→valor usado nas condições do catálogo.
    pub fn fact_map(&self) -> BTreeMap<String, String> {
        self.facts
            .iter()
            .filter_map(|(k, f)| f.value.clone().map(|v| (k.clone(), v)))
            .collect()
    }
}

/// Executa um comando e devolve stdout se ele terminar com sucesso.
pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn collect() -> Diagnosis {
    let mut d = Diagnosis { collected_at: crate::now_iso(), ..Default::default() };
    #[cfg(unix)]
    linux::collect(&mut d);
    #[cfg(windows)]
    windows::collect(&mut d);
    derive_findings(&mut d);
    d
}

/// Regras de achados. Cada regra cita os fatos usados e não infere nada além.
pub fn derive_findings(d: &mut Diagnosis) {
    let mut f = vec![];
    if let (Some(used), Some(total)) = (d.num("swap.used_mb"), d.num("mem.total_mb")) {
        if used > total * 0.15 {
            f.push(Finding {
                id: "memoria.swap-alto".into(),
                severity: "atenção".into(),
                text: format!(
                    "{used:.0} MB em swap agora (RAM total {total:.0} MB). Páginas de programas em swap \
                     precisam voltar para a RAM quando usadas, o que pode causar pausas. Veja quais processos \
                     ocupam mais memória antes de jogar; isto não prova que o swap é a causa do stuttering."
                ),
                based_on: vec!["swap.used_mb".into(), "mem.total_mb".into()],
            });
        }
    }
    if let Some(mb) = d.num("nvidia.glcache_mb") {
        if mb >= 900.0 {
            f.push(Finding {
                id: "nvidia.glcache-cheio".into(),
                severity: "atenção".into(),
                text: format!(
                    "Cache de shaders OpenGL da NVIDIA com {mb:.0} MB. O tamanho padrão do cache é limitado; \
                     perto do limite, shaders antigos podem ser descartados e recompilados (possível engasgo)."
                ),
                based_on: vec!["nvidia.glcache_mb".into()],
            });
        }
    }
    if let (Some(w), Some(gm), Some(pm)) = (d.num("gpu.pcie.width"), d.num("gpu.pcie.width_max"), d.num("gpu.pcie.port_width_max")) {
        if w < gm.min(pm) {
            f.push(Finding {
                id: "gpu.pcie-largura-reduzida".into(),
                severity: "atenção".into(),
                text: format!(
                    "O link PCIe da GPU está em x{w:.0}, mas a placa de vídeo e a porta suportam x{:.0}. \
                     Confirme com o jogo aberto (a largura normalmente não muda com economia de energia). \
                     Causas comuns: placa mal encaixada, contatos sujos, opção de divisão de lanes \
                     (bifurcação) na BIOS. Isto é hardware: o app não altera nada.",
                    gm.min(pm)
                ),
                based_on: vec!["gpu.pcie.width".into(), "gpu.pcie.width_max".into(), "gpu.pcie.port_width_max".into()],
            });
        }
    }
    if d.get("display.vrr") == Some("not supported")
        && d.get("display.connector").is_some_and(|c| c.starts_with("HDMI"))
        && d.get("display.edid_freesync_amd") == Some("sim")
        && d.get("display.edid_hdmi_vrr") == Some("não")
    {
        f.push(Finding {
            id: "monitor.vrr-pela-hdmi".into(),
            severity: "atenção".into(),
            text: format!(
                "O monitor anuncia faixa de {} e FreeSync (bloco AMD), mas não o VRR do HDMI 2.1. \
                 A NVIDIA aceita G-SYNC Compatible por DisplayPort (Pascal+) e por HDMI só com VRR do \
                 HDMI Forum, por isso o VRR está indisponível nesta conexão. Ver ação linux.monitor.vrr-displayport.",
                d.get("display.edid_range").unwrap_or("?")
            ),
            based_on: vec!["display.vrr".into(), "display.connector".into(), "display.edid_freesync_amd".into(), "display.edid_hdmi_vrr".into()],
        });
    }
    if d.get("secureboot") == Some("desativado") {
        f.push(Finding {
            id: "boot.secureboot-off".into(),
            severity: "informação".into(),
            text: "Secure Boot está desativado no firmware. O Riot Vanguard pode exigir Secure Boot e TPM 2.0 \
                   no Windows 11 (página 'Vanguard Security Requirements'). Nada é alterado automaticamente; \
                   ligar Secure Boot pode impedir o boot do CachyOS se o bootloader/kernel não estiverem assinados."
                .into(),
            based_on: vec!["secureboot".into()],
        });
    }
    if let Some(running) = d.minecraft.iter().find(|i| i.running) {
        f.push(Finding {
            id: "minecraft.instancia-aberta".into(),
            severity: "informação".into(),
            text: format!(
                "A instância '{}' está aberta agora; alterações nela serão bloqueadas até fechar o jogo.",
                running.name
            ),
            based_on: vec!["minecraft".into()],
        });
    }
    d.findings = f;
}
