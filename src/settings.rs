//! Configuração persistente (~/.config/undercovernc/config.json).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Lang {
    #[default]
    Pt,
    En,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Skin {
    #[default]
    Classic,
    Phosphor,
    Amber,
    Vapor,
    Pocket,
    Random,
}

impl Skin {
    pub const ALL: [Skin; 6] = [Skin::Classic, Skin::Phosphor, Skin::Amber, Skin::Vapor, Skin::Pocket, Skin::Random];
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub auto_orient: bool,
    pub auto_detect: bool,
    pub dummy_job: bool,
    pub dummy_name: String,
    pub reload_previous: bool,
    pub match_cover: bool,
    pub left_keywords: String,
    pub right_keywords: String,
    pub splash: bool,
    pub correct_area: bool,
    pub batch_mode: bool,
    pub border: bool,
    pub grayscale_skin: bool,
    pub scanlines: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            auto_orient: true,
            auto_detect: true,
            dummy_job: false,
            dummy_name: "Microsoft Word - Documento1".into(),
            reload_previous: true,
            match_cover: true,
            left_keywords: "front;frente;front + inside;-f".into(),
            right_keywords: "back;verso;back;-b".into(),
            splash: true,
            correct_area: true,
            batch_mode: false,
            border: false,
            grayscale_skin: false,
            scanlines: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PrintSetup {
    pub printer: Option<String>,
    pub paper: (f32, f32),
    pub margin: f32,
    /// Ajuste fino (calibração) somado a tudo na hora de imprimir, em mm.
    pub offset: (f32, f32),
    pub copies: u32,
    /// Divide capas maiores que o papel em várias folhas (para colar depois).
    pub tile: bool,
}

impl Default for PrintSetup {
    fn default() -> Self {
        Self { printer: None, paper: (210.0, 297.0), margin: 3.0, offset: (0.0, 0.0), copies: 1, tile: true }
    }
}

/// Última escolha da janela de impressão para uma impressora.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct PrintChoice {
    pub media: String,
    pub grayscale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SlotState {
    pub path: Option<PathBuf>,
    pub rotation: u8,
    pub format: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub options: Options,
    pub print: PrintSetup,
    pub skin: Skin,
    pub lang: Lang,
    pub single_page: bool,
    pub cut_lines: bool,
    pub inches: bool,
    pub left: SlotState,
    pub right: SlotState,
    /// Última versão da lista de formatos padrão já mesclada nos formatos salvos.
    pub formats_version: u32,
    /// Tipo de papel/qualidade escolhido por impressora.
    pub print_choices: std::collections::BTreeMap<String, PrintChoice>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            options: Options::default(),
            print: PrintSetup::default(),
            skin: Skin::default(),
            lang: Lang::default(),
            single_page: false,
            cut_lines: true,
            inches: false,
            left: SlotState { format: "CD Frente".into(), ..Default::default() },
            right: SlotState { format: "CD Verso".into(), ..Default::default() },
            formats_version: 0,
            print_choices: Default::default(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("undercovernc")
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("undercovernc")
}

fn path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load() -> Config {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(c: &Config) {
    let p = path();
    let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
    if let Ok(b) = serde_json::to_vec_pretty(c) {
        let _ = std::fs::write(p, b);
    }
}
