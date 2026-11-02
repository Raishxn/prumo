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

const VERDE: Color32 = Color32::from_rgb(120, 205, 145);
const AMARELO: Color32 = Color32::from_rgb(225, 190, 100);
const VERMELHO: Color32 = Color32::from_rgb(235, 115, 105);
const FUNDO: Color32 = Color32::BLACK;
const PAINEL: Color32 = Color32::from_rgb(8, 8, 8);
const CARTAO: Color32 = Color32::from_rgb(14, 14, 14);
const HOVER: Color32 = Color32::from_rgb(24, 24, 24);
const LINHA: Color32 = Color32::from_rgb(31, 31, 31);
const LINHA_FORTE: Color32 = Color32::from_rgb(58, 58, 58);
const TEXTO: Color32 = Color32::from_rgb(245, 245, 245);
const APAGADO: Color32 = Color32::from_rgb(140, 140, 140);

fn tema(ctx: &egui::Context) {
    use egui::{CornerRadius as R, FontFamily, FontId, Stroke, TextStyle};
    let mut v = egui::Visuals::dark();
    v.panel_fill = FUNDO;
    v.window_fill = CARTAO;
    v.window_stroke = Stroke::new(1.0, LINHA_FORTE);
    v.window_corner_radius = R::same(16);
    v.extreme_bg_color = PAINEL;
    v.faint_bg_color = PAINEL;
    v.override_text_color = Some(TEXTO);
    v.hyperlink_color = TEXTO;
    v.selection.bg_fill = HOVER;
    for (w, f, b) in [
        (&mut v.widgets.noninteractive, CARTAO, LINHA),
        (&mut v.widgets.inactive, CARTAO, LINHA_FORTE),
        (&mut v.widgets.hovered, HOVER, APAGADO),
        (&mut v.widgets.active, HOVER, TEXTO),
        (&mut v.widgets.open, HOVER, LINHA_FORTE),
    ] {
        w.bg_fill = f;
        w.weak_bg_fill = f;
        w.bg_stroke = Stroke::new(1.0, b);
        w.corner_radius = R::same(10);
    }
    ctx.set_visuals(v);
    let mut st = (*ctx.global_style()).clone();
    st.spacing.item_spacing = egui::vec2(10.0, 8.0);
    st.spacing.button_padding = egui::vec2(16.0, 8.0);
    st.text_styles.insert(TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional));
    st.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    st.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
    st.text_styles.insert(TextStyle::Small, FontId::new(12.0, FontFamily::Proportional));
    ctx.set_global_style(st);
}

fn cartao<R>(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARTAO)
        .stroke(egui::Stroke::new(1.0, LINHA))
        .corner_radius(16)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            f(ui)
        })
        .inner
}

/// Rótulo pequeno de seção, em maiúsculas.
fn rotulo_secao(ui: &mut egui::Ui, t: &str) {
    ui.label(RichText::new(t.to_uppercase()).size(11.0).color(APAGADO).strong());
}

fn titulo(ui: &mut egui::Ui, t: &str, sub: &str) {
    ui.label(RichText::new(t).size(26.0).strong());
    ui.label(RichText::new(sub).color(APAGADO));
    ui.add_space(12.0);
}

fn interruptor(ui: &mut egui::Ui, ligado: &mut bool, habilitado: bool) -> egui::Response {
    let tam = egui::vec2(36.0, 20.0);
    let sense = if habilitado { egui::Sense::click() } else { egui::Sense::hover() };
    let (r, mut resp) = ui.allocate_exact_size(tam, sense);
    if resp.clicked() {
        *ligado = !*ligado;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *ligado);
    let cor = |c: Color32| if habilitado { c } else { c.gamma_multiply(0.35) };
    let p = ui.painter();
    p.rect(r, 10, if *ligado { cor(TEXTO) } else { PAINEL }, egui::Stroke::new(1.0, cor(LINHA_FORTE)), egui::StrokeKind::Inside);
    let x = egui::lerp(r.left() + 10.0..=r.right() - 10.0, t);
    p.circle_filled(egui::pos2(x, r.center().y), 7.0, if *ligado { FUNDO } else { cor(APAGADO) });
    resp
}

fn botao_menu(ui: &mut egui::Ui, ativo: bool, t: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let fundo = if ativo { TEXTO } else if resp.hovered() { HOVER } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(r, 12, fundo);
    let cor = if ativo { FUNDO } else if resp.hovered() { TEXTO } else { APAGADO };
    ui.painter().text(r.left_center() + egui::vec2(14.0, 0.0), egui::Align2::LEFT_CENTER, t, egui::FontId::proportional(14.0), cor);
    resp.clicked()
}

/// Botão principal: branco sobre preto.
fn botao_forte(t: String) -> egui::Button<'static> {
    egui::Button::new(RichText::new(t).color(FUNDO).strong()).fill(TEXTO).corner_radius(10)
}

fn medidor(ui: &mut egui::Ui, nome: &str, usado_mb: f64, total_mb: f64) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(nome).color(APAGADO));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.monospace(format!("{:.1} / {:.1} GB", usado_mb / 1024.0, total_mb / 1024.0));
        });
    });
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 2, LINHA);
    let mut b = r;
    b.set_width(r.width() * (usado_mb / total_mb.max(1.0)).clamp(0.0, 1.0) as f32);
    ui.painter().rect_filled(b, 2, TEXTO);
}

/// Fato ausente aparece como "desconhecido", nunca inventado.
fn bloco_info(ui: &mut egui::Ui, nome: &str, valor: Option<&str>) {
    rotulo_secao(ui, nome);
    ui.label(RichText::new(valor.unwrap_or("desconhecido")).size(15.0).color(if valor.is_some() { TEXTO } else { AMARELO }));
}

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
        tema(&ctx);
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
        titulo(ui, "Início", "Estado da máquina, sem estimativas.");
        for s in self.sessoes.iter().filter(|s| !s.interrupted().is_empty()) {
            ui.colored_label(VERMELHO, format!("A sessão {} parou no meio. Veja em Histórico para voltar ao estado anterior.", s.id));
        }
        let Some(d) = &self.diag else {
            ui.label("Lendo o sistema…");
            return;
        };
        let pendentes = self.plano.iter().filter(|a| a.status == PlanStatus::Pending).count();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.columns(3, |c| {
                for (ui, (t, v)) in c.iter_mut().zip([("Prontas para aplicar", pendentes), ("Sessões", self.sessoes.len()), ("Achados", d.findings.len())]) {
                    cartao(ui, |ui| {
                        rotulo_secao(ui, t);
                        ui.label(RichText::new(v.to_string()).size(34.0).strong());
                    });
                }
            });
            ui.add_space(10.0);
            cartao(ui, |ui| {
                rotulo_secao(ui, "Sistema");
                ui.add_space(6.0);
                ui.columns(2, |c| {
                    bloco_info(&mut c[0], "Processador", d.get("cpu.model"));
                    bloco_info(&mut c[1], "Placa de vídeo", d.get("gpu.name"));
                });
                ui.add_space(10.0);
                if let (Some(t), Some(l)) = (d.num("mem.total_mb"), d.num("mem.available_mb")) {
                    medidor(ui, "Memória em uso", t - l, t);
                }
                if let (Some(t), Some(u)) = (d.num("swap.total_mb"), d.num("swap.used_mb")) {
                    medidor(ui, "Swap em uso", u, t);
                }
            });
            ui.add_space(10.0);
            rotulo_secao(ui, "Achados");
            if d.findings.is_empty() {
                ui.label(RichText::new("Nenhum achado.").color(APAGADO));
            }
            for f in &d.findings {
                cartao(ui, |ui| {
                    ui.label(&f.text);
                    ui.small(RichText::new(format!("{} · baseado em: {}", f.severity, f.based_on.join(", "))).color(APAGADO));
                });
                ui.add_space(4.0);
            }
            ui.label(RichText::new(format!("Lido em {}", d.collected_at)).small().color(APAGADO));
        });
    }

    fn diagnostico(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Diagnóstico").size(26.0).strong());
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
        titulo(ui, "Plano", "Ligue o que quer aplicar. Nada muda até você confirmar.");
        let pendentes = self.plano.iter().filter(|a| a.status == PlanStatus::Pending).count();
        ui.horizontal(|ui| {
            let pode = pendentes > 0 && self.ocupado.is_none();
            if ui.add_enabled(pode, botao_forte(format!("Aplicar {pendentes} ação(ões)…"))).clicked() {
                self.confirmar_aplicar = true;
            }
            if cfg!(windows) {
                interruptor(ui, &mut self.ponto_existente, true);
                ui.label("Aceitar ponto de restauração com menos de 24 h");
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
                    ui.add_space(10.0);
                    rotulo_secao(ui, area);
                }
                cartao(ui, |ui| {
                    ui.horizontal(|ui| {
                        let marcavel = matches!(a.status, PlanStatus::Pending | PlanStatus::NotSelected);
                        let mut m = self.marcadas.contains(&a.def.id);
                        if interruptor(ui, &mut m, marcavel).changed() {
                            if m {
                                self.marcadas.insert(a.def.id.clone());
                            } else {
                                self.marcadas.remove(&a.def.id);
                            }
                            trocou = true;
                        }
                        if ui.link(RichText::new(&a.def.title).strong()).clicked() {
                            abrir = Some(a.def.id.clone());
                        }
                    });
                    let (cor, txt) = rotulo(&a.status);
                    ui.colored_label(cor, txt);
                    ui.small(RichText::new(format!("{} · {}", a.def.evidence.label(), a.def.restart.label())).color(APAGADO));
                    if !a.targets.is_empty() {
                        ui.small(RichText::new(format!("alvos: {}", a.targets.join(", "))).color(APAGADO));
                    }
                    if a.status == PlanStatus::Pending {
                        for o in a.ops.iter().filter(|o| !o.satisfied) {
                            ui.small(RichText::new(format!("→ {}  (atual: {})", o.op.describe(), o.current)).monospace().color(APAGADO));
                        }
                    }
                });
                ui.add_space(4.0);
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
        titulo(ui, "Histórico", "Cada sessão guarda o estado anterior real.");
        if self.sessoes.is_empty() {
            ui.label("Nenhuma sessão registrada.");
        }
        let mut pedir = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for s in &self.sessoes {
                cartao(ui, |ui| {
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
        egui::Panel::left("menu")
            .exact_size(240.0)
            .frame(egui::Frame::new().fill(PAINEL).stroke(egui::Stroke::new(1.0, LINHA)).inner_margin(12)).show(ui, |ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(RichText::new("PRUMO").size(16.0).strong());
            });
            ui.add_space(18.0);
            for (p, t) in [(Pagina::Inicio, "Início"), (Pagina::Diagnostico, "Diagnóstico"), (Pagina::Plano, "Plano"), (Pagina::Historico, "Histórico")] {
                if botao_menu(ui, self.pagina == p, t) {
                    self.pagina = p;
                }
            }
            ui.add_space(12.0);
            if let Some(o) = self.ocupado {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(o);
                });
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(FUNDO).inner_margin(28)).show(ui, |ui| {
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
        viewport: egui::ViewportBuilder::default().with_title("Prumo").with_app_id("prumo").with_inner_size([1240.0, 800.0]).with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native("Prumo", opts, Box::new(move |cc| Ok(Box::new(App::novo(cc.egui_ctx.clone(), cat)))))
        .map_err(|e| anyhow::anyhow!("não foi possível abrir a janela: {e}"))
}
