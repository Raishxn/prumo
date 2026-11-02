//! Tipos do catálogo de otimizações (definições declarativas, compartilhadas
//! entre o aplicativo e o playbook do AME Wizard).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Linux,
    Windows,
}

/// Classificação da evidência, exatamente como pedida no projeto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    /// Documentação oficial descreve o funcionamento (não prova ganho).
    OfficialDoc,
    /// Benefício demonstrado por medição externa relevante.
    ExternalMeasurement,
    /// Benefício medido neste computador.
    MeasuredHere,
    /// Hipótese ainda não validada.
    Hypothesis,
    /// Ajuste sem justificativa suficiente: rejeitado, nunca aplicado.
    Rejected,
}

impl Evidence {
    pub fn label(self) -> &'static str {
        match self {
            Evidence::OfficialDoc => "Documentação oficial do funcionamento (não prova ganho)",
            Evidence::ExternalMeasurement => "Benefício medido externamente",
            Evidence::MeasuredHere => "Benefício medido neste computador",
            Evidence::Hypothesis => "Hipótese não validada",
            Evidence::Rejected => "Rejeitado",
        }
    }
}

/// Em que parte do produto a ação aparece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Entra no plano recomendado de aplicação única.
    Recommended,
    /// Área avançada/experimental: só com seleção explícita.
    Advanced,
    /// Orientação manual (BIOS, hardware, decisões do usuário): o app não altera nada.
    Manual,
    /// Documentado como rejeitado; o motor recusa aplicar.
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    User,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Restart {
    None,
    /// Reiniciar o programa afetado (ex.: Minecraft, OBS).
    App,
    /// Encerrar e reabrir a sessão gráfica.
    Logout,
    Reboot,
}

impl Restart {
    pub fn label(self) -> &'static str {
        match self {
            Restart::None => "Não precisa reiniciar",
            Restart::App => "Reabrir o programa afetado",
            Restart::Logout => "Encerrar e reabrir a sessão",
            Restart::Reboot => "Reiniciar o computador",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub title: String,
    pub url: String,
    /// Data de consulta (AAAA-MM-DD).
    pub consulted: String,
    /// Trecho ou resumo do que a fonte realmente afirma.
    #[serde(default)]
    pub says: String,
}

/// Parâmetro ajustável de uma ação, com validação explícita. O executor
/// privilegiado só aceita valores que passem por `allowed`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    /// Valor fixo, ou "fact:<chave>" para usar um valor do diagnóstico
    /// (ex.: bitrate sugerido a partir do upload medido).
    pub default: String,
    pub description: String,
    /// Validação: "int:MIN:MAX" ou "enum:a|b|c".
    pub allowed: String,
}

/// Seletor de instâncias do Prism Launcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSelector {
    /// Versão do Minecraft exigida (ex.: "1.7.10").
    pub minecraft: String,
    /// Componente que precisa existir no mmc-pack.json (ex.: lwjgl3ify).
    #[serde(default)]
    pub requires_component: Option<String>,
    /// Termos que devem aparecer no nome da instância (qualquer um).
    #[serde(default)]
    pub name_contains_any: Vec<String>,
    /// Versão mínima de Java da instância (major).
    #[serde(default)]
    pub min_java: Option<u32>,
}

/// Passo declarativo. É expandido em operações concretas (`ops::Op`) na hora
/// do planejamento, com base no diagnóstico.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepDef {
    /// Escreve um arquivo inteiro (normalmente um drop-in próprio do app).
    WriteFile {
        path: String,
        content: String,
        #[serde(default)]
        mode: Option<u32>,
    },
    /// Define uma chave em arquivo INI preservando o resto do arquivo.
    IniSet {
        path: String,
        #[serde(default)]
        section: Option<String>,
        key: String,
        value: String,
    },
    /// Define chaves em instance.cfg de cada instância do Prism selecionada.
    PrismInstance {
        selector: InstanceSelector,
        keys: BTreeMap<String, String>,
    },
    /// Copia um arquivo só se o destino não existir (conteúdo fora do diário).
    CopyIfMissing { from: String, to: String },
    /// Valor de registro do Windows (backend Windows).
    RegSet {
        path: String,
        name: String,
        /// REG_DWORD | REG_SZ
        value_type: String,
        data: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub platform: Platform,
    pub area: String,
    pub tier: Tier,
    pub evidence: Evidence,
    pub privilege: Privilege,
    pub restart: Restart,
    /// 1. Problema que pretende resolver.
    pub problem: String,
    /// 2. Sistemas e versões compatíveis.
    pub compat: String,
    /// 3. Condições em que faz sentido (texto) …
    pub when: String,
    /// … e condições verificáveis contra o diagnóstico ("chave=valor").
    #[serde(default)]
    pub conditions: Vec<String>,
    /// 4. Alteração técnica exata.
    pub change: String,
    /// 5. Benefício esperado, sem percentuais inventados.
    pub benefit: String,
    /// 6. Possíveis efeitos negativos.
    pub side_effects: String,
    /// 8. Forma de desfazer.
    pub undo: String,
    /// 9. Método de validação.
    pub validation: String,
    /// 10. Fontes.
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub params: BTreeMap<String, ParamDef>,
    #[serde(default)]
    pub steps: Vec<StepDef>,
}
