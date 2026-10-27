//! Interface gráfica: janela principal, menus e diálogos.

use crate::files;
use crate::format::{self, CoverFormat, FitMode, InternalType};
use crate::i18n::tr;
use crate::layout::{self, DrawOpts, PageSetup, Prepared, Side};
use crate::paint::paint_page;
use crate::pdf;
use crate::print;
use crate::settings::{self, Config, Lang, PrintChoice, PrintSetup, Skin};
use crate::theme::{self, Palette};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Key, KeyboardShortcut, Layout, Modifiers, Rect, RichText, Sense, Stroke, StrokeKind, TextStyle,
    TextureHandle, Ui, pos2, vec2,
};
use image::RgbImage;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Lado maior máximo da imagem usada para imprimir / para a tela.
const FULL_MAX: u32 = 6000;
const SMALL_MAX: u32 = 1000;
const SPLASH_SECS: f64 = 2.6;

const PAPERS: &[(&str, (f32, f32))] = &[
    ("A4", (210.0, 297.0)),
    ("Carta (Letter)", (215.9, 279.4)),
    ("Ofício (Legal)", (215.9, 355.6)),
    ("A5", (148.0, 210.0)),
];

const TIPS: &[&str] = &[
    "Oi! Arraste uma capa pra cá que eu cuido do resto!",
    "Clique com o botão direito numa capa pra girar ou limpar.",
    "Nomeie como \"jogo front.jpg\" e \"jogo back.jpg\" que eu acho o par sozinha!",
    "Ctrl+V cola uma imagem copiada direto aqui.",
    "Tracejado = corte. Tracinho cheio = dobra!",
    "Papel fotográfico deixa as capas lindonas.",
    "\"Imprimir numa folha só\" economiza papel com capas pequenas.",
    "Medidas erradas? Calibre em Arquivo > Configurar impressão.",
    "Dá pra criar formatos novos no Editor de formatos.",
    "Fita gravada em casa? Salve um modelo VHS em branco (botão direito na capa) e desenhe no GIMP!",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Which {
    Left,
    Right,
    Both,
}

impl Which {
    fn sides(self) -> &'static [usize] {
        match self {
            Which::Left => &[0],
            Which::Right => &[1],
            Which::Both => &[0, 1],
        }
    }
}

fn side_of(i: usize) -> Side {
    if i == 0 { Side::Left } else { Side::Right }
}

enum Action {
    New,
    Open(usize),
    Paste(Option<usize>),
    Rotate(usize, u8),
    Template(usize),
    Clear(usize),
    Print(Which),
    Export(Which),
    Preview(Which),
    PrintSetup,
    Options,
    FormatEditor,
    Batch,
    About,
    SetSkin(Skin),
    SetLang(Lang),
    Quit,
}

type Cache = Option<(u64, Prepared, TextureHandle)>;

#[derive(Default)]
struct Slot {
    path: Option<PathBuf>,
    full: Option<Arc<RgbImage>>,
    small: Option<RgbImage>,
    generation: u64,
    rotation: u8,
    format: usize,
    /// A imagem veio da busca automática da capa par (pode ser substituída por outra busca).
    matched: bool,
    cache: Cache,
}

struct FormatEditor {
    list: Vec<CoverFormat>,
    sel: usize,
    markers: String,
    markers_err: Option<String>,
    printer_opts: print::PrinterOptions,
    thumbs: [Cache; 2],
}

struct PrintSetupDlg {
    setup: PrintSetup,
    printers: Vec<String>,
    default: Option<String>,
}

struct PreviewDlg {
    which: Which,
    page: usize,
}

struct BatchDlg {
    dir: Option<PathBuf>,
    pairs: Vec<(Option<PathBuf>, Option<PathBuf>, bool)>,
}

/// Janela "Imprimir": o trabalho já montado esperando a escolha de impressora e papel.
struct PrintDlg {
    groups: Vec<Vec<CoverJob>>,
    detect: bool,
    calibration: bool,
    title: String,
    printer: String,
    printers: Vec<String>,
    opts: print::PrinterOptions,
    choice: PrintChoice,
    copies: u32,
    pages: usize,
}

#[derive(Clone)]
enum ImgSrc {
    Mem(Arc<RgbImage>),
    File(PathBuf),
}

#[derive(Clone)]
struct CoverJob {
    side: Side,
    fmt: CoverFormat,
    img: ImgSrc,
    rotation: u8,
}

enum JobKind {
    Print { printer: String, title: String, copies: u32, choice: PrintChoice },
    Export(PathBuf),
}

struct JobSpec {
    groups: Vec<Vec<CoverJob>>,
    detect: Option<Vec<CoverFormat>>,
    auto_orient: bool,
    single_page: bool,
    setup: PageSetup,
    cut_lines: bool,
    border: bool,
    offset: (f32, f32),
    kind: JobKind,
}

pub struct App {
    cfg: Config,
    formats: Vec<CoverFormat>,
    slots: [Slot; 2],
    slot_rects: [Rect; 2],
    skin: Skin,
    applied: Option<(Skin, bool)>,
    pal: Palette,
    start: Option<f64>,
    status: (String, bool),
    job: Option<mpsc::Receiver<Result<String, String>>>,
    tip: usize,
    tip_time: f64,
    happy_until: f64,
    options_open: bool,
    about_open: bool,
    editor: Option<FormatEditor>,
    print_dlg: Option<PrintSetupDlg>,
    printing: Option<PrintDlg>,
    preview: Option<PreviewDlg>,
    batch: Option<BatchDlg>,
    title_sent: String,
    default_printer: Option<String>,
    saved_sig: u64,
}

fn hash_of(v: impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn format_key(f: &CoverFormat, generation: u64, rotation: u8, auto: bool) -> u64 {
    hash_of((format!("{f:?}"), generation, rotation, auto))
}

/// Prepara (com cache) a imagem de tela de um formato e devolve a textura.
fn cached<'c>(ctx: &egui::Context, cache: &'c mut Cache, key: u64, img: &RgbImage, fmt: &CoverFormat, rot: u8, auto: bool) -> &'c (u64, Prepared, TextureHandle) {
    if cache.as_ref().is_none_or(|c| c.0 != key) {
        let p = layout::prepare(img, fmt, rot, auto);
        let ci = egui::ColorImage::from_rgb([p.img.width() as usize, p.img.height() as usize], p.img.as_raw());
        let tex = ctx.load_texture(format!("cover-{key}"), ci, egui::TextureOptions::LINEAR);
        *cache = Some((key, p, tex));
    }
    cache.as_ref().unwrap()
}

fn random_skin() -> Skin {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    Skin::ALL[(n as usize) % (Skin::ALL.len() - 1)]
}

fn len_edit(ui: &mut Ui, v: &mut f32, inches: bool, enabled: bool) -> bool {
    let mut x = if inches { *v / 25.4 } else { *v };
    let r = ui.add_enabled(
        enabled,
        egui::DragValue::new(&mut x)
            .speed(if inches { 0.01 } else { 0.1 })
            .range(-1000.0..=1000.0)
            .fixed_decimals(if inches { 2 } else { 1 })
            .suffix(if inches { " pol" } else { " mm" }),
    );
    if r.changed() {
        *v = if inches { x * 25.4 } else { x };
    }
    r.changed()
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl App {
    pub fn new(cc: &eframe::CreationContext, files_arg: Vec<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let mut cfg = settings::load();
        let mut formats = format::load_formats();
        // Formatos padrão novos (ex.: VHS) entram na lista que o usuário já salvou.
        if cfg.formats_version < format::DEFAULTS_VERSION {
            let saved = format::formats_path().is_some_and(|p| p.exists());
            for d in format::default_formats() {
                if !formats.iter().any(|f| f.name == d.name) {
                    formats.push(d);
                }
            }
            if saved {
                let _ = format::save_formats(&formats);
            }
            cfg.formats_version = format::DEFAULTS_VERSION;
        }
        let skin = if cfg.skin == Skin::Random { random_skin() } else { cfg.skin };
        let find = |name: &str, fallback: usize| formats.iter().position(|f| f.name == name).unwrap_or(fallback.min(formats.len() - 1));
        let mut app = Self {
            slots: [
                Slot { format: find(&cfg.left.format, 0), ..Default::default() },
                Slot { format: find(&cfg.right.format, 1), ..Default::default() },
            ],
            pal: theme::palette(skin),
            skin,
            applied: None,
            slot_rects: [Rect::NOTHING; 2],
            start: cfg.options.splash.then_some(0.0),
            status: (String::new(), false),
            job: None,
            tip: 0,
            tip_time: 0.0,
            happy_until: 0.0,
            options_open: false,
            about_open: false,
            editor: None,
            print_dlg: None,
            printing: None,
            preview: None,
            batch: None,
            title_sent: String::new(),
            default_printer: print::printers().1,
            saved_sig: 0,
            formats,
            cfg,
        };
        app.status = (app.t("Pronto. Que capas vamos imprimir hoje?").into(), false);
        if !files_arg.is_empty() {
            for (i, p) in files_arg.iter().take(2).enumerate() {
                app.load_into(i, p, files_arg.len() == 1);
            }
        } else if app.cfg.options.reload_previous {
            for i in 0..2 {
                let st = if i == 0 { app.cfg.left.clone() } else { app.cfg.right.clone() };
                if let Some(p) = st.path.filter(|p| p.exists()) {
                    if let Ok(img) = files::load_image(&p) {
                        let fmt = app.slots[i].format;
                        app.set_image(i, img, Some(p), false);
                        app.slots[i].format = fmt;
                        app.slots[i].rotation = st.rotation % 4;
                    }
                }
            }
        }
        app
    }

    fn t(&self, s: &'static str) -> &'static str {
        tr(self.cfg.lang, s)
    }

    fn ok(&mut self, msg: impl Into<String>) {
        self.status = (msg.into(), false);
    }

    fn err(&mut self, msg: impl Into<String>) {
        self.status = (msg.into(), true);
    }

    fn persist(&mut self) {
        for i in 0..2 {
            let s = &self.slots[i];
            let st = settings::SlotState {
                path: s.path.clone(),
                rotation: s.rotation,
                format: self.formats.get(s.format).map(|f| f.name.clone()).unwrap_or_default(),
            };
            if i == 0 {
                self.cfg.left = st;
            } else {
                self.cfg.right = st;
            }
        }
        settings::save(&self.cfg);
    }

    fn keywords(&self, i: usize) -> (Vec<String>, Vec<String>) {
        let l = files::keywords(&self.cfg.options.left_keywords);
        let r = files::keywords(&self.cfg.options.right_keywords);
        if i == 0 { (l, r) } else { (r, l) }
    }

    fn set_image(&mut self, i: usize, img: RgbImage, path: Option<PathBuf>, matched: bool) {
        let full = layout::limit_size(img, FULL_MAX);
        let small = layout::limit_size(full.clone(), SMALL_MAX);
        if self.cfg.options.auto_detect {
            if let Some(f) = layout::detect_format(full.width(), full.height(), &self.formats, self.cfg.options.auto_orient) {
                self.slots[i].format = f;
            }
        }
        let s = &mut self.slots[i];
        s.generation += 1;
        s.path = path;
        s.full = Some(Arc::new(full));
        s.small = Some(small);
        s.rotation = 0;
        s.matched = matched;
        s.cache = None;
    }

    fn load_into(&mut self, i: usize, path: &Path, try_match: bool) {
        match files::load_image(path) {
            Ok(img) => {
                self.set_image(i, img, Some(path.to_path_buf()), false);
                self.ok(format!("{} {}", self.t("Carregado:"), file_name(path)));
                if try_match && self.cfg.options.match_cover {
                    let o = 1 - i;
                    if self.slots[o].full.is_none() || self.slots[o].matched {
                        let (from, to) = self.keywords(i);
                        if let Some(m) = files::find_match(path, &from, &to) {
                            if let Ok(img) = files::load_image(&m) {
                                self.set_image(o, img, Some(m.clone()), true);
                                self.ok(format!("{} {}  +  {}", self.t("Carregado:"), file_name(path), file_name(&m)));
                            }
                        }
                    }
                }
            }
            Err(e) => self.err(format!("{e:#}")),
        }
    }

    fn pick_image(&self, i: usize) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new()
            .set_title(if i == 0 { self.t("Abrir imagem da esquerda") } else { self.t("Abrir imagem da direita") })
            .add_filter(self.t("Imagens"), files::EXTENSIONS);
        if let Some(dir) = self.slots.iter().find_map(|s| s.path.as_ref().and_then(|p| p.parent())) {
            d = d.set_directory(dir);
        }
        d.pick_file()
    }

    fn paste(&mut self, target: Option<usize>) {
        let i = target.unwrap_or_else(|| if self.slots[0].full.is_none() || self.slots[1].full.is_some() { 0 } else { 1 });
        let Ok(mut cb) = arboard::Clipboard::new() else {
            return self.err(self.t("Não consegui acessar a área de transferência."));
        };
        if let Ok(img) = cb.get_image() {
            let mut rgb = RgbImage::new(img.width as u32, img.height as u32);
            for (px, src) in rgb.pixels_mut().zip(img.bytes.chunks_exact(4)) {
                let a = src[3] as u32;
                let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                *px = image::Rgb([mix(src[0]), mix(src[1]), mix(src[2])]);
            }
            let dir = settings::cache_dir();
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(if i == 0 { "colado-esquerda.png" } else { "colado-direita.png" });
            let saved = rgb.save(&path).is_ok();
            self.set_image(i, rgb, saved.then_some(path), false);
            return self.ok(self.t("Imagem colada."));
        }
        // Arquivos copiados no gerenciador de arquivos chegam como texto (file://...).
        if let Ok(text) = cb.get_text() {
            let paths: Vec<PathBuf> = text
                .lines()
                .map(|l| l.trim().trim_start_matches("file://"))
                .map(|l| PathBuf::from(percent_decode(l)))
                .filter(|p| files::is_image(p) && p.exists())
                .collect();
            match paths.as_slice() {
                [] => {}
                [one] => return self.load_into(i, &one.clone(), true),
                [a, b, ..] => {
                    let (a, b) = (a.clone(), b.clone());
                    self.load_into(0, &a, false);
                    return self.load_into(1, &b, false);
                }
            }
        }
        self.err(self.t("Não há imagem na área de transferência."));
    }

    fn handle_drop(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if dropped.is_empty() {
            return;
        }
        let imgs: Vec<PathBuf> = dropped.into_iter().filter(|p| files::is_image(p)).collect();
        if imgs.is_empty() {
            return self.err(self.t("Isso não parece uma imagem."));
        }
        if imgs.len() >= 2 {
            let (_, right_kw) = self.keywords(0);
            let lower = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            let (a, b) = if right_kw.iter().any(|k| lower(&imgs[0]).contains(k.as_str())) { (&imgs[1], &imgs[0]) } else { (&imgs[0], &imgs[1]) };
            let (a, b) = (a.clone(), b.clone());
            self.load_into(0, &a, false);
            self.load_into(1, &b, false);
            return;
        }
        let pos = ctx.input(|i| i.pointer.latest_pos());
        let i = match pos {
            Some(p) if self.slot_rects[1].contains(p) => 1,
            Some(p) if self.slot_rects[0].contains(p) => 0,
            _ => {
                let (_, right_kw) = self.keywords(0);
                let stem = imgs[0].file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
                if right_kw.iter().any(|k| stem.contains(k.as_str())) || (self.slots[0].full.is_some() && self.slots[1].full.is_none()) { 1 } else { 0 }
            }
        };
        self.load_into(i, &imgs[0].clone(), true);
    }

    fn setup(&self) -> PageSetup {
        PageSetup { paper: self.cfg.print.paper, margin: self.cfg.print.margin, correct_area: self.cfg.options.correct_area, tile: self.cfg.print.tile }
    }

    fn draw_opts(&self, margin: f32) -> DrawOpts {
        DrawOpts { cut_lines: self.cfg.cut_lines, border: self.cfg.options.border, margin }
    }

    fn printer(&self) -> Option<String> {
        self.cfg.print.printer.clone().or_else(|| self.default_printer.clone())
    }

    fn job_title(&self, which: Which) -> String {
        if self.cfg.options.dummy_job {
            return self.cfg.options.dummy_name.clone();
        }
        let names: Vec<String> = which.sides().iter().filter_map(|&i| self.slots[i].path.as_deref().map(file_name)).collect();
        format!("UnderCoverNc - {}", names.join(" + "))
    }

    fn cover_jobs(&mut self, which: Which) -> Option<Vec<CoverJob>> {
        let jobs: Vec<CoverJob> = which
            .sides()
            .iter()
            .filter_map(|&i| {
                let s = &self.slots[i];
                Some(CoverJob { side: side_of(i), fmt: self.formats[s.format].clone(), img: ImgSrc::Mem(s.full.clone()?), rotation: s.rotation })
            })
            .collect();
        if jobs.is_empty() {
            self.err(match which {
                Which::Left => self.t("Não há imagem na esquerda."),
                Which::Right => self.t("Não há imagem na direita."),
                Which::Both => self.t("Abra pelo menos uma capa primeiro."),
            });
            return None;
        }
        Some(jobs)
    }

    fn spec(&self, groups: Vec<Vec<CoverJob>>, kind: JobKind, detect: bool) -> JobSpec {
        JobSpec {
            groups,
            detect: (detect && self.cfg.options.auto_detect).then(|| self.formats.clone()),
            auto_orient: self.cfg.options.auto_orient,
            single_page: self.cfg.single_page,
            setup: self.setup(),
            cut_lines: self.cfg.cut_lines,
            border: self.cfg.options.border,
            offset: self.cfg.print.offset,
            kind,
        }
    }

    fn start_job(&mut self, spec: JobSpec) {
        if self.job.is_some() {
            return self.err(self.t("Espere o trabalho atual terminar."));
        }
        let (tx, rx) = mpsc::channel();
        let lang = self.cfg.lang;
        let printing = matches!(spec.kind, JobKind::Print { .. });
        std::thread::spawn(move || {
            let _ = tx.send(run_job(spec, lang).map_err(|e| format!("{e:#}")));
        });
        self.job = Some(rx);
        self.ok(if printing { self.t("Mandando para a impressora...") } else { self.t("Gerando PDF...") });
    }

    fn do_print(&mut self, which: Which) {
        let Some(jobs) = self.cover_jobs(which) else { return };
        let title = self.job_title(which);
        self.open_print_dialog(vec![jobs], false, false, title);
    }

    /// Abre a janela "Imprimir" (impressora, tipo de papel/qualidade, cor, cópias).
    fn open_print_dialog(&mut self, groups: Vec<Vec<CoverJob>>, detect: bool, calibration: bool, title: String) {
        let (printers, default) = print::printers();
        self.default_printer = default;
        let Some(printer) = self.printer().or_else(|| printers.first().cloned()) else {
            return self.err(self.t("Nenhuma impressora encontrada. Veja Arquivo > Configurar impressão."));
        };
        let opts = print::printer_options(&printer);
        let fmt_media = groups.iter().flatten().map(|c| c.fmt.media_type.clone()).find(|m| !m.is_empty());
        let mut choice = self.cfg.print_choices.get(&printer).cloned().unwrap_or_default();
        if let Some(m) = fmt_media {
            choice.media = m;
        }
        if choice.media.is_empty() || opts.media_label(&choice.media).is_none() {
            choice.media = opts.media_default.clone();
        }
        // Quantas folhas vão sair (só para mostrar).
        let setup = self.setup();
        let pages = groups
            .iter()
            .map(|g| {
                let covers: Vec<_> = g.iter().map(|c| (c.side, &c.fmt, None)).collect();
                layout::plan(&covers, self.cfg.single_page && !calibration, &setup).0.len()
            })
            .sum();
        self.printing = Some(PrintDlg { groups, detect, calibration, title, printer, printers, opts, choice, copies: self.cfg.print.copies, pages });
    }

    fn do_export(&mut self, which: Which) {
        let Some(jobs) = self.cover_jobs(which) else { return };
        let name = self.slots.iter().find_map(|s| s.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned())).unwrap_or("capas".into());
        let Some(path) = rfd::FileDialog::new().set_title(self.t("Exportar PDF")).add_filter("PDF", &["pdf"]).set_file_name(format!("{name}.pdf")).save_file() else {
            return;
        };
        let spec = self.spec(vec![jobs], JobKind::Export(path), false);
        self.start_job(spec);
    }

    /// Salva um PNG em branco no tamanho exato do formato (300 dpi), com guias de dobra
    /// e margem de segurança, para desenhar a capa num editor de imagens.
    fn export_template(&mut self, i: usize) {
        let fmt = self.formats[self.slots[i].format].clone();
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.t("Salvar modelo em branco (PNG)"))
            .add_filter("PNG", &["png"])
            .set_file_name(format!("modelo {}.png", fmt.name))
            .save_file()
        else {
            return;
        };
        let img = crate::template::render(&fmt, 300.0);
        match img.save(&path) {
            Ok(()) => self.ok(format!("{} {} ({}×{} px, 300 dpi)", self.t("Modelo salvo:"), file_name(&path), img.width(), img.height())),
            Err(e) => self.err(format!("{} {e}", self.t("Erro:"))),
        }
    }

    fn calibration_page(&mut self) {
        let mut fmt = CoverFormat { name: "Calibração".into(), markers: vec![], ..self.formats[0].clone() };
        fmt.kind = InternalType::Custom;
        fmt.label = false;
        fmt.width = 100.0;
        fmt.height = 100.0;
        fmt.paper = None;
        fmt.left_pos = Some((20.0, 20.0));
        fmt.fit = FitMode::Stretch;
        fmt.markers = format::parse_markers("RECT = (0, 0)(100, 100), SOLID\nLINE = (50, 0)(50, 100), DASHED\nLINE = (0, 50)(100, 50), DASHED").unwrap_or_default();
        let white = Arc::new(RgbImage::from_pixel(2, 2, image::Rgb([255, 255, 255])));
        let job = CoverJob { side: Side::Left, fmt, img: ImgSrc::Mem(white), rotation: 0 };
        self.open_print_dialog(vec![vec![job]], false, true, "UnderCoverNc - calibração".into());
    }

    fn print_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.printing.take() else { return };
        let p = self.pal;
        let lang = self.cfg.lang;
        let t = |s| tr(lang, s);
        let mut open = true;
        let (mut go, mut cancel) = (false, false);
        let mut switch_printer = None;
        let pt = lang == Lang::Pt;
        let media_name = |label: &str| if pt { print::media_pt(label) } else { label.to_string() };
        dialog(ctx, &p, "print", t("Imprimir"), &mut open, 560.0, |ui| {
            egui::Grid::new("prn").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                ui.label(t("Impressora:"));
                egui::ComboBox::from_id_salt("prn-printer").width(360.0).selected_text(&dlg.printer).show_ui(ui, |ui| {
                    for pr in &dlg.printers {
                        if ui.selectable_label(*pr == dlg.printer, pr).clicked() && *pr != dlg.printer {
                            switch_printer = Some(pr.clone());
                        }
                    }
                });
                ui.end_row();
                ui.label(t("Papel e qualidade:"));
                if dlg.opts.media_types.is_empty() {
                    ui.label(t("(o driver não informa tipos de papel)"));
                } else {
                    let cur = dlg.opts.media_label(&dlg.choice.media).map(media_name).unwrap_or_else(|| dlg.choice.media.clone());
                    egui::ComboBox::from_id_salt("prn-media").width(360.0).height(420.0).selected_text(cur).show_ui(ui, |ui| {
                        for (code, label) in &dlg.opts.media_types {
                            ui.selectable_value(&mut dlg.choice.media, code.clone(), media_name(label));
                        }
                    });
                }
                ui.end_row();
                if dlg.opts.color.is_some() {
                    ui.label(t("Cor:"));
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut dlg.choice.grayscale, false, t("Colorido"));
                        ui.radio_value(&mut dlg.choice.grayscale, true, t("Tons de cinza"));
                    });
                    ui.end_row();
                }
                ui.label(t("Cópias:"));
                ui.add(egui::DragValue::new(&mut dlg.copies).range(1..=99));
                ui.end_row();
            });
            ui.add_space(4.0);
            let paper = PAPERS.iter().find(|(_, z)| *z == self.cfg.print.paper).map(|(n, _)| tr(lang, n).to_string()).unwrap_or_else(|| {
                format!("{:.0}×{:.0} mm", self.cfg.print.paper.0, self.cfg.print.paper.1)
            });
            ui.label(RichText::new(format!("{} {} · {} {}", dlg.pages, t("página(s)"), t("Papel:"), paper)).size(17.0));
            ui.label(RichText::new(t("Dica: para capas, papel fotográfico brilhante (glossy) ou semibrilho em qualidade alta fica lindo. Coloque o papel com o lado brilhante para cima.")).size(16.0));
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, &p, t("Cancelar"), true, 100.0).clicked() {
                    cancel = true;
                }
                if theme::button(ui, &p, t("Imprimir"), self.job.is_none(), 110.0).clicked() {
                    go = true;
                }
            });
        });
        if let Some(pr) = switch_printer {
            dlg.opts = print::printer_options(&pr);
            dlg.choice = self.cfg.print_choices.get(&pr).cloned().unwrap_or_default();
            if dlg.opts.media_label(&dlg.choice.media).is_none() {
                dlg.choice.media = dlg.opts.media_default.clone();
            }
            dlg.printer = pr;
        }
        if go {
            self.cfg.print_choices.insert(dlg.printer.clone(), dlg.choice.clone());
            let kind = JobKind::Print { printer: dlg.printer.clone(), title: dlg.title.clone(), copies: dlg.copies, choice: dlg.choice.clone() };
            let mut spec = self.spec(std::mem::take(&mut dlg.groups), kind, dlg.detect);
            if dlg.calibration {
                spec.cut_lines = true;
                spec.single_page = false;
                spec.setup.correct_area = false;
            }
            self.start_job(spec);
            return;
        }
        if open && !cancel {
            self.printing = Some(dlg);
        }
    }

    fn poll_job(&mut self) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(Ok(msg)) => {
                self.job = None;
                self.happy_until = f64::MAX;
                self.ok(msg);
            }
            Ok(Err(e)) => {
                self.job = None;
                self.err(format!("{} {e}", self.t("Erro:")));
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.job = None,
        }
    }

    fn act(&mut self, ctx: &egui::Context, a: Action) {
        match a {
            Action::New => {
                for s in &mut self.slots {
                    let f = s.format;
                    *s = Slot { format: f, ..Default::default() };
                }
                self.ok(self.t("Tudo limpo!"));
            }
            Action::Open(i) => {
                if let Some(p) = self.pick_image(i) {
                    self.load_into(i, &p, true);
                }
            }
            Action::Paste(i) => self.paste(i),
            Action::Rotate(i, q) => {
                if self.slots[i].full.is_some() {
                    self.slots[i].rotation = (self.slots[i].rotation + q) % 4;
                }
            }
            Action::Template(i) => self.export_template(i),
            Action::Clear(i) => {
                let f = self.slots[i].format;
                self.slots[i] = Slot { format: f, ..Default::default() };
            }
            Action::Print(w) => self.do_print(w),
            Action::Export(w) => self.do_export(w),
            Action::Preview(w) => self.preview = Some(PreviewDlg { which: w, page: 0 }),
            Action::PrintSetup => {
                let (printers, default) = print::printers();
                self.default_printer = default.clone();
                self.print_dlg = Some(PrintSetupDlg { setup: self.cfg.print.clone(), printers, default });
            }
            Action::Options => self.options_open = true,
            Action::FormatEditor => {
                let sel = self.slots[0].format;
                let printer_opts = print::printer_options(&self.printer().unwrap_or_default());
                self.editor = Some(FormatEditor {
                    markers: self.formats[sel].markers_text(),
                    list: self.formats.clone(),
                    sel,
                    markers_err: None,
                    printer_opts,
                    thumbs: [None, None],
                });
            }
            Action::Batch => self.batch = Some(BatchDlg { dir: None, pairs: vec![] }),
            Action::About => self.about_open = true,
            Action::SetSkin(s) => {
                self.cfg.skin = s;
                self.skin = if s == Skin::Random { random_skin() } else { s };
            }
            Action::SetLang(l) => self.cfg.lang = l,
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
        self.persist();
    }

    fn shortcuts(&self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let sc = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        let shift = Modifiers::COMMAND | Modifiers::SHIFT;
        if sc(Modifiers::COMMAND, Key::N) {
            actions.push(Action::New);
        }
        if sc(shift, Key::O) {
            actions.push(Action::Open(1));
        }
        if sc(Modifiers::COMMAND, Key::O) {
            actions.push(Action::Open(0));
        }
        if sc(shift, Key::P) {
            actions.push(Action::Print(Which::Both));
        }
        if sc(Modifiers::COMMAND, Key::P) {
            actions.push(Action::Print(Which::Left));
        }
        if sc(Modifiers::COMMAND, Key::Q) {
            actions.push(Action::Quit);
        }
        // O egui transforma Ctrl+V em evento de colar texto; trata os dois casos.
        let pasted = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))) || sc(Modifiers::COMMAND, Key::V);
        if pasted {
            let hover = ctx.input(|i| i.pointer.hover_pos());
            let target = hover.and_then(|p| (0..2).find(|&i| self.slot_rects[i].contains(p)));
            actions.push(Action::Paste(target));
        }
    }

    // ---------- janela principal ----------

    fn menu(&self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let has = |i: usize| self.slots[i].full.is_some();
        let any = has(0) || has(1);
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(self.t("Arquivo"), |ui| {
                item(ui, actions, self.t("Novo"), "Ctrl+N", true, Action::New);
                item(ui, actions, self.t("Abrir imagem da esquerda..."), "Ctrl+O", true, Action::Open(0));
                item(ui, actions, self.t("Abrir imagem da direita..."), "Ctrl+Shift+O", true, Action::Open(1));
                item(ui, actions, self.t("Colar"), "Ctrl+V", true, Action::Paste(None));
                ui.menu_button(self.t("Girar"), |ui| {
                    for (i, side) in [(0, self.t("Esquerda")), (1, self.t("Direita"))] {
                        for (q, txt) in [(1, "90° ↻"), (3, "90° ↺"), (2, "180°")] {
                            if ui.add_enabled(has(i), egui::Button::new(format!("{side} {txt}"))).clicked() {
                                actions.push(Action::Rotate(i, q));
                                ui.close();
                            }
                        }
                        if i == 0 {
                            ui.separator();
                        }
                    }
                });
                ui.separator();
                item(ui, actions, self.t("Imprimir esquerda..."), "Ctrl+P", has(0), Action::Print(Which::Left));
                item(ui, actions, self.t("Imprimir direita..."), "", has(1), Action::Print(Which::Right));
                item(ui, actions, self.t("Imprimir ambas..."), "Ctrl+Shift+P", any, Action::Print(Which::Both));
                item(ui, actions, self.t("Pré-visualizar impressão..."), "", any, Action::Preview(Which::Both));
                item(ui, actions, self.t("Exportar PDF..."), "", any, Action::Export(Which::Both));
                ui.menu_button(self.t("Salvar modelo em branco (PNG)"), |ui| {
                    for i in 0..2 {
                        let name = &self.formats[self.slots[i].format].name;
                        if ui.button(name).clicked() {
                            actions.push(Action::Template(i));
                            ui.close();
                        }
                    }
                });
                item(ui, actions, self.t("Configurar impressão..."), "", true, Action::PrintSetup);
                if self.cfg.options.batch_mode {
                    item(ui, actions, self.t("Imprimir em lote..."), "", true, Action::Batch);
                }
                ui.separator();
                item(ui, actions, self.t("Sair"), "Ctrl+Q", true, Action::Quit);
            });
            ui.menu_button(self.t("Opções"), |ui| {
                if ui.button(self.t("Opções...")).clicked() {
                    actions.push(Action::Options);
                    ui.close();
                }
                ui.menu_button(self.t("Skins"), |ui| {
                    for s in Skin::ALL {
                        let name = skin_name(self.cfg.lang, s);
                        if ui.radio(self.cfg.skin == s, name).clicked() {
                            actions.push(Action::SetSkin(s));
                            ui.close();
                        }
                    }
                });
                ui.menu_button(self.t("Idioma"), |ui| {
                    for (l, name) in [(Lang::Pt, "Português (Brasil)"), (Lang::En, "English")] {
                        if ui.radio(self.cfg.lang == l, name).clicked() {
                            actions.push(Action::SetLang(l));
                            ui.close();
                        }
                    }
                });
                if ui.button(self.t("Editor de formatos...")).clicked() {
                    actions.push(Action::FormatEditor);
                    ui.close();
                }
            });
            ui.menu_button(self.t("Ajuda"), |ui| {
                if ui.button(self.t("Sobre o UnderCoverNc...")).clicked() {
                    actions.push(Action::About);
                    ui.close();
                }
            });
        });
    }

    fn slot_ui(&mut self, ui: &mut Ui, i: usize, actions: &mut Vec<Action>) {
        let p = self.pal;
        let w = ui.available_width();
        let mut sel = self.slots[i].format;
        egui::ComboBox::from_id_salt(("fmt", i)).width(w).height(560.0).selected_text(&self.formats[sel].name).show_ui(ui, |ui| {
            for (k, f) in self.formats.iter().enumerate() {
                ui.selectable_value(&mut sel, k, &f.name);
            }
        });
        if sel != self.slots[i].format {
            self.slots[i].format = sel;
            self.persist();
        }
        let h = ui.available_height() - 44.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h.max(120.0)), Sense::click());
        self.slot_rects[i] = rect;
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, p.desktop);
        // Padrãozinho de pontos no fundo (cara de área de trabalho antiga).
        let dot = p.desktop.lerp_to_gamma(p.light, 0.12);
        let mut y = rect.top() + 6.0;
        while y < rect.bottom() {
            let mut x = rect.left() + 6.0 + if ((y - rect.top()) as i32 / 12) % 2 == 0 { 0.0 } else { 6.0 };
            while x < rect.right() {
                painter.rect_filled(Rect::from_min_size(pos2(x, y), vec2(2.0, 2.0)), 0.0, dot);
                x += 12.0;
            }
            y += 12.0;
        }
        let fmt = self.formats[self.slots[i].format].clone();
        let auto = self.cfg.options.auto_orient;
        let opts = DrawOpts { cut_lines: self.cfg.cut_lines, border: self.cfg.options.border, margin: 2.0 };
        let empty_text = format!("{}\n\n{}\n{:.1} × {:.1} mm", self.t("Clique ou arraste\numa capa aqui"), fmt.name, fmt.width, fmt.height);
        let area = rect.shrink(18.0);
        let pad = 10.0;
        let page_size = (fmt.width + 2.0 * pad, fmt.height + 2.0 * pad);
        let ctx = ui.ctx().clone();
        let slot = &mut self.slots[i];
        let prepared = match &slot.small {
            Some(img) => {
                let key = format_key(&fmt, slot.generation, slot.rotation, auto);
                Some(cached(&ctx, &mut slot.cache, key, img, &fmt, slot.rotation, auto))
            }
            None => None,
        };
        let page = layout::Page {
            size: page_size,
            items: vec![layout::Placed { side: side_of(i), fmt: std::borrow::Cow::Borrowed(&fmt), prepared: prepared.map(|c| std::borrow::Cow::Borrowed(&c.1)), x: pad, y: pad, tile: None }],
        };
        let tex = prepared.map(|c| c.2.id());
        let sheet = paint_page(&painter, area, &page, &opts, |_| tex, true);
        if prepared.is_none() {
            painter.text(sheet.center(), Align2::CENTER_CENTER, empty_text, FontId::proportional(19.0), Color32::from_rgb(60, 70, 110));
        }
        bevel_sunken(&painter, rect, &p);
        let hovering_files = ui.ctx().input(|inp| !inp.raw.hovered_files.is_empty());
        if hovering_files && (resp.hovered() || ui.ctx().input(|inp| inp.pointer.latest_pos()).is_some_and(|q| rect.contains(q))) {
            painter.rect_stroke(rect.shrink(3.0), 0.0, Stroke::new(3.0_f32, p.highlight), StrokeKind::Inside);
        }
        let has = self.slots[i].full.is_some();
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            actions.push(Action::Open(i));
        }
        resp.context_menu(|ui| {
            if ui.button(self.t("Abrir imagem...")).clicked() {
                actions.push(Action::Open(i));
                ui.close();
            }
            if ui.button(self.t("Colar")).clicked() {
                actions.push(Action::Paste(Some(i)));
                ui.close();
            }
            ui.separator();
            for (q, txt) in [(1, self.t("Girar 90° ↻")), (3, self.t("Girar 90° ↺")), (2, self.t("Girar 180°"))] {
                if ui.add_enabled(has, egui::Button::new(txt)).clicked() {
                    actions.push(Action::Rotate(i, q));
                    ui.close();
                }
            }
            ui.separator();
            if ui.button(self.t("Salvar modelo em branco (PNG)...")).clicked() {
                actions.push(Action::Template(i));
                ui.close();
            }
            if ui.add_enabled(has, egui::Button::new(self.t("Limpar"))).clicked() {
                actions.push(Action::Clear(i));
                ui.close();
            }
        });
        if let Some(path) = &self.slots[i].path {
            resp.on_hover_text(path.display().to_string());
        }
        ui.add_space(6.0);
        let label = if i == 0 { self.t("Imprimir esquerda") } else { self.t("Imprimir direita") };
        if theme::button(ui, &p, label, has && self.job.is_none(), 170.0).clicked() {
            actions.push(Action::Print(if i == 0 { Which::Left } else { Which::Right }));
        }
    }

    fn center_ui(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let p = self.pal;
        let now = ui.input(|i| i.time);
        ui.add_space(4.0);
        let (single, lines) = (self.t("Imprimir numa folha só"), self.t("Linhas de corte / dobra"));
        let mut changed = ui.checkbox(&mut self.cfg.single_page, single).changed();
        changed |= ui.checkbox(&mut self.cfg.cut_lines, lines).changed();
        if changed {
            self.persist();
        }
        let below = 92.0;
        let h = (ui.available_height() - below).max(160.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
        let px = (rect.width() / 30.0).min(h / 40.0).clamp(3.0, 7.0);
        let center = pos2(rect.center().x, rect.bottom() - 15.0 * px);
        if now - self.tip_time > 8.0 {
            self.tip = (self.tip + 1) % TIPS.len();
            self.tip_time = now;
        }
        if resp.clicked() {
            self.tip = (self.tip + 1) % TIPS.len();
            self.tip_time = now;
            self.happy_until = now + 1.2;
        }
        if self.happy_until == f64::MAX {
            self.happy_until = now + 2.0;
        }
        let happy = now < self.happy_until;
        let painter = ui.painter_at(rect.expand(4.0));
        theme::mascot(&painter, center, px, now, happy);
        let busy = self.job.is_some();
        let tip = if busy { self.t("Imprimindo... segura aí!") } else { tr(self.cfg.lang, TIPS[self.tip]) };
        theme::speech(&painter, pos2(center.x + 4.0 * px, center.y - 14.0 * px), tip, rect);
        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.add_space(8.0);
        let any = self.slots.iter().any(|s| s.full.is_some());
        if theme::button(ui, &p, self.t("Imprimir ambas"), any && !busy, 170.0).clicked() {
            actions.push(Action::Print(Which::Both));
        }
        ui.add_space(4.0);
        if theme::button(ui, &p, self.t("Pré-visualizar"), any, 170.0).clicked() {
            actions.push(Action::Preview(Which::Both));
        }
    }

    fn status_bar(&self, ui: &mut Ui) {
        let p = self.pal;
        ui.horizontal(|ui| {
            let printer = self.printer().unwrap_or_else(|| self.t("(nenhuma impressora)").into());
            let paper = PAPERS.iter().find(|(_, s)| *s == self.cfg.print.paper).map(|(n, _)| n.to_string()).unwrap_or_else(|| {
                format!("{:.0}×{:.0} mm", self.cfg.print.paper.0, self.cfg.print.paper.1)
            });
            let right_w = 330.0;
            let w = ui.available_width() - right_w - 8.0;
            sunken_field(ui, &p, w, &self.status.0, if self.status.1 { Color32::from_rgb(200, 30, 30) } else { p.text });
            sunken_field(ui, &p, right_w, &format!("{} {printer} · {paper}", self.t("Impressora:")), p.text);
        });
    }

    // ---------- diálogos ----------

    fn options_dialog(&mut self, ctx: &egui::Context) {
        if !self.options_open {
            return;
        }
        let p = self.pal;
        let lang = self.cfg.lang;
        let t = |s| tr(lang, s);
        let mut open = true;
        let mut close = false;
        dialog(ctx, &p, "opts", t("Opções"), &mut open, 620.0, |ui| {
            let o = &mut self.cfg.options;
            ui.checkbox(&mut o.auto_orient, t("Corrigir a orientação automaticamente ao carregar capas"));
            ui.checkbox(&mut o.auto_detect, t("Detectar o formato das imagens automaticamente"));
            ui.checkbox(&mut o.dummy_job, t("Nome falso para o trabalho de impressão (pra imprimir no escritório ;-)"));
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.add_enabled(o.dummy_job, egui::TextEdit::singleline(&mut o.dummy_name).desired_width(360.0));
            });
            ui.checkbox(&mut o.reload_previous, t("Recarregar as capas anteriores ao abrir"));
            ui.checkbox(&mut o.match_cover, t("Tentar carregar a capa par (pelas palavras-chave no nome do arquivo)"));
            egui::Grid::new("kw").num_columns(2).show(ui, |ui| {
                ui.label(format!("     {}", t("Esquerda:")));
                ui.add_enabled(o.match_cover, egui::TextEdit::singleline(&mut o.left_keywords).desired_width(380.0));
                ui.end_row();
                ui.label(format!("     {}", t("Direita:")));
                ui.add_enabled(o.match_cover, egui::TextEdit::singleline(&mut o.right_keywords).desired_width(380.0));
                ui.end_row();
            });
            ui.checkbox(&mut o.splash, t("Mostrar tela de abertura"));
            ui.checkbox(&mut o.correct_area, t("Corrigir capas posicionadas fora da área imprimível"));
            ui.checkbox(&mut o.batch_mode, t("Ativar modo lote (Arquivo > Imprimir em lote)"));
            ui.checkbox(&mut o.border, t("Desenhar uma borda em volta das capas"));
            ui.checkbox(&mut o.grayscale_skin, t("Skin em tons de cinza"));
            ui.checkbox(&mut o.scanlines, t("Linhas de CRT nas skins de monitor (fósforo, âmbar, bolso)"));
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, &p, "OK", true, 90.0).clicked() {
                    close = true;
                }
            });
        });
        if !open || close {
            self.options_open = false;
            self.persist();
        }
    }

    fn print_setup_dialog(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.print_dlg else { return };
        let p = self.pal;
        let lang = self.cfg.lang;
        let t = |s| tr(lang, s);
        let mut open = true;
        let (mut ok, mut cancel, mut calib) = (false, false, false);
        dialog(ctx, &p, "psetup", t("Configurar impressão"), &mut open, 520.0, |ui| {
            let s = &mut dlg.setup;
            egui::Grid::new("ps").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(t("Impressora:"));
                ui.horizontal(|ui| {
                    let def = format!("{} ({})", t("Padrão do sistema"), dlg.default.clone().unwrap_or("-".into()));
                    egui::ComboBox::from_id_salt("printer").width(300.0).selected_text(s.printer.clone().unwrap_or(def.clone())).show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.printer, None, def);
                        for pr in &dlg.printers {
                            ui.selectable_value(&mut s.printer, Some(pr.clone()), pr);
                        }
                    });
                    if ui.button("⟳").on_hover_text(t("Atualizar lista")).clicked() {
                        (dlg.printers, dlg.default) = print::printers();
                    }
                });
                ui.end_row();
                ui.label(t("Papel:"));
                ui.horizontal(|ui| {
                    let cur = PAPERS.iter().find(|(_, z)| *z == s.paper).map(|(n, _)| tr(lang, n)).unwrap_or(t("Personalizado"));
                    egui::ComboBox::from_id_salt("paper").width(170.0).selected_text(cur).show_ui(ui, |ui| {
                        for (n, z) in PAPERS {
                            ui.selectable_value(&mut s.paper, *z, tr(lang, n));
                        }
                        if ui.selectable_label(!PAPERS.iter().any(|(_, z)| *z == s.paper), t("Personalizado")).clicked() {
                            s.paper.0 += 0.1;
                        }
                    });
                    len_edit(ui, &mut s.paper.0, false, true);
                    ui.label("×");
                    len_edit(ui, &mut s.paper.1, false, true);
                });
                ui.end_row();
                ui.label(t("Margem mínima:"));
                len_edit(ui, &mut s.margin, false, true);
                ui.end_row();
                ui.label(t("Cópias:"));
                ui.add(egui::DragValue::new(&mut s.copies).range(1..=99));
                ui.end_row();
                ui.label(t("Capas grandes:"));
                ui.checkbox(&mut s.tile, t("Dividir em várias folhas (para colar)"));
                ui.end_row();
                ui.label(t("Calibração (X / Y):"));
                ui.horizontal(|ui| {
                    len_edit(ui, &mut s.offset.0, false, true);
                    len_edit(ui, &mut s.offset.1, false, true);
                });
                ui.end_row();
            });
            ui.label(
                RichText::new(t("Dica: imprima a página de calibração e meça o quadrado de 100 mm. Se ele sair deslocado, ajuste X/Y (positivo = direita/baixo)."))
                    .size(17.0),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if theme::button(ui, &p, t("Página de calibração"), true, 0.0).clicked() {
                    calib = true;
                }
                if theme::button(ui, &p, t("Impressoras do sistema..."), true, 0.0).clicked() {
                    print::open_system_settings();
                }
            });
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, &p, t("Cancelar"), true, 90.0).clicked() {
                    cancel = true;
                }
                if theme::button(ui, &p, "OK", true, 90.0).clicked() {
                    ok = true;
                }
            });
        });
        if ok || calib {
            self.cfg.print = dlg.setup.clone();
            self.persist();
        }
        if calib {
            self.calibration_page();
        }
        if ok || cancel || !open {
            self.print_dlg = None;
        }
    }

    fn preview_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.preview.take() else { return };
        let p = self.pal;
        let lang = self.cfg.lang;
        let t = |s| tr(lang, s);
        let mut open = true;
        let mut action = None;
        let setup = self.setup();
        let opts = self.draw_opts(self.cfg.print.margin);
        let auto = self.cfg.options.auto_orient;
        // Garante que as texturas de tela estão prontas.
        for i in 0..2 {
            let fmt = self.formats[self.slots[i].format].clone();
            let s = &mut self.slots[i];
            if let Some(img) = &s.small {
                cached(ctx, &mut s.cache, format_key(&fmt, s.generation, s.rotation, auto), img, &fmt, s.rotation, auto);
            }
        }
        let covers: Vec<_> = dlg
            .which
            .sides()
            .iter()
            .filter(|&&i| self.slots[i].cache.is_some())
            .map(|&i| (side_of(i), &self.formats[self.slots[i].format], self.slots[i].cache.as_ref().map(|c| &c.1)))
            .collect();
        let (pages, warnings) = layout::plan(&covers, self.cfg.single_page, &setup);
        dlg.page = dlg.page.min(pages.len().saturating_sub(1));
        let width = ((ctx.content_rect().height() - 230.0).clamp(260.0, 640.0) * 0.8).max(440.0);
        dialog(ctx, &p, "preview", t("Pré-visualizar impressão"), &mut open, width, |ui| {
            ui.horizontal(|ui| {
                for (w, n) in [(Which::Left, t("Esquerda")), (Which::Right, t("Direita")), (Which::Both, t("Ambas"))] {
                    if ui.radio(dlg.which == w, n).clicked() {
                        dlg.which = w;
                        dlg.page = 0;
                    }
                }
            });
            let h = (ctx.content_rect().height() - 230.0).clamp(260.0, 640.0);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, p.desktop);
            bevel_sunken(ui.painter(), rect, &p);
            if let Some(page) = pages.get(dlg.page) {
                let tex = |item: usize| {
                    let side = page.items[item].side;
                    self.slots[if side == Side::Left { 0 } else { 1 }].cache.as_ref().map(|c| c.2.id())
                };
                let pr = paint_page(ui.painter(), rect.shrink(16.0), page, &opts, tex, true);
                // Margem mínima, só como guia na tela.
                let s = pr.width() / page.size.0;
                ui.painter().rect_stroke(pr.shrink(setup.margin * s), 0.0, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 80, 80, 70)), StrokeKind::Inside);
                ui.painter().text(
                    pr.right_bottom() - vec2(4.0, 4.0),
                    Align2::RIGHT_BOTTOM,
                    format!("{:.0} × {:.0} mm", page.size.0, page.size.1),
                    FontId::proportional(16.0),
                    Color32::GRAY,
                );
            } else {
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, t("Nada para imprimir"), FontId::proportional(22.0), p.light);
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(dlg.page > 0, egui::Button::new("◀")).clicked() {
                    dlg.page -= 1;
                }
                ui.label(format!("{} {} / {}", t("Página"), (dlg.page + 1).min(pages.len()), pages.len()));
                if ui.add_enabled(dlg.page + 1 < pages.len(), egui::Button::new("▶")).clicked() {
                    dlg.page += 1;
                }
            });
            for w in &warnings {
                ui.colored_label(Color32::from_rgb(220, 120, 0), format!("⚠ {w}"));
            }
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, &p, t("Fechar"), true, 90.0).clicked() {
                    action = Some(None);
                }
                if theme::button(ui, &p, t("Exportar PDF..."), !pages.is_empty(), 0.0).clicked() {
                    action = Some(Some(Action::Export(dlg.which)));
                }
                if theme::button(ui, &p, t("Imprimir"), !pages.is_empty() && self.job.is_none(), 110.0).clicked() {
                    action = Some(Some(Action::Print(dlg.which)));
                }
            });
        });
        drop(pages);
        match action {
            Some(Some(a)) => self.act(ctx, a),
            Some(None) => {}
            None if open => self.preview = Some(dlg),
            None => {}
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let p = self.pal;
        let lang = self.cfg.lang;
        let t = |s| tr(lang, s);
        let mut open = true;
        let mut close = false;
        let now = ctx.input(|i| i.time);
        dialog(ctx, &p, "about", t("Sobre o UnderCoverNc"), &mut open, 440.0, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(130.0, 150.0), Sense::hover());
                theme::mascot(ui.painter(), r.center() - vec2(0.0, 6.0), 5.0, now, true);
                ui.vertical(|ui| {
                    ui.heading(format!("UnderCoverNc v{VERSION}"));
                    ui.label(t("Imprime capas de CD, DVD, Blu-ray e jogos no tamanho certinho, com linhas de corte e dobra."));
                    ui.label(t("Inspirado no saudoso UnderCoverXP."));
                    ui.label(RichText::new(t("Fonte VT323 (licença OFL). Mascote: a Disquinha.")).size(16.0));
                });
            });
            ui.label(RichText::new(t("Atalhos: Ctrl+O/Ctrl+Shift+O abrir · Ctrl+V colar · Ctrl+P imprimir esquerda · Ctrl+Shift+P ambas · botão direito na capa: girar/limpar")).size(16.0));
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, &p, "OK", true, 90.0).clicked() {
                    close = true;
                }
            });
        });
        if !open || close {
            self.about_open = false;
        }
    }

    fn batch_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.batch.take() else { return };
        let p = self.pal;
        let lang = self.cfg.lang;
        let t = |s| tr(lang, s);
        let mut open = true;
        let mut go: Option<bool> = None; // Some(true) = imprimir, Some(false) = exportar
        let mut pick = false;
        let mut close = false;
        dialog(ctx, &p, "batch", t("Imprimir em lote"), &mut open, 640.0, |ui| {
            ui.label(t("Escolha uma pasta: as capas são pareadas pelas palavras-chave (Opções). Usa os formatos selecionados na janela principal."));
            ui.horizontal(|ui| {
                if theme::button(ui, &p, t("Escolher pasta..."), true, 0.0).clicked() {
                    pick = true;
                }
                if let Some(d) = &dlg.dir {
                    ui.label(d.display().to_string());
                }
            });
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                egui::Grid::new("pairs").striped(true).num_columns(3).show(ui, |ui| {
                    for (l, r, on) in &mut dlg.pairs {
                        ui.checkbox(on, "");
                        ui.label(l.as_deref().map(file_name).unwrap_or("—".into()));
                        ui.label(r.as_deref().map(file_name).unwrap_or("—".into()));
                        ui.end_row();
                    }
                });
            });
            if dlg.dir.is_some() && dlg.pairs.is_empty() {
                ui.label(t("Nenhuma capa encontrada com as palavras-chave."));
            }
            let n = dlg.pairs.iter().filter(|x| x.2).count();
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, &p, t("Fechar"), true, 90.0).clicked() {
                    close = true;
                }
                if theme::button(ui, &p, t("Exportar PDF..."), n > 0, 0.0).clicked() {
                    go = Some(false);
                }
                if theme::button(ui, &p, &format!("{} ({n})", t("Imprimir")), n > 0 && self.job.is_none(), 0.0).clicked() {
                    go = Some(true);
                }
            });
        });
        if pick {
            if let Some(d) = rfd::FileDialog::new().set_title(t("Escolher pasta...")).pick_folder() {
                let l = files::keywords(&self.cfg.options.left_keywords);
                let r = files::keywords(&self.cfg.options.right_keywords);
                dlg.pairs = files::batch_pairs(&d, &l, &r).into_iter().map(|(a, b)| (a, b, true)).collect();
                dlg.dir = Some(d);
            }
        }
        if let Some(print_it) = go {
            let groups: Vec<Vec<CoverJob>> = dlg
                .pairs
                .iter()
                .filter(|x| x.2)
                .map(|(l, r, _)| {
                    [(0, l), (1, r)]
                        .into_iter()
                        .filter_map(|(i, path)| {
                            Some(CoverJob { side: side_of(i), fmt: self.formats[self.slots[i].format].clone(), img: ImgSrc::File(path.clone()?), rotation: 0 })
                        })
                        .collect()
                })
                .collect();
            if print_it {
                let title = self.job_title(Which::Both);
                self.open_print_dialog(groups, true, false, title);
            } else if let Some(path) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name("lote.pdf").save_file() {
                let spec = self.spec(groups, JobKind::Export(path), true);
                self.start_job(spec);
            }
        }
        if open && !close {
            self.batch = Some(dlg);
        }
    }

    fn format_editor(&mut self, ctx: &egui::Context) {
        let Some(mut ed) = self.editor.take() else { return };
        let p = self.pal;
        let lang = self.cfg.lang;
        let t = |s| tr(lang, s);
        let inches = self.cfg.inches;
        let mut inches_new = inches;
        let mut open = true;
        let (mut ok, mut cancel) = (false, false);
        let auto = self.cfg.options.auto_orient;
        let setup = self.setup();
        let opts = self.draw_opts(setup.margin);
        dialog(ctx, &p, "fmted", t("Editor de formatos"), &mut open, 940.0, |ui| {
            // Linha de cima: miniatura esquerda, lista, botões, miniatura direita.
            ui.horizontal_top(|ui| {
                let thumb = |ui: &mut Ui, i: usize, ed: &mut FormatEditor| {
                    let (r, _) = ui.allocate_exact_size(vec2(150.0, 210.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, p.desktop);
                    bevel_sunken(ui.painter(), r, &p);
                    let fmt = ed.list[ed.sel].clone();
                    let s = &self.slots[i];
                    let prepared = s.small.as_ref().map(|img| {
                        let key = format_key(&fmt, s.generation, s.rotation, auto) ^ 0x5eed;
                        cached(ctx, &mut ed.thumbs[i], key, img, &fmt, s.rotation, auto)
                    });
                    let (pages, _) = layout::plan(&[(side_of(i), &fmt, prepared.map(|c| &c.1))], false, &setup);
                    let tex = prepared.map(|c| c.2.id());
                    if let Some(pg) = pages.first() {
                        paint_page(ui.painter(), r.shrink(8.0), pg, &opts, |_| tex, true);
                    }
                };
                thumb(ui, 0, &mut ed);
                ui.vertical(|ui| {
                    let (frame_rect, _) = ui.allocate_exact_size(vec2(380.0, 210.0), Sense::hover());
                    ui.painter().rect_filled(frame_rect, 0.0, p.field);
                    bevel_sunken(ui.painter(), frame_rect, &p);
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(frame_rect.shrink(3.0)));
                    egui::ScrollArea::vertical().id_salt("fmtlist").show(&mut child, |ui| {
                        ui.set_width(frame_rect.width() - 20.0);
                        let mut clicked = None;
                        for (k, f) in ed.list.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
                            let sel = k == ed.sel;
                            let bg = if sel {
                                p.highlight
                            } else if k % 2 == 0 {
                                p.field.lerp_to_gamma(p.accent, 0.08)
                            } else {
                                p.field
                            };
                            ui.painter().rect_filled(r, 0.0, bg);
                            let col = if sel { contrast(p.highlight) } else { p.field_text };
                            let font = if sel { FontId::proportional(21.0) } else { FontId::proportional(19.0) };
                            ui.painter().text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, &f.name, font, col);
                            if resp.clicked() {
                                clicked = Some(k);
                            }
                        }
                        if let Some(k) = clicked {
                            if commit_markers(&mut ed) {
                                ed.sel = k;
                                ed.markers = ed.list[k].markers_text();
                            }
                        }
                    });
                });
                ui.vertical(|ui| {
                    let n = ed.list.len();
                    let sel = ed.sel;
                    if theme::button(ui, &p, t("Remover"), n > 1, 120.0).clicked() {
                        ed.list.remove(sel);
                        ed.sel = sel.min(ed.list.len() - 1);
                        ed.markers = ed.list[ed.sel].markers_text();
                        ed.markers_err = None;
                    }
                    if theme::button(ui, &p, t("Adicionar"), true, 120.0).clicked() && commit_markers(&mut ed) {
                        let mut f = ed.list[sel].clone();
                        f.name = t("Novo formato").into();
                        f.kind = InternalType::Custom;
                        f.autodetect = false;
                        ed.list.insert(sel + 1, f);
                        ed.sel = sel + 1;
                        ed.markers = ed.list[ed.sel].markers_text();
                    }
                    if theme::button(ui, &p, t("Copiar"), true, 120.0).clicked() && commit_markers(&mut ed) {
                        let mut f = ed.list[sel].clone();
                        f.name = format!("{} ({})", f.name, t("cópia"));
                        ed.list.insert(sel + 1, f);
                        ed.sel = sel + 1;
                    }
                    if theme::button(ui, &p, t("Subir"), sel > 0, 120.0).clicked() {
                        ed.list.swap(sel, sel - 1);
                        ed.sel -= 1;
                    }
                    if theme::button(ui, &p, t("Descer"), sel + 1 < n, 120.0).clicked() {
                        ed.list.swap(sel, sel + 1);
                        ed.sel += 1;
                    }
                    ui.add_space(4.0);
                    if theme::button(ui, &p, t("Padrões"), true, 120.0).on_hover_text(t("Volta à lista de formatos original")).clicked() {
                        ed.list = format::default_formats();
                        ed.sel = 0;
                        ed.markers = ed.list[0].markers_text();
                        ed.markers_err = None;
                    }
                });
                thumb(ui, 1, &mut ed);
            });
            ui.add_space(6.0);
            // Linha de baixo: campos.
            let sel = ed.sel;
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(230.0);
                    let f = &mut ed.list[sel];
                    pos_group(ui, &p, t("Posição esquerda"), &mut f.left_pos, inches, lang);
                    theme::group(ui, &p, t("Tamanho"), |ui| {
                        egui::Grid::new("size").num_columns(2).show(ui, |ui| {
                            ui.label(t("Largura:"));
                            len_edit(ui, &mut f.width, inches, true);
                            ui.end_row();
                            ui.label(t("Altura:"));
                            len_edit(ui, &mut f.height, inches, true);
                            ui.end_row();
                        });
                        f.width = f.width.max(5.0);
                        f.height = f.height.max(5.0);
                    });
                    theme::group(ui, &p, t("Papel (bandeja)"), |ui| {
                        let mut own = f.paper.is_some();
                        if ui.checkbox(&mut own, t("Papel próprio")).changed() {
                            f.paper = own.then_some(self.cfg.print.paper);
                        }
                        let mut z = f.paper.unwrap_or(self.cfg.print.paper);
                        ui.horizontal(|ui| {
                            len_edit(ui, &mut z.0, inches, own);
                            ui.label("×");
                            len_edit(ui, &mut z.1, inches, own);
                        });
                        if own {
                            f.paper = Some((z.0.max(10.0), z.1.max(10.0)));
                        }
                    });
                });
                ui.vertical(|ui| {
                    ui.set_width(440.0);
                    let f = &mut ed.list[sel];
                    egui::Grid::new("fields").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label(t("Nome:"));
                        ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(300.0));
                        ui.end_row();
                        ui.label(t("Tipo interno:"));
                        egui::ComboBox::from_id_salt("kind").width(220.0).selected_text(f.kind.code()).show_ui(ui, |ui| {
                            for k in InternalType::ALL {
                                ui.selectable_value(&mut f.kind, k, k.code());
                            }
                        });
                        ui.end_row();
                        ui.checkbox(&mut f.label, t("Rótulo"));
                        ui.horizontal(|ui| {
                            ui.add_enabled(f.label, egui::Label::new(t("Furo:")));
                            len_edit(ui, &mut f.hole, inches, f.label);
                        });
                        ui.end_row();
                        let slim = f.kind == InternalType::SlimlineDvd;
                        let mut alt = f.alt_spine.is_some();
                        if ui.add_enabled(slim, egui::Checkbox::new(&mut alt, t("Lombada alternativa"))).changed() {
                            f.alt_spine = alt.then_some(9.0);
                        }
                        ui.horizontal(|ui| {
                            let mut v = f.alt_spine.unwrap_or(9.0);
                            ui.add_enabled(slim && alt, egui::Label::new(t("Valor:")));
                            if len_edit(ui, &mut v, inches, slim && alt) {
                                f.alt_spine = Some(v.clamp(1.0, 80.0));
                            }
                        });
                        ui.end_row();
                        ui.checkbox(&mut f.autodetect, t("Detecção automática"));
                        ui.horizontal(|ui| {
                            ui.label(t("Encaixe:"));
                            let fit_name = |m: FitMode| match m {
                                FitMode::Stretch => tr(lang, "Esticar"),
                                FitMode::Fit => tr(lang, "Caber (bordas)"),
                                FitMode::Fill => tr(lang, "Preencher (cortar)"),
                            };
                            egui::ComboBox::from_id_salt("fit").width(170.0).selected_text(fit_name(f.fit)).show_ui(ui, |ui| {
                                for m in [FitMode::Stretch, FitMode::Fit, FitMode::Fill] {
                                    ui.selectable_value(&mut f.fit, m, fit_name(m));
                                }
                            });
                        });
                        ui.end_row();
                    });
                    ui.label(t("Marcadores de corte / dobra:"));
                    let r = ui.add(egui::TextEdit::multiline(&mut ed.markers).font(TextStyle::Monospace).desired_rows(5).desired_width(430.0));
                    if r.changed() {
                        ed.markers_err = format::parse_markers(&ed.markers).err();
                        if ed.markers_err.is_none() {
                            commit_markers(&mut ed);
                        }
                    }
                    ui.horizontal(|ui| {
                        let add = |s: &str, ed: &mut FormatEditor| {
                            if !ed.markers.is_empty() && !ed.markers.ends_with('\n') {
                                ed.markers.push('\n');
                            }
                            ed.markers.push_str(s);
                            commit_markers(ed);
                        };
                        if theme::button(ui, &p, t("+ Linha de borda"), true, 0.0).clicked() {
                            add("LINE = LEFT, DASHED", &mut ed);
                        }
                        if theme::button(ui, &p, t("+ Linha"), true, 0.0).clicked() {
                            add("LINE = (0.0, -10.0)(0.0, 0.0), DASHED", &mut ed);
                        }
                        if theme::button(ui, &p, t("+ Retângulo"), true, 0.0).clicked() {
                            add("RECT = (0.0, 0.0)(50.0, 50.0), SOLID", &mut ed);
                        }
                    });
                    match &ed.markers_err {
                        Some(e) => ui.colored_label(Color32::from_rgb(210, 40, 40), e),
                        None => ui.label(RichText::new(t("LINE = LEFT|RIGHT|TOP|BOTTOM  ·  LINE = (x1, y1)(x2, y2)  ·  RECT = (x, y)(larg, alt)  ·  , SOLID|DASHED")).size(15.0)),
                    };
                });
                ui.vertical(|ui| {
                    let f = &mut ed.list[sel];
                    pos_group(ui, &p, t("Posição direita"), &mut f.right_pos, inches, lang);
                    ui.label(t("Tipo de mídia da impressora:"));
                    choice_combo(ui, "media", &mut f.media_type, &ed.printer_opts.media_types, t("(padrão)"), lang == Lang::Pt);
                    ui.label(t("Bandeja / origem do papel:"));
                    choice_combo(ui, "source", &mut f.paper_source, &ed.printer_opts.sources, t("(padrão)"), false);
                    ui.add_space(4.0);
                    theme::group(ui, &p, t("Unidades"), |ui| {
                        ui.radio_value(&mut inches_new, true, t("Polegadas"));
                        ui.radio_value(&mut inches_new, false, t("Milímetros"));
                    });
                });
            });
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, &p, t("Cancelar"), true, 100.0).clicked() {
                    cancel = true;
                }
                if theme::button(ui, &p, "OK", ed.markers_err.is_none(), 100.0).clicked() {
                    ok = true;
                }
            });
        });
        if inches_new != inches {
            self.cfg.inches = inches_new;
        }
        if ok && commit_markers(&mut ed) {
            let names: Vec<String> = self.slots.iter().map(|s| self.formats[s.format].name.clone()).collect();
            self.formats = ed.list;
            for (i, n) in names.iter().enumerate() {
                self.slots[i].format = self.formats.iter().position(|f| &f.name == n).unwrap_or(i.min(self.formats.len() - 1));
            }
            match format::save_formats(&self.formats) {
                Ok(()) => self.ok(self.t("Formatos salvos.")),
                Err(e) => self.err(format!("{e:#}")),
            }
            self.persist();
            return;
        }
        if open && !cancel && !ok {
            self.editor = Some(ed);
        }
    }

    fn splash(&mut self, ctx: &egui::Context) {
        let Some(start) = self.start else { return };
        let now = ctx.input(|i| i.time);
        let start = if start == 0.0 {
            self.start = Some(now);
            now
        } else {
            start
        };
        let el = now - start;
        if el > SPLASH_SECS || ctx.input(|i| i.pointer.any_click() || !i.keys_down.is_empty()) && el > 0.2 {
            self.start = None;
            return;
        }
        let p = self.pal;
        egui::Area::new("splash".into()).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(440.0, 300.0), Sense::click());
            let painter = ui.painter();
            painter.rect_filled(r.translate(vec2(8.0, 8.0)), 0.0, Color32::from_black_alpha(120));
            painter.rect_filled(r, 0.0, p.face);
            theme::bevel(painter, r, &p, false);
            let tb = Rect::from_min_size(r.min + vec2(4.0, 4.0), vec2(r.width() - 8.0, 28.0));
            theme::gradient(painter, tb, p.title_a, p.title_b);
            painter.text(tb.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, format!("UnderCoverNc v{VERSION}"), FontId::proportional(21.0), p.title_text);
            theme::mascot(painter, pos2(r.left() + 105.0, r.center().y + 10.0), 5.5, now, el > 1.4);
            painter.text(pos2(r.left() + 210.0, r.top() + 90.0), Align2::LEFT_CENTER, "UnderCover", FontId::proportional(44.0), p.text);
            painter.text(pos2(r.left() + 210.0, r.top() + 128.0), Align2::LEFT_CENTER, "Nc", FontId::proportional(44.0), p.accent);
            painter.text(pos2(r.left() + 210.0, r.top() + 165.0), Align2::LEFT_CENTER, self.t("capas no tamanho certinho"), FontId::proportional(19.0), p.text);
            // Barra de "carregando" em blocos.
            let bar = Rect::from_min_size(pos2(r.left() + 210.0, r.top() + 200.0), vec2(200.0, 22.0));
            painter.rect_filled(bar, 0.0, p.field);
            bevel_sunken(painter, bar, &p);
            let n = ((el / (SPLASH_SECS - 0.4)).min(1.0) * 12.0) as i32;
            for k in 0..n {
                painter.rect_filled(
                    Rect::from_min_size(pos2(bar.left() + 4.0 + k as f32 * 16.0, bar.top() + 4.0), vec2(13.0, 14.0)),
                    0.0,
                    p.accent,
                );
            }
            painter.text(pos2(r.left() + 210.0, r.top() + 245.0), Align2::LEFT_CENTER, self.t("carregando capinhas..."), FontId::proportional(17.0), p.disabled);
        });
    }
}

fn item(ui: &mut Ui, actions: &mut Vec<Action>, text: &str, sc: &str, enabled: bool, a: Action) {
    if ui.add_enabled(enabled, egui::Button::new(text).shortcut_text(sc)).clicked() {
        actions.push(a);
        ui.close();
    }
}

fn contrast(c: Color32) -> Color32 {
    if (c.r() as u32 * 299 + c.g() as u32 * 587 + c.b() as u32 * 114) / 1000 > 128 { Color32::BLACK } else { Color32::WHITE }
}

fn commit_markers(ed: &mut FormatEditor) -> bool {
    match format::parse_markers(&ed.markers) {
        Ok(m) => {
            ed.list[ed.sel].markers = m;
            ed.markers_err = None;
            true
        }
        Err(e) => {
            ed.markers_err = Some(e);
            false
        }
    }
}

fn choice_combo(ui: &mut Ui, id: &str, v: &mut String, choices: &[(String, String)], default: &str, pt: bool) {
    let name = |l: &str| if pt { print::media_pt(l) } else { l.to_string() };
    let shown = if v.is_empty() {
        default.to_string()
    } else {
        choices.iter().find(|(c, _)| c == v).map(|(_, l)| name(l)).unwrap_or_else(|| v.clone())
    };
    egui::ComboBox::from_id_salt(id).width(200.0).selected_text(shown).show_ui(ui, |ui| {
        ui.selectable_value(v, String::new(), default);
        for (c, l) in choices {
            ui.selectable_value(v, c.clone(), name(l));
        }
    });
}

fn pos_group(ui: &mut Ui, p: &Palette, title: &str, pos: &mut Option<(f32, f32)>, inches: bool, lang: Lang) {
    theme::group(ui, p, title, |ui| {
        let mut manual = pos.is_some();
        ui.radio_value(&mut manual, false, tr(lang, "Automática"));
        ui.radio_value(&mut manual, true, tr(lang, "Manual"));
        let mut v = pos.unwrap_or((10.0, 10.0));
        ui.horizontal(|ui| {
            ui.label("X/Y");
            len_edit(ui, &mut v.0, inches, manual);
            len_edit(ui, &mut v.1, inches, manual);
        });
        *pos = manual.then_some(v);
    });
}

fn skin_name(lang: Lang, s: Skin) -> &'static str {
    tr(
        lang,
        match s {
            Skin::Classic => "Clássico 98",
            Skin::Phosphor => "Fósforo verde",
            Skin::Amber => "Monitor âmbar",
            Skin::Vapor => "Vaporwave",
            Skin::Pocket => "Bolso (4 tons)",
            Skin::Random => "Aleatória",
        },
    )
}

fn bevel_sunken(painter: &egui::Painter, r: Rect, p: &Palette) {
    theme::bevel(painter, r, p, true);
}

fn sunken_field(ui: &mut Ui, p: &Palette, w: f32, text: &str, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(w.max(40.0), 24.0), Sense::hover());
    let painter = ui.painter_at(r);
    bevel_sunken(&painter, r, p);
    painter.text(r.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, text, FontId::proportional(18.0), color);
}

/// Janela de diálogo retrô: sem a barra do egui, com faixa em degradê e botão X.
fn dialog(ctx: &egui::Context, p: &Palette, id: &str, title: &str, open: &mut bool, width: f32, add: impl FnOnce(&mut Ui)) {
    let now = ctx.input(|i| i.time);
    egui::Window::new(id)
        .id(egui::Id::new(id))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(egui::Frame::window(&ctx.style()).inner_margin(4.0).fill(p.face).corner_radius(0.0))
        .show(ctx, |ui| {
            ui.set_width(width);
            let top = ui.cursor().min;
            theme::title_bar(ui, p, title, now);
            let close = Rect::from_min_size(pos2(top.x + width - 30.0, top.y + 4.0), vec2(26.0, 22.0));
            if ui.put(close, |ui: &mut Ui| theme::button(ui, p, "x", true, 26.0)).clicked() {
                *open = false;
            }
            egui::Frame::new().inner_margin(10.0).show(ui, add);
            let full = ui.min_rect().expand(4.0);
            theme::bevel(ui.painter(), full, p, false);
        });
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        *open = false;
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Roda fora da interface: carrega, prepara em resolução cheia, monta o PDF e imprime/salva.
fn run_job(spec: JobSpec, lang: Lang) -> anyhow::Result<String> {
    let mut groups = Vec::new();
    for g in &spec.groups {
        let mut v = Vec::new();
        for c in g {
            let img = match &c.img {
                ImgSrc::Mem(a) => a.clone(),
                ImgSrc::File(p) => Arc::new(layout::limit_size(files::load_image(p)?, FULL_MAX)),
            };
            let mut fmt = c.fmt.clone();
            if let Some(list) = &spec.detect {
                if let Some(k) = layout::detect_format(img.width(), img.height(), list, spec.auto_orient) {
                    fmt = list[k].clone();
                }
            }
            let prep = layout::prepare(&img, &fmt, c.rotation, spec.auto_orient);
            v.push((c.side, fmt, prep));
        }
        groups.push(v);
    }
    let mut pages = Vec::new();
    let mut warns = Vec::new();
    for g in &groups {
        let covers: Vec<_> = g.iter().map(|(s, f, p)| (*s, f, Some(p))).collect();
        let (pg, w) = layout::plan(&covers, spec.single_page, &spec.setup);
        pages.extend(pg);
        warns.extend(w);
    }
    let opts = DrawOpts { cut_lines: spec.cut_lines, border: spec.border, margin: spec.setup.margin };
    let title = match &spec.kind {
        JobKind::Print { title, .. } => title.clone(),
        JobKind::Export(p) => file_name(p),
    };
    let pdf = pdf::render(&pages, &opts, spec.offset, &title)?;
    let warn = if warns.is_empty() { String::new() } else { format!("  ⚠ {}", warns.join(" ")) };
    let first = groups.iter().flatten().next().map(|g| &g.1);
    match &spec.kind {
        JobKind::Export(path) => {
            std::fs::write(path, &pdf)?;
            Ok(format!("{} {} ({} {}){warn}", tr(lang, "PDF salvo:"), path.display(), pages.len(), tr(lang, "página(s)")))
        }
        JobKind::Print { printer, title, copies, choice } => {
            let path = std::env::temp_dir().join(format!("undercovernc-{}.pdf", std::process::id()));
            std::fs::write(&path, &pdf)?;
            let job = print::Job {
                printer,
                title,
                copies: *copies,
                paper: pages.first().map(|p| p.size).unwrap_or(spec.setup.paper),
                media_type: if choice.media.is_empty() { first.map(|f| f.media_type.as_str()).unwrap_or("") } else { &choice.media },
                paper_source: first.map(|f| f.paper_source.as_str()).unwrap_or(""),
                grayscale: choice.grayscale,
            };
            // O lp copia o arquivo para a fila do CUPS, então o PDF temporário pode sumir logo.
            let out = print::print_pdf(&path, &job);
            let _ = std::fs::remove_file(&path);
            let out = out?;
            Ok(format!("{} {printer} ({} {}). {out}{warn}", tr(lang, "Enviado para"), pages.len(), tr(lang, "página(s)")))
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let gray = self.cfg.options.grayscale_skin;
        if self.applied != Some((self.skin, gray)) {
            self.pal = if gray { theme::palette(self.skin).grayscale() } else { theme::palette(self.skin) };
            theme::apply(ctx, &self.pal);
            self.applied = Some((self.skin, gray));
        }
        self.poll_job();
        self.handle_drop(ctx);
        let mut actions = Vec::new();
        self.shortcuts(ctx, &mut actions);
        let p = self.pal;
        let title = format!("UnderCoverNc v{VERSION} - {}", self.t("Que capas vamos imprimir hoje?"));
        if self.title_sent != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_sent = title.clone();
        }

        egui::TopBottomPanel::top("top").frame(egui::Frame::new().fill(p.face).inner_margin(egui::Margin { left: 4, right: 4, top: 4, bottom: 2 })).show(ctx, |ui| {
            theme::title_bar(ui, &p, &title, ui.input(|i| i.time));
            ui.add_space(2.0);
            self.menu(ui, &mut actions);
        });
        egui::TopBottomPanel::bottom("status").frame(egui::Frame::new().fill(p.face).inner_margin(4.0)).show(ctx, |ui| self.status_bar(ui));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.face).inner_margin(10.0)).show(ctx, |ui| {
            let r = ui.max_rect().expand(6.0);
            theme::bevel(ui.painter(), r, &p, false);
            let avail = ui.available_size();
            let side_w = ((avail.x - 40.0) * 0.37).min(420.0);
            let center_w = avail.x - 2.0 * side_w - 2.0 * ui.spacing().item_spacing.x;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(side_w, avail.y), Layout::top_down(Align::Center), |ui| self.slot_ui(ui, 0, &mut actions));
                ui.allocate_ui_with_layout(vec2(center_w, avail.y), Layout::top_down(Align::Center), |ui| self.center_ui(ui, &mut actions));
                ui.allocate_ui_with_layout(vec2(side_w, avail.y), Layout::top_down(Align::Center), |ui| self.slot_ui(ui, 1, &mut actions));
            });
        });

        self.options_dialog(ctx);
        self.print_setup_dialog(ctx);
        self.print_dialog(ctx);
        self.preview_dialog(ctx);
        self.format_editor(ctx);
        self.batch_dialog(ctx);
        self.about_dialog(ctx);
        self.splash(ctx);
        if p.crt && self.cfg.options.scanlines {
            theme::scanlines(ctx);
        }
        for a in actions {
            self.act(ctx, a);
        }
        let sig = hash_of((format!("{:?}", self.cfg), self.slots.iter().map(|s| (s.path.clone(), s.rotation, s.format)).collect::<Vec<_>>()));
        if sig != self.saved_sig {
            self.saved_sig = sig;
            self.persist();
        }
        ctx.request_repaint_after(Duration::from_millis(40));
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.persist();
    }
}
