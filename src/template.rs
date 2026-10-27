//! Modelo em branco de um formato: PNG no tamanho exato, com guias para desenhar a capa.

use crate::format::{CoverFormat, Marker};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use image::{Rgb, RgbImage, imageops};

const FOLD: Rgb<u8> = Rgb([0, 170, 220]);
const SAFE: Rgb<u8> = Rgb([230, 60, 160]);
const EDGE: Rgb<u8> = Rgb([150, 150, 150]);
const SPINE: Rgb<u8> = Rgb([236, 240, 246]);
/// Margem de segurança: texto importante deve ficar para dentro dela.
const SAFE_MM: f32 = 3.0;

/// Linhas de dobra (vertical?, posição em mm): lombada do tipo interno e marcadores
/// que ficam do lado de fora da capa apontando para ela (ex.: tracinhos de dobra).
pub fn folds(fmt: &CoverFormat) -> Vec<(bool, f32)> {
    let (w, h) = (fmt.width, fmt.height);
    let mut out = Vec::new();
    if let Some((s0, s)) = fmt.spine_band() {
        let vertical = w > h;
        out.push((vertical, s0));
        out.push((vertical, s0 + s));
    }
    for m in &fmt.markers {
        if let Marker::Line { a, b, .. } = *m {
            let outside_x = |x: f32| x <= 0.0 || x >= w;
            let outside_y = |y: f32| y <= 0.0 || y >= h;
            if (a.1 - b.1).abs() < 0.01 && a.1 > 0.0 && a.1 < h && outside_x(a.0) && outside_x(b.0) {
                out.push((false, a.1));
            } else if (a.0 - b.0).abs() < 0.01 && a.0 > 0.0 && a.0 < w && outside_y(a.1) && outside_y(b.1) {
                out.push((true, a.0));
            }
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out.dedup_by(|a, b| a.0 == b.0 && (a.1 - b.1).abs() < 0.05);
    out
}

/// Modelo para desenhar. Capas com lombada no meio saem deitadas (verso | lombada | frente),
/// do jeito que se desenha; ao abrir a arte pronta, a orientação automática acerta tudo.
pub fn render(fmt: &CoverFormat, dpi: f32) -> RgbImage {
    let img = render_upright(fmt, dpi);
    let f = folds(fmt);
    let horizontal: Vec<f32> = f.iter().filter(|x| !x.0).map(|x| x.1).collect();
    if fmt.height > fmt.width && horizontal.len() == 2 && f.len() == 2 {
        let (a, b) = (horizontal[0], horizontal[1]);
        let mut img = imageops::rotate90(&img);
        // Depois de girar: esquerda = verso (embaixo), meio = lombada, direita = frente (em cima).
        let px = dpi / 25.4;
        let h = fmt.height;
        if b - a < a.min(h - b) {
            label(&mut img, "VERSO", (h - b) / 2.0 * px, (h - b) * px, px);
            label(&mut img, "LOMBADA", (h - (a + b) / 2.0) * px, (b - a) * px, px);
            label(&mut img, "FRENTE", (h - a / 2.0) * px, a * px, px);
        }
        return img;
    }
    img
}

/// Escreve um rótulo cinza-claro centralizado em `cx` (px), no meio da altura.
fn label(img: &mut RgbImage, text: &str, cx: f32, max_w: f32, px: f32) {
    let Ok(font) = FontRef::try_from_slice(include_bytes!("../assets/VT323-Regular.ttf")) else { return };
    let mut size = (px * 9.0).max(12.0);
    let advance = |size: f32| -> f32 {
        let sf = font.as_scaled(PxScale::from(size));
        text.chars().map(|c| sf.h_advance(sf.glyph_id(c))).sum()
    };
    // Lombada estreita: o texto não pode passar dela, então diminui.
    let w0 = advance(size);
    if w0 > max_w * 0.8 {
        size *= max_w * 0.8 / w0;
    }
    let sf = font.as_scaled(PxScale::from(size));
    let width = advance(size);
    let (iw, ih) = img.dimensions();
    let mut x = cx - width / 2.0;
    let y = ih as f32 / 2.0 + sf.ascent() / 2.0 - sf.descent() / 2.0;
    for c in text.chars() {
        let g = sf.scaled_glyph(c);
        let adv = sf.h_advance(g.id);
        let g = ab_glyph::Glyph { position: point(x, y), ..g };
        if let Some(o) = font.outline_glyph(g) {
            let bb = o.px_bounds();
            o.draw(|gx, gy, cov| {
                let (xx, yy) = (bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32);
                if xx >= 0 && yy >= 0 && (xx as u32) < iw && (yy as u32) < ih && cov > 0.1 {
                    let p = img.get_pixel_mut(xx as u32, yy as u32);
                    let v = (255.0 - cov * 55.0) as u8;
                    *p = Rgb([p[0].min(v), p[1].min(v), p[2].min(v)]);
                }
            });
        }
        x += adv;
    }
}

fn render_upright(fmt: &CoverFormat, dpi: f32) -> RgbImage {
    let px = dpi / 25.4;
    let (w, h) = (((fmt.width * px).round() as u32).max(1), ((fmt.height * px).round() as u32).max(1));
    let mut img = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
    let thick = (px * 0.3).round().max(1.0) as u32;
    let dash = (px * 3.0) as u32;

    if let Some((s0, s)) = fmt.spine_band() {
        let (a, b) = ((s0 * px) as u32, ((s0 + s) * px) as u32);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let at = if fmt.width > fmt.height { x } else { y };
            if at >= a && at < b {
                *p = SPINE;
            }
        }
    }
    let hline = |img: &mut RgbImage, y: f32, x0: f32, x1: f32, c: Rgb<u8>, dashed: bool| {
        let y = (y * px) as i64;
        for x in (x0 * px) as u32..((x1 * px) as u32).min(w) {
            if dashed && (x / dash.max(1)) % 2 == 1 {
                continue;
            }
            for t in 0..thick as i64 {
                let yy = y + t - thick as i64 / 2;
                if yy >= 0 && (yy as u32) < h {
                    img.put_pixel(x, yy as u32, c);
                }
            }
        }
    };
    let vline = |img: &mut RgbImage, x: f32, y0: f32, y1: f32, c: Rgb<u8>, dashed: bool| {
        let x = (x * px) as i64;
        for y in (y0 * px) as u32..((y1 * px) as u32).min(h) {
            if dashed && (y / dash.max(1)) % 2 == 1 {
                continue;
            }
            for t in 0..thick as i64 {
                let xx = x + t - thick as i64 / 2;
                if xx >= 0 && (xx as u32) < w {
                    img.put_pixel(xx as u32, y, c);
                }
            }
        }
    };
    let (fw, fh) = (fmt.width, fmt.height);
    if fmt.label {
        let (cx, cy) = (fw / 2.0, fh / 2.0);
        let r = fw.min(fh) / 2.0;
        for (x, y, p) in img.enumerate_pixels_mut() {
            let d = ((x as f32 / px - cx).powi(2) + (y as f32 / px - cy).powi(2)).sqrt();
            let near = |rr: f32| (d - rr).abs() < 0.25;
            if near(r) || near(fmt.hole / 2.0) {
                *p = EDGE;
            } else if near(r - SAFE_MM) || near(fmt.hole / 2.0 + SAFE_MM) {
                *p = SAFE;
            }
        }
        return img;
    }
    // Margem de segurança em cada painel (entre dobras).
    let folds = folds(fmt);
    let cuts = |vertical: bool, len: f32| {
        let mut c = vec![0.0];
        c.extend(folds.iter().filter(|f| f.0 == vertical).map(|f| f.1));
        c.push(len);
        c
    };
    let (xs, ys) = (cuts(true, fw), cuts(false, fh));
    for xw in xs.windows(2) {
        for yw in ys.windows(2) {
            let (x0, x1, y0, y1) = (xw[0] + SAFE_MM, xw[1] - SAFE_MM, yw[0] + SAFE_MM, yw[1] - SAFE_MM);
            if x1 > x0 && y1 > y0 {
                hline(&mut img, y0, x0, x1, SAFE, true);
                hline(&mut img, y1, x0, x1, SAFE, true);
                vline(&mut img, x0, y0, y1, SAFE, true);
                vline(&mut img, x1, y0, y1, SAFE, true);
            }
        }
    }
    for &(vertical, at) in &folds {
        if vertical {
            vline(&mut img, at, 0.0, fh, FOLD, true);
        } else {
            hline(&mut img, at, 0.0, fw, FOLD, true);
        }
    }
    let e = 0.5 / px;
    hline(&mut img, e, 0.0, fw, EDGE, false);
    hline(&mut img, fh - e, 0.0, fw, EDGE, false);
    vline(&mut img, e, 0.0, fh, EDGE, false);
    vline(&mut img, fw - e, 0.0, fh, EDGE, false);
    img
}
