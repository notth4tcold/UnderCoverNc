//! Preparação das imagens e montagem das páginas. Gera uma lista de primitivas
//! em mm que tanto o PDF quanto a pré-visualização desenham igualzinho.

use crate::format::{CoverFormat, DVD_SPINE_MM, Edge, FitMode, InternalType, LineStyle, Marker};
use image::{RgbImage, imageops};
use std::borrow::Cow;

/// Altura de referência de uma capa de DVD inteira (para achar a lombada na imagem).
const DVD_FULL_MM: f32 = 274.3;
const GAP_MM: f32 = 8.0;
/// Sobreposição entre as partes de uma capa dividida em várias folhas (área de cola).
pub const OVERLAP_MM: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub struct PageSetup {
    pub paper: (f32, f32),
    pub margin: f32,
    pub correct_area: bool,
    /// Divide capas maiores que o papel em várias folhas.
    pub tile: bool,
}

/// Imagem já girada/remapeada/recortada e o retângulo (em mm, relativo à capa) onde vai.
#[derive(Clone)]
pub struct Prepared {
    pub img: RgbImage,
    pub rect: (f32, f32, f32, f32),
}

pub struct Placed<'a> {
    pub side: Side,
    /// Formato e imagem como vão na folha (giradas 90° quando a divisão em folhas economiza papel).
    pub fmt: Cow<'a, CoverFormat>,
    pub prepared: Option<Cow<'a, Prepared>>,
    pub x: f32,
    pub y: f32,
    pub tile: Option<Tile>,
}

/// Pedaço de uma capa grande: só a área `clip` (mm na página) aparece nesta folha.
#[derive(Clone, Debug)]
pub struct Tile {
    pub clip: (f32, f32, f32, f32),
    /// Linhas de emenda (vertical?, posição em mm na capa).
    pub seams: Vec<(bool, f32)>,
}

pub struct Page<'a> {
    pub size: (f32, f32),
    pub items: Vec<Placed<'a>>,
}

#[derive(Clone, Debug)]
pub enum Prim {
    Image { item: usize, rect: (f32, f32, f32, f32), clip: Option<((f32, f32), f32, f32)> },
    Line { a: (f32, f32), b: (f32, f32), dashed: bool },
    Rect { rect: (f32, f32, f32, f32), dashed: bool },
    Circle { c: (f32, f32), r: f32, dashed: bool },
}

// ---------- imagem ----------

/// Gira a imagem em quartos de volta no sentido horário.
pub fn rotate(img: &RgbImage, quarter_turns: u8) -> RgbImage {
    match quarter_turns % 4 {
        1 => imageops::rotate90(img),
        2 => imageops::rotate180(img),
        3 => imageops::rotate270(img),
        _ => img.clone(),
    }
}

/// Reduz imagens enormes para algo razoável (lado maior `max`).
pub fn limit_size(img: RgbImage, max: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    if w.max(h) <= max {
        return img;
    }
    let s = max as f32 / w.max(h) as f32;
    imageops::resize(&img, ((w as f32 * s) as u32).max(1), ((h as f32 * s) as u32).max(1), imageops::FilterType::Triangle)
}

fn is_square(w: f32, h: f32) -> bool {
    (w / h - 1.0).abs() < 0.03
}

pub fn prepare(src: &RgbImage, fmt: &CoverFormat, quarter_turns: u8, auto_orient: bool) -> Prepared {
    let mut img = rotate(src, quarter_turns);
    let (iw, ih) = img.dimensions();
    if auto_orient && !is_square(fmt.width, fmt.height) && !is_square(iw as f32, ih as f32) && (iw > ih) != (fmt.width > fmt.height) {
        // Capas deitadas (verso | lombada | frente) ficam de pé com a frente para cima.
        img = imageops::rotate270(&img);
    }
    if fmt.kind == InternalType::SlimlineDvd {
        if let Some(spine) = fmt.alt_spine {
            img = remap_spine(&img, fmt.width > fmt.height, fmt.width.max(fmt.height), spine);
        }
    }
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let (fw, fh) = (fmt.width, fmt.height);
    match fmt.fit {
        FitMode::Stretch => Prepared { img, rect: (0.0, 0.0, fw, fh) },
        FitMode::Fit => {
            let s = (fw / iw).min(fh / ih);
            let (w, h) = (iw * s, ih * s);
            Prepared { img, rect: ((fw - w) / 2.0, (fh - h) / 2.0, w, h) }
        }
        FitMode::Fill => {
            let s = (fw / iw).max(fh / ih);
            let (cw, ch) = ((fw / s).round() as u32, (fh / s).round() as u32);
            let (cw, ch) = (cw.clamp(1, iw as u32), ch.clamp(1, ih as u32));
            let x = (iw as u32 - cw) / 2;
            let y = (ih as u32 - ch) / 2;
            let img = imageops::crop_imm(&img, x, y, cw, ch).to_image();
            Prepared { img, rect: (0.0, 0.0, fw, fh) }
        }
    }
}

/// Trata a imagem como capa de DVD (lombada de 14 mm no meio do lado maior) e
/// redistribui: as duas metades vão para (comprimento - lombada)/2 e a lombada para `spine`.
fn remap_spine(img: &RgbImage, horizontal: bool, target_len: f32, spine: f32) -> RgbImage {
    let img = if horizontal { imageops::rotate90(img) } else { img.clone() };
    let (w, h) = img.dimensions();
    let px_per_mm = h as f32 / DVD_FULL_MM;
    let src_spine = (DVD_SPINE_MM * px_per_mm).round().max(1.0) as u32;
    let src_half = (h.saturating_sub(src_spine)) / 2;
    let dst_half = (((target_len - spine) / 2.0) * px_per_mm).round().max(1.0) as u32;
    let dst_spine = (spine * px_per_mm).round().max(1.0) as u32;
    let filter = imageops::FilterType::CatmullRom;
    let piece = |y: u32, hh: u32, nh: u32| {
        let c = imageops::crop_imm(&img, 0, y, w, hh.max(1)).to_image();
        imageops::resize(&c, w, nh, filter)
    };
    let top = piece(0, src_half, dst_half);
    let mid = piece(src_half, src_spine, dst_spine);
    let bot = piece(src_half + src_spine, h - src_half - src_spine, dst_half);
    let mut out = RgbImage::new(w, dst_half * 2 + dst_spine);
    imageops::replace(&mut out, &top, 0, 0);
    imageops::replace(&mut out, &mid, 0, dst_half as i64);
    imageops::replace(&mut out, &bot, 0, (dst_half + dst_spine) as i64);
    if horizontal { imageops::rotate270(&out) } else { out }
}

/// Escolhe o formato (entre os com detecção automática) com proporção mais parecida.
pub fn detect_format(w: u32, h: u32, formats: &[CoverFormat], auto_orient: bool) -> Option<usize> {
    let ratio = |a: f32, b: f32| if auto_orient { a.max(b) / a.min(b) } else { a / b };
    let r = ratio(w as f32, h as f32);
    formats
        .iter()
        .enumerate()
        .filter(|(_, f)| f.autodetect)
        .map(|(i, f)| (i, (r / ratio(f.width, f.height)).ln().abs()))
        .filter(|&(_, d)| d < 0.06)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

// ---------- páginas ----------

fn paper_of(fmt: &CoverFormat, setup: &PageSetup) -> (f32, f32) {
    fmt.paper.unwrap_or(setup.paper)
}

fn manual_pos(fmt: &CoverFormat, side: Side) -> Option<(f32, f32)> {
    match side {
        Side::Left => fmt.left_pos,
        Side::Right => fmt.right_pos,
    }
}

fn clamp_into(x: f32, y: f32, fmt: &CoverFormat, paper: (f32, f32), m: f32) -> (f32, f32) {
    let fit = |p: f32, size: f32, total: f32| {
        if size > total - 2.0 * m {
            (total - size) / 2.0
        } else {
            p.clamp(m, total - m - size)
        }
    };
    (fit(x, fmt.width, paper.0), fit(y, fmt.height, paper.1))
}

fn center(fmt: &CoverFormat, paper: (f32, f32)) -> (f32, f32) {
    ((paper.0 - fmt.width) / 2.0, (paper.1 - fmt.height) / 2.0)
}

fn fixup(pos: (f32, f32), fmt: &CoverFormat, paper: (f32, f32), setup: &PageSetup) -> (f32, f32) {
    if setup.correct_area { clamp_into(pos.0, pos.1, fmt, paper, setup.margin) } else { pos }
}

/// Monta as páginas. Retorna também avisos (capa maior que o papel, não coube junto...).
pub fn plan<'a>(
    covers: &[(Side, &'a CoverFormat, Option<&'a Prepared>)],
    single_page: bool,
    setup: &PageSetup,
) -> (Vec<Page<'a>>, Vec<String>) {
    let mut warn = Vec::new();
    for (_, f, _) in covers {
        let p = paper_of(f, setup);
        let fits = |a: f32, b: f32| a <= p.0 - 2.0 * setup.margin + 0.01 && b <= p.1 - 2.0 * setup.margin + 0.01;
        if !fits(f.width, f.height)
            && !setup.tile {
                warn.push(format!("\"{}\" ({:.1} × {:.1} mm) é maior que a área imprimível do papel.", f.name, f.width, f.height));
            }
    }
    if single_page && covers.len() == 2 {
        let (a, b) = (covers[0], covers[1]);
        let (pa, pb) = (paper_of(a.1, setup), paper_of(b.1, setup));
        if pa == pb {
            if let Some(page) = pack_two(a, b, pa, setup) {
                return (vec![page], warn);
            }
        }
        warn.push("As duas capas não cabem numa única folha; vão em páginas separadas.".into());
    }
    let mut pages = Vec::new();
    for &(side, fmt, prepared) in covers {
        let paper = paper_of(fmt, setup);
        let m = setup.margin;
        let (uw, uh) = (paper.0 - 2.0 * m, paper.1 - 2.0 * m);
        let count = |w: f32, h: f32| tiles(w, uw).len() * tiles(h, uh).len();
        if setup.tile && count(fmt.width, fmt.height) > 1 {
            // Deitada pode precisar de menos folhas (ex.: clamshell VHS: 4 → 2).
            let (fmt, prepared): (Cow<CoverFormat>, Option<Cow<Prepared>>) = if count(fmt.height, fmt.width) < count(fmt.width, fmt.height) {
                (Cow::Owned(rotate_format(fmt)), prepared.map(|p| Cow::Owned(rotate_prepared(p, fmt.height))))
            } else {
                (Cow::Borrowed(fmt), prepared.map(Cow::Borrowed))
            };
            let (tx, ty) = (tiles(fmt.width, uw), tiles(fmt.height, uh));
            warn.push(format!(
                "\"{}\" ({:.0} × {:.0} mm) foi dividida em {} folhas: corte uma na linha contínua de emenda e cole por cima da outra.",
                fmt.name,
                fmt.width.max(fmt.height),
                fmt.width.min(fmt.height),
                tx.len() * ty.len()
            ));
            let seams = |t: &[(f32, f32)]| t.iter().skip(1).map(|&(s, _)| s + OVERLAP_MM / 2.0).collect::<Vec<_>>();
            let (sx_all, sy_all) = (seams(&tx), seams(&ty));
            for &(sy, th) in &ty {
                for &(sx, tw) in &tx {
                    let x = (paper.0 - tw) / 2.0 - sx;
                    let y = (paper.1 - th) / 2.0 - sy;
                    let l = if sx <= 0.01 { m } else { x + sx };
                    let r = if sx + tw >= fmt.width - 0.01 { paper.0 - m } else { x + sx + tw };
                    let t = if sy <= 0.01 { m } else { y + sy };
                    let b = if sy + th >= fmt.height - 0.01 { paper.1 - m } else { y + sy + th };
                    let mut seams: Vec<(bool, f32)> = sx_all.iter().filter(|&&s| s > sx && s < sx + tw).map(|&s| (true, s)).collect();
                    seams.extend(sy_all.iter().filter(|&&s| s > sy && s < sy + th).map(|&s| (false, s)));
                    let tile = Tile { clip: (l, t, r - l, b - t), seams };
                    pages.push(Page { size: paper, items: vec![Placed { side, fmt: fmt.clone(), prepared: prepared.clone(), x, y, tile: Some(tile) }] });
                }
            }
            continue;
        }
        let pos = fixup(manual_pos(fmt, side).unwrap_or_else(|| center(fmt, paper)), fmt, paper, setup);
        pages.push(Page { size: paper, items: vec![Placed { side, fmt: Cow::Borrowed(fmt), prepared: prepared.map(Cow::Borrowed), x: pos.0, y: pos.1, tile: None }] });
    }
    (pages, warn)
}

/// Gira um formato 90° no sentido horário: (u, v) → (altura - v, u).
fn rotate_format(f: &CoverFormat) -> CoverFormat {
    let h = f.height;
    let pt = |(u, v): (f32, f32)| (h - v, u);
    let mut r = f.clone();
    r.width = f.height;
    r.height = f.width;
    r.left_pos = None;
    r.right_pos = None;
    r.markers = f
        .markers
        .iter()
        .map(|m| match *m {
            Marker::Edge(e, s) => Marker::Edge(
                match e {
                    Edge::Left => Edge::Top,
                    Edge::Top => Edge::Right,
                    Edge::Right => Edge::Bottom,
                    Edge::Bottom => Edge::Left,
                },
                s,
            ),
            Marker::Line { a, b, style } => Marker::Line { a: pt(a), b: pt(b), style },
            Marker::Rect { pos, size, style } => Marker::Rect { pos: (h - pos.1 - size.1, pos.0), size: (size.1, size.0), style },
        })
        .collect();
    r
}

fn rotate_prepared(p: &Prepared, fmt_height: f32) -> Prepared {
    let (x, y, w, h) = p.rect;
    Prepared { img: imageops::rotate90(&p.img), rect: (fmt_height - y - h, x, h, w) }
}

/// Pedaços (início, tamanho) iguais que cobrem `len` cabendo em `usable`, com sobreposição.
fn tiles(len: f32, usable: f32) -> Vec<(f32, f32)> {
    if len <= usable + 0.01 || usable <= OVERLAP_MM * 2.0 {
        return vec![(0.0, len)];
    }
    let n = ((len - OVERLAP_MM) / (usable - OVERLAP_MM)).ceil().max(1.0) as usize;
    let size = (len + (n - 1) as f32 * OVERLAP_MM) / n as f32;
    (0..n).map(|i| (i as f32 * (size - OVERLAP_MM), size)).collect()
}

type Cover<'a> = (Side, &'a CoverFormat, Option<&'a Prepared>);

fn pack_two<'a>(a: Cover<'a>, b: Cover<'a>, paper: (f32, f32), setup: &PageSetup) -> Option<Page<'a>> {
    let m = setup.margin;
    let (aw, ah, bw, bh) = (a.1.width, a.1.height, b.1.width, b.1.height);
    let (uw, uh) = (paper.0 - 2.0 * m, paper.1 - 2.0 * m);
    let mk = |pa: (f32, f32), pb: (f32, f32)| {
        let pa = fixup(manual_pos(a.1, a.0).unwrap_or(pa), a.1, paper, setup);
        let pb = fixup(manual_pos(b.1, b.0).unwrap_or(pb), b.1, paper, setup);
        Page {
            size: paper,
            items: vec![
                Placed { side: a.0, fmt: Cow::Borrowed(a.1), prepared: a.2.map(Cow::Borrowed), x: pa.0, y: pa.1, tile: None },
                Placed { side: b.0, fmt: Cow::Borrowed(b.1), prepared: b.2.map(Cow::Borrowed), x: pb.0, y: pb.1, tile: None },
            ],
        }
    };
    // Empilhadas (uma em cima da outra), centralizadas.
    if aw.max(bw) <= uw && ah + bh + GAP_MM <= uh {
        let gap = GAP_MM.max(((uh - ah - bh) / 3.0).min(20.0));
        let top = (paper.1 - ah - bh - gap) / 2.0;
        return Some(mk(((paper.0 - aw) / 2.0, top), ((paper.0 - bw) / 2.0, top + ah + gap)));
    }
    // Lado a lado.
    if aw + bw + GAP_MM <= uw && ah.max(bh) <= uh {
        let gap = GAP_MM.max(((uw - aw - bw) / 3.0).min(20.0));
        let left = (paper.0 - aw - bw - gap) / 2.0;
        return Some(mk((left, (paper.1 - ah) / 2.0), (left + aw + gap, (paper.1 - bh) / 2.0)));
    }
    None
}

// ---------- primitivas ----------

pub struct DrawOpts {
    pub cut_lines: bool,
    pub border: bool,
    pub margin: f32,
}

/// Remove do segmento [a, b] os intervalos ocupados por capas.
fn subtract(a: f32, b: f32, holes: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut segs = vec![(a.min(b), a.max(b))];
    for &(h0, h1) in holes {
        segs = segs
            .into_iter()
            .flat_map(|(s0, s1)| {
                if h1 <= s0 || h0 >= s1 {
                    vec![(s0, s1)]
                } else {
                    let mut v = vec![];
                    if h0 > s0 {
                        v.push((s0, h0));
                    }
                    if h1 < s1 {
                        v.push((h1, s1));
                    }
                    v
                }
            })
            .collect();
    }
    segs.into_iter().filter(|(s0, s1)| s1 - s0 > 0.5).collect()
}

/// Primitivas da página, cada uma com o recorte (mm na página) que deve ser aplicado.
pub type Clip = Option<(f32, f32, f32, f32)>;

pub fn page_prims(page: &Page, opts: &DrawOpts) -> Vec<(Clip, Prim)> {
    let mut spans: Vec<(usize, Clip)> = Vec::new();
    let mut out = Vec::new();
    let rects: Vec<(f32, f32, f32, f32)> = page.items.iter().map(|p| (p.x, p.y, p.fmt.width, p.fmt.height)).collect();
    let (pw, ph) = page.size;
    let m = opts.margin;
    for (i, it) in page.items.iter().enumerate() {
        spans.push((out.len(), it.tile.as_ref().map(|t| t.clip)));
        let f = &*it.fmt;
        let (x, y, w, h) = (it.x, it.y, f.width, f.height);
        let label = f.label.then(|| ((x + w / 2.0, y + h / 2.0), w.min(h) / 2.0, f.hole / 2.0));
        if let Some(p) = &it.prepared {
            let r = p.rect;
            out.push(Prim::Image { item: i, rect: (x + r.0, y + r.1, r.2, r.3), clip: label });
        }
        if opts.border {
            match label {
                Some((c, r, hole)) => {
                    out.push(Prim::Circle { c, r, dashed: false });
                    out.push(Prim::Circle { c, r: hole, dashed: false });
                }
                None => out.push(Prim::Rect { rect: (x, y, w, h), dashed: false }),
            }
        }
        // Emendas de capa dividida: linha contínua atravessando a capa + marquinhas fora.
        if let Some(t) = &it.tile {
            for &(vertical, at) in &t.seams {
                if vertical {
                    out.push(Prim::Line { a: (x + at, y - 9.0), b: (x + at, y + h + 9.0), dashed: false });
                } else {
                    out.push(Prim::Line { a: (x - 9.0, y + at), b: (x + w + 9.0, y + at), dashed: false });
                }
            }
        }
        if !opts.cut_lines {
            continue;
        }
        if let Some((c, r, hole)) = label {
            out.push(Prim::Circle { c, r, dashed: true });
            out.push(Prim::Circle { c, r: hole, dashed: true });
        }
        let others: Vec<_> = rects.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, r)| *r).collect();
        for mk in &f.markers {
            match *mk {
                Marker::Edge(e, s) => {
                    let dashed = s == LineStyle::Dashed;
                    let vertical = matches!(e, Edge::Left | Edge::Right);
                    let pos = match e {
                        Edge::Left => x,
                        Edge::Right => x + w,
                        Edge::Top => y,
                        Edge::Bottom => y + h,
                    };
                    // Linha ao longo da borda, do limite da margem até o outro, sem passar por cima das capas.
                    let mut holes: Vec<(f32, f32)> = vec![if vertical { (y, y + h) } else { (x, x + w) }];
                    for o in &others {
                        let inside = if vertical { pos >= o.0 - 0.01 && pos <= o.0 + o.2 + 0.01 } else { pos >= o.1 - 0.01 && pos <= o.1 + o.3 + 0.01 };
                        if inside {
                            holes.push(if vertical { (o.1, o.1 + o.3) } else { (o.0, o.0 + o.2) });
                        }
                    }
                    let (lo, hi) = if vertical { (m, ph - m) } else { (m, pw - m) };
                    for (s0, s1) in subtract(lo, hi, &holes) {
                        let (a, b) = if vertical { ((pos, s0), (pos, s1)) } else { ((s0, pos), (s1, pos)) };
                        out.push(Prim::Line { a, b, dashed });
                    }
                }
                Marker::Line { a, b, style } => out.push(Prim::Line {
                    a: (x + a.0, y + a.1),
                    b: (x + b.0, y + b.1),
                    dashed: style == LineStyle::Dashed,
                }),
                Marker::Rect { pos, size, style } => out.push(Prim::Rect {
                    rect: (x + pos.0, y + pos.1, size.0, size.1),
                    dashed: style == LineStyle::Dashed,
                }),
            }
        }
        // Marcas de dobra da lombada (DVD e afins), do lado de fora da capa.
        if let Some((s0, s)) = f.spine_band() {
            for at in [s0, s0 + s] {
                if h >= w {
                    out.push(Prim::Line { a: (x - 9.0, y + at), b: (x - 2.0, y + at), dashed: false });
                    out.push(Prim::Line { a: (x + w + 2.0, y + at), b: (x + w + 9.0, y + at), dashed: false });
                } else {
                    out.push(Prim::Line { a: (x + at, y - 9.0), b: (x + at, y - 2.0), dashed: false });
                    out.push(Prim::Line { a: (x + at, y + h + 2.0), b: (x + at, y + h + 9.0), dashed: false });
                }
            }
        }
    }
    out.into_iter()
        .enumerate()
        .map(|(k, p)| (spans.iter().rev().find(|(s, _)| *s <= k).and_then(|(_, c)| *c), p))
        .collect()
}
