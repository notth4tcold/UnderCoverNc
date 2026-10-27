//! Formatos de capa: tamanho, tipo interno, linhas de corte/dobra e afins.
//! Todas as medidas ficam em milímetros; polegadas são só exibição.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Lombada padrão de uma capa de DVD (usada para remapear para slim/6 DVD).
pub const DVD_SPINE_MM: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InternalType {
    Custom,
    CdFront,
    CdFrontInside,
    CdBack,
    Dvd,
    SlimlineDvd,
    CdLabel,
}

impl InternalType {
    pub const ALL: [InternalType; 7] = [
        Self::Custom,
        Self::CdFront,
        Self::CdFrontInside,
        Self::CdBack,
        Self::Dvd,
        Self::SlimlineDvd,
        Self::CdLabel,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Custom => "CUSTOM",
            Self::CdFront => "CD_FRONT",
            Self::CdFrontInside => "CD_FRONT_INSIDE",
            Self::CdBack => "CD_BACK",
            Self::Dvd => "DVD",
            Self::SlimlineDvd => "SLIMLINE_DVD",
            Self::CdLabel => "CD_LABEL",
        }
    }
}

/// Como a imagem ocupa a área da capa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FitMode {
    /// Estica para o tamanho exato (o normal para capas escaneadas).
    #[default]
    Stretch,
    /// Cabe inteira, sobrando bordas brancas.
    Fit,
    /// Preenche tudo, cortando o que sobrar.
    Fill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineStyle {
    Solid,
    Dashed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Marcador de corte/dobra, em mm relativos ao canto superior esquerdo da capa.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Marker {
    /// Linha ao longo de uma borda, estendida até a margem do papel (fora da capa).
    Edge(Edge, LineStyle),
    Line { a: (f32, f32), b: (f32, f32), style: LineStyle },
    /// Retângulo (x, y, largura, altura).
    Rect { pos: (f32, f32), size: (f32, f32), style: LineStyle },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoverFormat {
    pub name: String,
    pub kind: InternalType,
    pub width: f32,
    pub height: f32,
    /// Posição manual (x, y) no papel quando a capa está na esquerda; None = automática.
    pub left_pos: Option<(f32, f32)>,
    pub right_pos: Option<(f32, f32)>,
    /// Rótulo redondo de CD (recorta em círculo com furo).
    pub label: bool,
    pub hole: f32,
    /// Lombada alternativa: a imagem é tratada como capa de DVD e a lombada é remapeada.
    pub alt_spine: Option<f32>,
    pub autodetect: bool,
    #[serde(default)]
    pub fit: FitMode,
    /// Papel próprio (ex.: bandeja de CD); None = papel da configuração de impressão.
    #[serde(default)]
    pub paper: Option<(f32, f32)>,
    #[serde(default)]
    pub media_type: String,
    #[serde(default)]
    pub paper_source: String,
    pub markers: Vec<Marker>,
}

impl CoverFormat {
    fn new(name: &str, kind: InternalType, width: f32, height: f32) -> Self {
        Self {
            name: name.into(),
            kind,
            width,
            height,
            left_pos: None,
            right_pos: None,
            label: kind == InternalType::CdLabel,
            hole: 41.0,
            alt_spine: None,
            autodetect: false,
            fit: FitMode::Stretch,
            paper: None,
            media_type: String::new(),
            paper_source: String::new(),
            markers: edges(&[Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]),
        }
    }

    fn auto(mut self) -> Self {
        self.autodetect = true;
        self
    }

    fn spine(mut self, mm: f32) -> Self {
        self.alt_spine = Some(mm);
        self
    }

    fn fit(mut self, fit: FitMode) -> Self {
        self.fit = fit;
        self
    }

    fn markers(mut self, m: Vec<Marker>) -> Self {
        self.markers = m;
        self
    }

    /// Lombada (início ao longo do lado maior, espessura) quando a capa tem uma.
    pub fn spine_band(&self) -> Option<(f32, f32)> {
        let s = match self.kind {
            InternalType::Dvd => DVD_SPINE_MM,
            InternalType::SlimlineDvd => self.alt_spine.unwrap_or(DVD_SPINE_MM),
            _ => return None,
        };
        Some(((self.width.max(self.height) - s) / 2.0, s))
    }

    pub fn markers_text(&self) -> String {
        self.markers.iter().map(marker_to_string).collect::<Vec<_>>().join("\n")
    }
}

fn edges(e: &[Edge]) -> Vec<Marker> {
    e.iter().map(|&e| Marker::Edge(e, LineStyle::Dashed)).collect()
}

fn line(a: (f32, f32), b: (f32, f32)) -> Marker {
    Marker::Line { a, b, style: LineStyle::Dashed }
}

/// Marcas de dobra das lombadas laterais do verso de CD (6,5 mm de cada lado).
fn cd_back_markers(w: f32, h: f32) -> Vec<Marker> {
    let mut m = edges(&[Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]);
    for x in [6.5, w - 6.5] {
        m.push(line((x, -10.0), (x, 0.0)));
        m.push(line((x, h), (x, h + 10.0)));
    }
    m
}

/// Marcas de dobra (tracinhos fora da capa, à esquerda e à direita) nas alturas `ys`.
fn folds_y(w: f32, ys: &[f32]) -> Vec<Marker> {
    let mut m = edges(&[Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]);
    for &y in ys {
        m.push(Marker::Line { a: (-9.0, y), b: (-2.0, y), style: LineStyle::Solid });
        m.push(Marker::Line { a: (w + 2.0, y), b: (w + 9.0, y), style: LineStyle::Solid });
    }
    m
}

/// Versão da lista de formatos padrão; ao subir, os formatos novos entram na lista salva.
pub const DEFAULTS_VERSION: u32 = 2;

pub fn default_formats() -> Vec<CoverFormat> {
    use InternalType::*;
    let cd_back = |name: &str| CoverFormat::new(name, CdBack, 151.0, 117.6).markers(cd_back_markers(151.0, 117.6));
    vec![
        CoverFormat::new("CD Frente", CdFront, 120.5, 120.5).auto(),
        cd_back("CD Verso").auto(),
        CoverFormat::new("CD Frente + Interno", CdFrontInside, 120.5, 241.0).auto().markers({
            let mut m = edges(&[Edge::Left, Edge::Right, Edge::Top, Edge::Bottom]);
            m.push(line((-10.0, 120.5), (0.0, 120.5)));
            m.push(line((120.5, 120.5), (130.5, 120.5)));
            m
        }),
        CoverFormat::new("Caixa de DVD", Dvd, 184.2, 274.3).auto(),
        CoverFormat::new("Blu-ray", Custom, 147.3, 264.2).auto(),
        CoverFormat::new("Caixa de DVD meia altura", Custom, 135.0, 254.0),
        CoverFormat::new("DVD impresso como Slim 9mm", SlimlineDvd, 183.0, 268.0).spine(9.0),
        CoverFormat::new("DVD impresso como Slim 7mm", SlimlineDvd, 183.0, 268.0).spine(7.0),
        CoverFormat::new("Caixa Slim DVD 9mm", Custom, 183.0, 264.0),
        CoverFormat::new("Caixa Slim DVD 7mm", Custom, 183.0, 262.0),
        CoverFormat::new("DVD impresso como caixa de 6 DVDs", SlimlineDvd, 183.0, 285.0).spine(24.0),
        CoverFormat::new("Caixa de 6 DVDs", Custom, 183.0, 285.0),
        cd_back("PlayStation 1 Verso"),
        CoverFormat::new("PlayStation 2 / Nintendo Wii", Dvd, 183.0, 273.0),
        CoverFormat::new("PlayStation 3", Custom, 150.0, 270.0),
        CoverFormat::new("PlayStation Portable (UMD)", Custom, 135.0, 177.0),
        CoverFormat::new("CD Verso sem laterais", Custom, 138.0, 117.6),
        CoverFormat::new("Rótulo de CD (A4)", CdLabel, 117.0, 117.0).markers(vec![]),
        CoverFormat::new("Foto 10x15 (bordas brancas)", Custom, 102.0, 152.0).fit(FitMode::Fit),
        CoverFormat::new("Foto 10x15 (cortada)", Custom, 102.0, 152.0).fit(FitMode::Fill),
        // VHS. Caixa de locadora: 10,125" × 7,75" com lombada de 1,125" (duplication.com).
        CoverFormat::new("VHS caixa de locadora", Custom, 196.9, 257.2).markers(folds_y(196.9, &[114.3, 142.9])),
        // Clamshell: encarte de 8 5/16" × 11 19/32"; lombada = espessura da caixa fechada, 1 3/32".
        CoverFormat::new("VHS clamshell", Custom, 211.1, 294.5).markers(folds_y(211.1, &[133.4, 161.2])),
        // Luva de papelão (slipcover) planificada, tamanho total (vhs.texs.org).
        CoverFormat::new("VHS luva de papelão", Custom, 233.0, 273.0),
        CoverFormat::new("VHS etiqueta da frente", Custom, 78.0, 47.0),
        CoverFormat::new("VHS etiqueta da lombada", Custom, 147.0, 17.0),
        CoverFormat::new("VHS-C luva", Custom, 126.0, 171.0),
    ]
}

// ---------- texto dos marcadores (mesma sintaxe do UnderCoverXP) ----------

fn style_str(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "SOLID",
        LineStyle::Dashed => "DASHED",
    }
}

pub fn marker_to_string(m: &Marker) -> String {
    match *m {
        Marker::Edge(e, s) => {
            let e = match e {
                Edge::Left => "LEFT",
                Edge::Right => "RIGHT",
                Edge::Top => "TOP",
                Edge::Bottom => "BOTTOM",
            };
            format!("LINE = {e}, {}", style_str(s))
        }
        Marker::Line { a, b, style } => {
            format!("LINE = ({:.1}, {:.1})({:.1}, {:.1}), {}", a.0, a.1, b.0, b.1, style_str(style))
        }
        Marker::Rect { pos, size, style } => {
            format!("RECT = ({:.1}, {:.1})({:.1}, {:.1}), {}", pos.0, pos.1, size.0, size.1, style_str(style))
        }
    }
}

fn parse_pair(s: &str) -> Result<(f32, f32)> {
    let s = s.trim().trim_start_matches('(').trim_end_matches(')');
    let mut it = s.split(',').map(|v| v.trim().parse::<f32>());
    match (it.next(), it.next(), it.next()) {
        (Some(Ok(a)), Some(Ok(b)), None) => Ok((a, b)),
        _ => bail!("coordenada inválida: ({s})"),
    }
}

pub fn parse_marker(line: &str) -> Result<Marker> {
    let (kind, rest) = line.split_once('=').ok_or_else(|| anyhow::anyhow!("falta '='"))?;
    let kind = kind.trim().to_ascii_uppercase();
    let rest = rest.trim();
    let (body, style) = match rest.rsplit_once(',') {
        Some((b, s)) if !s.trim_end().ends_with(')') && !s.trim().starts_with(|c: char| c.is_ascii_digit() || c == '-') => {
            let style = match s.trim().to_ascii_uppercase().as_str() {
                "SOLID" => LineStyle::Solid,
                "DASHED" => LineStyle::Dashed,
                o => bail!("estilo desconhecido: {o} (use SOLID ou DASHED)"),
            };
            (b.trim(), style)
        }
        _ => (rest, LineStyle::Dashed),
    };
    let up = body.to_ascii_uppercase();
    let edge = match up.as_str() {
        "LEFT" => Some(Edge::Left),
        "RIGHT" => Some(Edge::Right),
        "TOP" => Some(Edge::Top),
        "BOTTOM" => Some(Edge::Bottom),
        _ => None,
    };
    let two = || -> Result<((f32, f32), (f32, f32))> {
        let mid = body.find(")(").ok_or_else(|| anyhow::anyhow!("esperado (x, y)(x, y)"))?;
        Ok((parse_pair(&body[..=mid])?, parse_pair(&body[mid + 1..])?))
    };
    match kind.as_str() {
        "LINE" => match edge {
            Some(e) => Ok(Marker::Edge(e, style)),
            None => {
                let (a, b) = two()?;
                Ok(Marker::Line { a, b, style })
            }
        },
        "RECT" => {
            let (pos, size) = two()?;
            Ok(Marker::Rect { pos, size, style })
        }
        o => bail!("tipo desconhecido: {o} (use LINE ou RECT)"),
    }
}

pub fn parse_markers(text: &str) -> Result<Vec<Marker>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| parse_marker(l).map_err(|e| format!("linha {}: {e}", i + 1)))
        .collect()
}

// ---------- persistência ----------

pub fn formats_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("undercovernc").join("formats.json"))
}

pub fn load_formats() -> Vec<CoverFormat> {
    formats_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Vec<CoverFormat>>(&b).ok())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(default_formats)
}

pub fn save_formats(f: &[CoverFormat]) -> Result<()> {
    let p = formats_path().ok_or_else(|| anyhow::anyhow!("sem pasta de configuração"))?;
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_vec_pretty(f)?)?;
    Ok(())
}
