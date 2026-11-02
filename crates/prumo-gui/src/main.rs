//! Interface gráfica do Prumo: casca fina sobre `prumo-core`. Nenhuma regra de
//! otimização mora aqui. Trabalho demorado roda em outra thread e a janela só
//! redesenha com evento, então parada não consome CPU.
#![cfg_attr(windows, windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use prumo_core::diag::{self, Diagnosis};
use prumo_core::engine::{self, PlanStatus, PlannedAction, Selection, UndoReport};
use prumo_core::journal::{list_sessions, ActionRecord, Session};
use prumo_core::model::ActionDef;
use prumo_core::protection::ProtectionOptions;
use prumo_core::{catalog, state_dir};
use std::collections::BTreeSet;
use std::sync::mpsc::{channel, Receiver, Sender};

const VERDE: Color32 = Color32::from_rgb(90, 190, 120);
const AMARELO: Color32 = Color32::from_rgb(225, 180, 70);
const VERMELHO: Color32 = Color32::from_rgb(230, 100, 90);

#[derive(PartialEq, Clone, Copy)]
enum Pagina {
    Inicio,
    Diagnostico,
    Plano,
    Historico,
}

enum Msg {
    Diag(Box<Diagnosis>),
    Aplicado(Result<(String, Vec<ActionRecord>), String>),
    Desfeito(Result<UndoReport, String>),
}

struct App {
    pagina: Pagina,
    cat: Vec<ActionDef>,
    diag: Option<Diagnosis>,
    plano: Vec<PlannedAction>,
    marcadas: BTreeSet<String>,
    detalhe: Option<String>,
    filtro: String,
    ponto_existente: bool,
    confirmar_aplicar: bool,
    confirmar_desfazer: Option<(String, bool)>,
    ocupado: Option<&'static str>,
    aviso: Option<(Color32, String)>,
    relatorio: Vec<String>,
    sessoes: Vec<Session>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

fn rotulo(s: &PlanStatus) -> (Color32, String) {
    match s {
        PlanStatus::Pending => (VERDE, "será aplicada".into()),
        PlanStatus::AlreadyConfigured => (Color32::GRAY, "já configurada".into()),
        PlanStatus::Incompatible(r) => (AMARELO, format!("incompatível: {r}")),
        PlanStatus::Blocked(r) => (AMARELO, format!("bloqueada: {r}")),
        PlanStatus::ManualOnly => (Color32::GRAY, "orientação manual".into()),
        PlanStatus::Rejected => (VERMELHO, "rejeitada".into()),
        PlanStatus::NotSelected => (Color32::GRAY, "não selecionada".into()),
    }
}

impl App {
    fn novo(ctx: egui::Context, cat: Vec<ActionDef>) -> Self {
        let (tx, rx) = channel();
        let mut app = App {
            pagina: Pagina::Inicio,
            cat,
            diag: None,
            plano: vec![],
            marcadas: BTreeSet::new(),
            detalhe: None,
            filtro: String::new(),
            ponto_existente: false,
            confirmar_aplicar: false,
            confirmar_desfazer: None,
            ocupado: None,
            aviso: None,
            relatorio: vec![],
            sessoes: list_sessions(&state_dir()),
            tx,
            rx,
        };
        app.diagnosticar(&ctx);
        app
    }

    fn em_fundo(&mut self, ctx: &egui::Context, nome: &'static str, f: impl FnOnce() -> Msg + Send + 'static) {
        self.ocupado = Some(nome);
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(f());
            ctx.request_repaint();
        });
    }

    fn diagnosticar(&mut self, ctx: &egui::Context) {
        self.em_fundo(ctx, "Lendo o sistema…", || Msg::Diag(Box::new(diag::collect())));
    }

    fn selecao(&self) -> Selection {
        Selection { ids: Some(self.marcadas.iter().cloned().collect()), ..Default::default() }
    }

    /// Prévia a partir do diagnóstico em memória.
    fn replanejar(&mut self) {
        let Some(d) = &self.diag else { return };
        match engine::plan(&self.cat, d, &self.selecao()) {
            Ok(p) => self.plano = p,
            Err(e) => self.aviso = Some((VERMELHO, format!("{e:#}"))),
        }
    }

    fn aplicar(&mut self, ctx: &egui::Context) {
        let (cat, sel) = (self.cat.clone(), self.selecao());
        let opts = ProtectionOptions { accept_recent_restore_point: self.ponto_existente };
        self.em_fundo(ctx, "Aplicando…", move || {
            // Diagnóstico novo: a prévia pode ter envelhecido (ex.: jogo aberto depois).
            let r = engine::plan(&cat, &diag::collect(), &sel)
                .and_then(|p| engine::apply_protected(&p, "app", &opts))
                .map(|(s, recs)| (s.id, recs))
                .map_err(|e| format!("{e:#}"));
            Msg::Aplicado(r)
        });
    }

    fn desfazer(&mut self, ctx: &egui::Context, id: String, forcar: bool) {
        self.em_fundo(ctx, "Desfazendo…", move || {
            let r = list_sessions(&state_dir())
                .into_iter()
                .find(|s| s.id == id)
                .ok_or_else(|| anyhow::anyhow!("sessão não encontrada"))
                .and_then(|s| Session::load(&s.dir))
                .and_then(|mut s| engine::undo(&mut s, None, forcar))
                .map_err(|e| format!("{e:#}"));
            Msg::Desfeito(r)
        });
    }

    fn receber(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            self.ocupado = None;
            match m {
                Msg::Diag(d) => {
                    let primeira = self.diag.is_none();
                    self.diag = Some(*d);
                    if primeira {
                        // Começa pelo plano recomendado.
                        let d = self.diag.as_ref().unwrap();
                        if let Ok(p) = engine::plan(&self.cat, d, &Selection::default()) {
                            self.marcadas = p.iter().filter(|a| a.status == PlanStatus::Pending).map(|a| a.def.id.clone()).collect();
                        }
                    }
                    self.replanejar();
                }
                Msg::Aplicado(r) => {
                    self.relatorio.clear();
                    match r {
                        Ok((id, recs)) => {
                            self.aviso = Some((VERDE, format!("Sessão {id} concluída.")));
                            self.relatorio = recs.iter().map(|r| format!("[{}] {} — {}", r.outcome.label(), r.action_id, r.detail)).collect();
                        }
                        Err(e) => self.aviso = Some((VERMELHO, e)),
                    }
                    self.sessoes = list_sessions(&state_dir());
                    self.diagnosticar(ctx);
                }
                Msg::Desfeito(r) => {
                    self.relatorio.clear();
                    match r {
                        Ok(u) => {
                            let cor = if u.conflicts.is_empty() && u.errors.is_empty() { VERDE } else { AMARELO };
                            self.aviso = Some((cor, format!("{} restaurado(s), {} conflito(s), {} erro(s).", u.restored.len(), u.conflicts.len(), u.errors.len())));
                            self.relatorio.extend(u.restored.iter().map(|x| format!("restaurado: {x}")));
                            self.relatorio.extend(u.conflicts.iter().map(|x| format!("NÃO restaurado: {x}")));
                            self.relatorio.extend(u.errors.iter().map(|x| format!("erro: {x}")));
                        }
                        Err(e) => self.aviso = Some((VERMELHO, e)),
                    }
                    self.sessoes = list_sessions(&state_dir());
                    self.diagnosticar(ctx);
                }
            }
        }
    }

    fn inicio(&mut self, ui: &mut egui::Ui) {
        ui.heading("Início");
        ui.add_space(8.0);
        let interrompidas: Vec<String> = self.sessoes.iter().filter(|s| !s.interrupted().is_empty()).map(|s| s.id.clone()).collect();
        for id in &interrompidas {
            ui.colored_label(VERMELHO, format!("A sessão {id} parou no meio. Veja em Histórico para voltar ao estado anterior."));
        }
        let Some(d) = &self.diag else {
            ui.label("Lendo o sistema…");
            return;
        };
        let pendentes = self.plano.iter().filter(|a| a.status == PlanStatus::Pending).count();
        ui.label(format!("Diagnóstico de {}", d.collected_at));
        ui.label(format!("{pendentes} ação(ões) prontas para aplicar · {} sessão(ões) no histórico", self.sessoes.len()));
        ui.add_space(12.0);
        ui.heading("Achados");
        if d.findings.is_empty() {
            ui.label("Nenhum achado.");
        }
        for f in &d.findings {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(format!("[{}] {}", f.severity, f.text));
                ui.small(format!("baseado em: {}", f.based_on.join(", ")));
            });
        }
    }

    fn diagnostico(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading("Diagnóstico");
            if ui.add_enabled(self.ocupado.is_none(), egui::Button::new("Ler de novo")).clicked() {
                self.diagnosticar(ctx);
            }
            ui.add(egui::TextEdit::singleline(&mut self.filtro).hint_text("filtrar"));
        });
        let Some(d) = &self.diag else { return };
        let filtro = self.filtro.to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("fatos").striped(true).num_columns(2).show(ui, |ui| {
                for (k, f) in d.facts.iter().filter(|(k, _)| k.to_lowercase().contains(&filtro)) {
                    ui.monospace(k);
                    match &f.value {
                        Some(v) => ui.label(v).on_hover_text(&f.source),
                        None => ui.colored_label(AMARELO, format!("desconhecido — {}", f.source)),
                    };
                    ui.end_row();
                }
            });
        });
    }

    fn plano_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Plano");
        ui.label("Marque o que quer aplicar. Nada muda até você confirmar.");
        let pendentes = self.plano.iter().filter(|a| a.status == PlanStatus::Pending).count();
        ui.horizontal(|ui| {
            let pode = pendentes > 0 && self.ocupado.is_none();
            if ui.add_enabled(pode, egui::Button::new(format!("Aplicar {pendentes} ação(ões)…"))).clicked() {
                self.confirmar_aplicar = true;
            }
            if cfg!(windows) {
                ui.checkbox(&mut self.ponto_existente, "Aceitar ponto de restauração com menos de 24 h");
            }
        });
        ui.separator();
        let mut trocou = false;
        let mut abrir = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut area = "";
            for a in &self.plano {
                if a.def.area != area {
                    area = &a.def.area;
                    ui.add_space(6.0);
                    ui.label(RichText::new(area).strong());
                }
                ui.horizontal(|ui| {
                    let marcavel = matches!(a.status, PlanStatus::Pending | PlanStatus::NotSelected);
                    let mut m = self.marcadas.contains(&a.def.id);
                    if ui.add_enabled(marcavel, egui::Checkbox::without_text(&mut m)).changed() {
                        if m {
                            self.marcadas.insert(a.def.id.clone());
                        } else {
                            self.marcadas.remove(&a.def.id);
                        }
                        trocou = true;
                    }
                    if ui.link(&a.def.title).clicked() {
                        abrir = Some(a.def.id.clone());
                    }
                });
                let (cor, txt) = rotulo(&a.status);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(26.0);
                    ui.colored_label(cor, txt);
                    ui.small(format!("· {} · {}", a.def.evidence.label(), a.def.restart.label()));
                });
                if !a.targets.is_empty() {
                    ui.small(format!("alvos: {}", a.targets.join(", ")));
                }
                if a.status == PlanStatus::Pending {
                    for o in a.ops.iter().filter(|o| !o.satisfied) {
                        ui.small(format!("→ {}  (atual: {})", o.op.describe(), o.current));
                    }
                }
            }
        });
        if trocou {
            self.replanejar();
        }
        if abrir.is_some() {
            self.detalhe = abrir;
        }
        if self.confirmar_aplicar {
            egui::Window::new("Confirmar").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(format!("Aplicar {pendentes} ação(ões)? O estado anterior fica no histórico para desfazer."));
                ui.horizontal(|ui| {
                    if ui.button("Aplicar").clicked() {
                        self.confirmar_aplicar = false;
                        self.aplicar(ctx);
                    }
                    if ui.button("Cancelar").clicked() {
                        self.confirmar_aplicar = false;
                    }
                });
            });
        }
    }

    fn historico(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Histórico");
        if self.sessoes.is_empty() {
            ui.label("Nenhuma sessão registrada.");
        }
        let mut pedir = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for s in &self.sessoes {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&s.id).monospace());
                    ui.small(format!("{} · origem {} · {} alteração(ões)", s.created, s.origin, s.entries.len()));
                    if !s.interrupted().is_empty() {
                        ui.colored_label(VERMELHO, format!("Parou no meio de {} alteração(ões).", s.interrupted().len()));
                    }
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.ocupado.is_none() && !s.entries.is_empty(), egui::Button::new("Desfazer")).clicked() {
                            pedir = Some((s.id.clone(), false));
                        }
                        if ui.add_enabled(self.ocupado.is_none() && !s.entries.is_empty(), egui::Button::new("Forçar")).on_hover_text("Sobrescreve também o que foi mudado depois por você ou outro programa").clicked() {
                            pedir = Some((s.id.clone(), true));
                        }
                    });
                });
            }
        });
        if pedir.is_some() {
            self.confirmar_desfazer = pedir;
        }
        if let Some((id, forcar)) = self.confirmar_desfazer.clone() {
            egui::Window::new("Confirmar desfazer").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(if forcar { format!("Forçar o desfazer de {id}? Mudanças feitas depois serão sobrescritas.") } else { format!("Desfazer a sessão {id}? Feche antes os programas afetados.") });
                ui.horizontal(|ui| {
                    if ui.button("Desfazer").clicked() {
                        self.confirmar_desfazer = None;
                        self.desfazer(ctx, id.clone(), forcar);
                    }
                    if ui.button("Cancelar").clicked() {
                        self.confirmar_desfazer = None;
                    }
                });
            });
        }
    }

    fn janela_detalhe(&mut self, ctx: &egui::Context) {
        let Some(x) = self.detalhe.as_ref().and_then(|id| self.cat.iter().find(|a| &a.id == id)) else { return };
        let mut aberta = true;
        egui::Window::new(&x.title).open(&mut aberta).default_size([560.0, 480.0]).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.monospace(format!("{} v{}", x.id, x.version));
                ui.label(format!("Evidência: {}", x.evidence.label()));
                for (t, v) in [
                    ("Problema", &x.problem),
                    ("Compatibilidade", &x.compat),
                    ("Quando faz sentido", &x.when),
                    ("Alteração exata", &x.change),
                    ("Benefício esperado", &x.benefit),
                    ("Efeitos negativos", &x.side_effects),
                    ("Como desfazer", &x.undo),
                    ("Validação", &x.validation),
                ] {
                    ui.add_space(6.0);
                    ui.label(RichText::new(t).strong());
                    ui.label(v);
                }
                if !x.sources.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Fontes").strong());
                }
                for s in &x.sources {
                    ui.hyperlink_to(&s.title, &s.url);
                    ui.small(format!("consultado em {}: {}", s.consulted, s.says));
                }
            });
        });
        if !aberta {
            self.detalhe = None;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.receber(&ctx);
        egui::Panel::left("menu").exact_size(170.0).show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Prumo");
            ui.add_space(8.0);
            for (p, t) in [(Pagina::Inicio, "Início"), (Pagina::Diagnostico, "Diagnóstico"), (Pagina::Plano, "Plano"), (Pagina::Historico, "Histórico")] {
                ui.selectable_value(&mut self.pagina, p, t);
            }
            ui.add_space(12.0);
            if let Some(o) = self.ocupado {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(o);
                });
            }
        });
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some((c, t)) = self.aviso.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(c, t);
                    if ui.small_button("✕").clicked() {
                        self.aviso = None;
                        self.relatorio.clear();
                    }
                });
                for r in &self.relatorio {
                    ui.small(r);
                }
                ui.separator();
            }
            match self.pagina {
                Pagina::Inicio => self.inicio(ui),
                Pagina::Diagnostico => self.diagnostico(ui, &ctx),
                Pagina::Plano => self.plano_ui(ui, &ctx),
                Pagina::Historico => self.historico(ui, &ctx),
            }
        });
        self.janela_detalhe(&ctx);
    }
}

fn main() -> anyhow::Result<()> {
    let all = catalog::load()?;
    let cat = catalog::for_platform(&all, catalog::current_platform());
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Prumo").with_app_id("prumo").with_inner_size([980.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native("Prumo", opts, Box::new(move |cc| Ok(Box::new(App::novo(cc.egui_ctx.clone(), cat)))))
        .map_err(|e| anyhow::anyhow!("não foi possível abrir a janela: {e}"))
}
