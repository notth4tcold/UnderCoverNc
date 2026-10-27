//! Desenha uma página (mesmas primitivas do PDF) na tela.

use crate::layout::{DrawOpts, Page, Prim, page_prims};
use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, TextureId, epaint::Vertex, pos2, vec2};
use std::f32::consts::TAU;

const INK: Color32 = Color32::from_rgb(90, 90, 100);

/// Retângulo da página na tela, centrada em `area`, e o fator mm → pontos.
pub fn page_rect(area: Rect, size: (f32, f32)) -> (Rect, f32) {
    let s = (area.width() / size.0).min(area.height() / size.1);
    (Rect::from_center_size(area.center(), vec2(size.0 * s, size.1 * s)), s)
}

pub fn paint_page(painter: &Painter, area: Rect, page: &Page, opts: &DrawOpts, tex: impl Fn(usize) -> Option<TextureId>, sheet: bool) -> Rect {
    let (r, s) = page_rect(area, page.size);
    let at = |(x, y): (f32, f32)| pos2(r.left() + x * s, r.top() + y * s);
    if sheet {
        painter.rect_filled(r.translate(vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(90));
        painter.rect_filled(r, 0.0, Color32::WHITE);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_gray(60)), StrokeKind::Outside);
    }
    let painter = painter.with_clip_rect(r.expand(1.0));
    for it in &page.items {
        if it.prepared.is_none() {
            let painter = match &it.tile {
                Some(t) => painter.with_clip_rect(Rect::from_min_size(at((t.clip.0, t.clip.1)), vec2(t.clip.2 * s, t.clip.3 * s)).intersect(painter.clip_rect())),
                None => painter.clone(),
            };
            let rr = Rect::from_min_size(at((it.x, it.y)), vec2(it.fmt.width * s, it.fmt.height * s));
            if it.fmt.label {
                painter.circle_filled(rr.center(), rr.width().min(rr.height()) / 2.0, Color32::from_rgb(214, 226, 240));
                painter.circle_filled(rr.center(), it.fmt.hole / 2.0 * s, Color32::WHITE);
            } else {
                painter.rect_filled(rr, 0.0, Color32::from_rgb(214, 226, 240));
            }
        }
    }
    let base = painter.clone();
    for (clip, prim) in page_prims(page, opts) {
        let painter = match clip {
            Some((cx, cy, cw, ch)) => base.with_clip_rect(Rect::from_min_size(at((cx, cy)), vec2(cw * s, ch * s)).intersect(base.clip_rect())),
            None => base.clone(),
        };
        let line = |a: Pos2, b: Pos2, dashed: bool| {
            let stroke = Stroke::new(1.0_f32, INK);
            if dashed {
                painter.extend(Shape::dashed_line(&[a, b], stroke, 4.0, 3.0));
            } else {
                painter.line_segment([a, b], stroke);
            }
        };
        match prim {
            Prim::Image { item, rect, clip } => {
                let Some(id) = tex(item) else { continue };
                let ir = Rect::from_min_size(at((rect.0, rect.1)), vec2(rect.2 * s, rect.3 * s));
                match clip {
                    None => {
                        painter.image(id, ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    Some((c, ro, hole)) => {
                        painter.add(annulus(id, ir, at(c), ro * s, hole * s));
                    }
                }
            }
            Prim::Line { a, b, dashed } => line(at(a), at(b), dashed),
            Prim::Rect { rect, dashed } => {
                let a = at((rect.0, rect.1));
                let b = at((rect.0 + rect.2, rect.1 + rect.3));
                line(a, pos2(b.x, a.y), dashed);
                line(pos2(b.x, a.y), b, dashed);
                line(b, pos2(a.x, b.y), dashed);
                line(pos2(a.x, b.y), a, dashed);
            }
            Prim::Circle { c, r: rad, dashed } => {
                let c = at(c);
                let n = 96;
                let pts: Vec<Pos2> = (0..=n).map(|i| c + vec2(1.0, 0.0).rot(i as f32 / n as f32 * TAU) * rad * s).collect();
                let stroke = Stroke::new(1.0_f32, INK);
                if dashed {
                    painter.extend(Shape::dashed_line(&pts, stroke, 4.0, 3.0));
                } else {
                    painter.add(Shape::line(pts, stroke));
                }
            }
        }
    }
    r
}

trait Rot {
    fn rot(self, a: f32) -> Self;
}

impl Rot for eframe::egui::Vec2 {
    fn rot(self, a: f32) -> Self {
        let (s, c) = a.sin_cos();
        vec2(self.x * c - self.y * s, self.x * s + self.y * c)
    }
}

/// Anel texturizado (rótulo de CD com furo no meio).
fn annulus(tex: TextureId, img: Rect, c: Pos2, outer: f32, inner: f32) -> Shape {
    let mut mesh = Mesh::with_texture(tex);
    let n = 96u32;
    let uv = |p: Pos2| pos2((p.x - img.left()) / img.width(), (p.y - img.top()) / img.height());
    for i in 0..=n {
        let d = vec2(1.0, 0.0).rot(i as f32 / n as f32 * TAU);
        for r in [outer, inner] {
            let p = c + d * r;
            mesh.vertices.push(Vertex { pos: p, uv: uv(p), color: Color32::WHITE });
        }
    }
    for i in 0..n {
        let k = i * 2;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    Shape::mesh(mesh)
}
