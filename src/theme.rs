//! Skins retrô, widgets com relevo (estilo Win9x) e o mascote em pixel art.

use crate::settings::Skin;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, pos2, vec2,
};
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
pub struct Palette {
    pub desktop: Color32,
    pub face: Color32,
    pub light: Color32,
    pub shadow: Color32,
    pub dark: Color32,
    pub text: Color32,
    pub disabled: Color32,
    pub title_a: Color32,
    pub title_b: Color32,
    pub title_text: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub field: Color32,
    pub field_text: Color32,
    pub highlight: Color32,
    pub crt: bool,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn palette(skin: Skin) -> Palette {
    match skin {
        Skin::Classic | Skin::Random => Palette {
            desktop: rgb(0x008080),
            face: rgb(0xc0c0c0),
            light: rgb(0xffffff),
            shadow: rgb(0x808080),
            dark: rgb(0x000000),
            text: rgb(0x000000),
            disabled: rgb(0x808080),
            title_a: rgb(0x000080),
            title_b: rgb(0x1084d0),
            title_text: rgb(0xffffff),
            accent: rgb(0x000080),
            accent_text: rgb(0xffffff),
            field: rgb(0xffffff),
            field_text: rgb(0x000000),
            highlight: rgb(0xffd700),
            crt: false,
        },
        Skin::Phosphor => Palette {
            desktop: rgb(0x000d04),
            face: rgb(0x03180b),
            light: rgb(0x3dff7a),
            shadow: rgb(0x0f5c28),
            dark: rgb(0x000000),
            text: rgb(0x3dff7a),
            disabled: rgb(0x1d7a3c),
            title_a: rgb(0x0f5c28),
            title_b: rgb(0x03180b),
            title_text: rgb(0xb8ffcf),
            accent: rgb(0x3dff7a),
            accent_text: rgb(0x000d04),
            field: rgb(0x000a03),
            field_text: rgb(0x3dff7a),
            highlight: rgb(0x3dff7a),
            crt: true,
        },
        Skin::Amber => Palette {
            desktop: rgb(0x0d0700),
            face: rgb(0x1c1000),
            light: rgb(0xffb000),
            shadow: rgb(0x6b4a00),
            dark: rgb(0x000000),
            text: rgb(0xffb000),
            disabled: rgb(0x7a5500),
            title_a: rgb(0x6b4a00),
            title_b: rgb(0x1c1000),
            title_text: rgb(0xffe0a0),
            accent: rgb(0xffb000),
            accent_text: rgb(0x0d0700),
            field: rgb(0x0a0500),
            field_text: rgb(0xffb000),
            highlight: rgb(0xffb000),
            crt: true,
        },
        Skin::Vapor => Palette {
            desktop: rgb(0x1a0b2e),
            face: rgb(0x2d1b4e),
            light: rgb(0xff71ce),
            shadow: rgb(0x120726),
            dark: rgb(0x07020f),
            text: rgb(0xf3e8ff),
            disabled: rgb(0x8a72b0),
            title_a: rgb(0xff71ce),
            title_b: rgb(0x01cdfe),
            title_text: rgb(0xffffff),
            accent: rgb(0x01cdfe),
            accent_text: rgb(0x1a0b2e),
            field: rgb(0x12082a),
            field_text: rgb(0xf3e8ff),
            highlight: rgb(0xfffb96),
            crt: false,
        },
        Skin::Pocket => Palette {
            desktop: rgb(0x0f380f),
            face: rgb(0x8bac0f),
            light: rgb(0x9bbc0f),
            shadow: rgb(0x306230),
            dark: rgb(0x0f380f),
            text: rgb(0x0f380f),
            disabled: rgb(0x306230),
            title_a: rgb(0x306230),
            title_b: rgb(0x0f380f),
            title_text: rgb(0x9bbc0f),
            accent: rgb(0x0f380f),
            accent_text: rgb(0x9bbc0f),
            field: rgb(0x9bbc0f),
            field_text: rgb(0x0f380f),
            highlight: rgb(0x306230),
            crt: true,
        },
    }
}

fn gray(c: Color32) -> Color32 {
    let l = (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) as u8;
    Color32::from_rgb(l, l, l)
}

impl Palette {
    pub fn grayscale(mut self) -> Self {
        for c in [
            &mut self.desktop,
            &mut self.face,
            &mut self.light,
            &mut self.shadow,
            &mut self.dark,
            &mut self.text,
            &mut self.disabled,
            &mut self.title_a,
            &mut self.title_b,
            &mut self.title_text,
            &mut self.accent,
            &mut self.accent_text,
            &mut self.field,
            &mut self.field_text,
            &mut self.highlight,
        ] {
            *c = gray(*c);
        }
        self
    }

    fn is_dark(&self) -> bool {
        gray(self.face).r() < 110
    }
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("vt323".into(), FontData::from_static(include_bytes!("../assets/VT323-Regular.ttf")).into());
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(fam).or_default().insert(0, "vt323".into());
    }
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, p: &Palette) {
    ctx.style_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(16.0)),
            (TextStyle::Body, FontId::proportional(20.0)),
            (TextStyle::Button, FontId::proportional(20.0)),
            (TextStyle::Heading, FontId::proportional(30.0)),
            (TextStyle::Monospace, FontId::monospace(19.0)),
        ]
        .into();
        s.spacing.item_spacing = vec2(8.0, 5.0);
        s.spacing.button_padding = vec2(10.0, 3.0);
        s.spacing.interact_size.y = 24.0;
        let mut v = if p.is_dark() { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.override_text_color = Some(p.text);
        v.panel_fill = p.face;
        v.window_fill = p.face;
        v.window_stroke = Stroke::new(2.0_f32, p.dark);
        v.window_corner_radius = CornerRadius::ZERO;
        v.menu_corner_radius = CornerRadius::ZERO;
        v.window_shadow = egui::Shadow { offset: [6, 6], blur: 0, spread: 0, color: Color32::from_black_alpha(110) };
        v.popup_shadow = egui::Shadow { offset: [4, 4], blur: 0, spread: 0, color: Color32::from_black_alpha(110) };
        v.extreme_bg_color = p.field;
        v.text_edit_bg_color = Some(p.field);
        v.faint_bg_color = p.face;
        v.code_bg_color = p.field;
        v.hyperlink_color = p.accent;
        v.selection.bg_fill = p.accent;
        v.selection.stroke = Stroke::new(1.0_f32, p.accent_text);
        v.striped = true;
        for (w, bg) in [
            (&mut v.widgets.noninteractive, p.face),
            (&mut v.widgets.inactive, p.face),
            (&mut v.widgets.hovered, p.face),
            (&mut v.widgets.active, p.shadow),
            (&mut v.widgets.open, p.face),
        ] {
            w.bg_fill = bg;
            w.weak_bg_fill = bg;
            w.corner_radius = CornerRadius::ZERO;
            w.bg_stroke = Stroke::new(1.0_f32, p.shadow);
            w.fg_stroke = Stroke::new(1.5_f32, p.text);
            w.expansion = 0.0;
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.shadow);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
        v.widgets.hovered.weak_bg_fill = p.accent;
        v.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, p.accent_text);
        v.widgets.active.fg_stroke = Stroke::new(1.5_f32, p.accent_text);
        v.widgets.active.weak_bg_fill = p.accent;
        v.widgets.inactive.bg_fill = p.field;
        v.widgets.inactive.fg_stroke = Stroke::new(1.5_f32, p.text);
        s.visuals = v;
    });
}

// ---------- widgets ----------

/// Relevo de 2 px: claro em cima/esquerda e escuro embaixo/direita (ou o contrário, afundado).
pub fn bevel(painter: &egui::Painter, r: Rect, p: &Palette, sunken: bool) {
    let (tl, br) = if sunken { (p.shadow, p.light) } else { (p.light, p.dark) };
    let (tl2, br2) = if sunken { (p.dark, p.face) } else { (p.face, p.shadow) };
    let r = r.shrink(0.5);
    let line = |a: Pos2, b: Pos2, c: Color32| painter.line_segment([a, b], Stroke::new(1.0_f32, c));
    line(r.left_bottom(), r.left_top(), tl);
    line(r.left_top(), r.right_top(), tl);
    line(r.right_top(), r.right_bottom(), br);
    line(r.right_bottom(), r.left_bottom(), br);
    let r = r.shrink(1.0);
    line(r.left_bottom(), r.left_top(), tl2);
    line(r.left_top(), r.right_top(), tl2);
    line(r.right_top(), r.right_bottom(), br2);
    line(r.right_bottom(), r.left_bottom(), br2);
}

pub fn button(ui: &mut Ui, p: &Palette, text: &str, enabled: bool, min_width: f32) -> Response {
    let font = TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, p.text);
    let size = vec2((galley.size().x + 24.0).max(min_width), galley.size().y + 10.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let pressed = enabled && resp.is_pointer_button_down_on();
        let painter = ui.painter();
        let fill = if enabled && resp.hovered() { p.face.lerp_to_gamma(p.light, 0.18) } else { p.face };
        painter.rect_filled(rect, 0.0, fill);
        bevel(painter, rect, p, pressed);
        let shift = if pressed { vec2(1.0, 1.0) } else { Vec2::ZERO };
        let pos = rect.center() - galley.size() / 2.0 + shift;
        if enabled {
            painter.galley(pos, galley, p.text);
        } else {
            let g2 = ui.painter().layout_no_wrap(text.to_string(), TextStyle::Button.resolve(ui.style()), p.light);
            painter.galley(pos + vec2(1.0, 1.0), g2, p.light);
            painter.galley(pos, galley, p.disabled);
        }
        if enabled && resp.has_focus() {
            painter.rect_stroke(rect.shrink(4.0), 0.0, Stroke::new(1.0_f32, p.shadow), StrokeKind::Inside);
        }
    }
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// Faixa de título com degradê (as janelas do Hyprland não têm barra de título).
pub fn title_bar(ui: &mut Ui, p: &Palette, text: &str, t: f64) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    gradient(ui.painter(), rect, p.title_a, p.title_b);
    // Ícone: um mini CD girando.
    let c = pos2(rect.left() + 17.0, rect.center().y);
    ui.painter().circle_filled(c, 10.0, Color32::from_rgb(220, 224, 236));
    let a = (t * 2.0) as f32;
    for i in 0..3 {
        let ang = a + i as f32 * TAU / 3.0;
        let col = [Color32::from_rgb(255, 113, 206), Color32::from_rgb(1, 205, 254), Color32::from_rgb(255, 251, 150)][i];
        ui.painter().line_segment([c + Vec2::angled(ang) * 4.0, c + Vec2::angled(ang) * 9.0], Stroke::new(2.0_f32, col));
    }
    ui.painter().circle_filled(c, 2.5, p.title_a);
    ui.painter().text(pos2(rect.left() + 34.0, rect.center().y), Align2::LEFT_CENTER, text, FontId::proportional(22.0), p.title_text);
}

pub fn gradient(painter: &egui::Painter, rect: Rect, a: Color32, b: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), a);
    mesh.colored_vertex(rect.right_top(), b);
    mesh.colored_vertex(rect.right_bottom(), b);
    mesh.colored_vertex(rect.left_bottom(), a);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// Caixa de grupo com borda entalhada e título.
pub fn group<R>(ui: &mut Ui, p: &Palette, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let font = FontId::proportional(18.0);
    let inner = egui::Frame::new().inner_margin(egui::Margin { left: 10, right: 10, top: 16, bottom: 8 }).show(ui, add);
    let r = inner.response.rect.shrink2(vec2(1.0, 0.0));
    let r = Rect::from_min_max(pos2(r.left(), r.top() + 8.0), r.max);
    let painter = ui.painter();
    painter.rect_stroke(r.translate(vec2(1.0, 1.0)), 0.0, Stroke::new(1.0_f32, p.light), StrokeKind::Inside);
    painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, p.shadow), StrokeKind::Inside);
    let galley = painter.layout_no_wrap(title.to_string(), font, p.text);
    let tr = Rect::from_min_size(pos2(r.left() + 8.0, r.top() - galley.size().y / 2.0), galley.size() + vec2(8.0, 0.0));
    painter.rect_filled(tr, 0.0, p.face);
    painter.galley(tr.min + vec2(4.0, 0.0), galley, p.text);
    inner.inner
}

/// Linhas de varredura de monitor CRT por cima de tudo.
pub fn scanlines(ctx: &egui::Context) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("crt")));
    let r = ctx.content_rect();
    let mut y = r.top();
    while y < r.bottom() {
        painter.rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 1.0)), 0.0, Color32::from_black_alpha(55));
        y += 3.0;
    }
}

// ---------- mascote: a "Disquinha", um CD kawaii ----------

const BOW: [&str; 5] = ["PP...PP", "PPP.PPP", "PPPKPPP", "PPP.PPP", "PP...PP"];

fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    Color32::from_rgb(((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

/// Desenha o mascote (grade de 24×26 "pixels") centrado em `center`, `px` = tamanho do pixel.
pub fn mascot(painter: &egui::Painter, center: Pos2, px: f32, t: f64, happy: bool) {
    let t = t as f32;
    let bob = (t * 2.2).sin() * px * 0.8;
    let origin = center - vec2(12.0 * px, 13.0 * px) + vec2(0.0, bob);
    let put = |x: i32, y: i32, c: Color32| {
        painter.rect_filled(Rect::from_min_size(origin + vec2(x as f32 * px, y as f32 * px), Vec2::splat(px + 0.3)), 0.0, c);
    };
    // Sombra no chão.
    let sh = center + vec2(0.0, 14.0 * px);
    painter.add(Shape::ellipse_filled(sh, vec2(8.0 * px - bob * 0.5, 1.6 * px), Color32::from_black_alpha(70)));
    // Pezinhos.
    let shoe = Color32::from_rgb(214, 48, 92);
    for (x0, y0) in [(6, 22), (15, 22)] {
        for dx in 0..4 {
            for dy in 0..2 {
                put(x0 + dx, y0 + dy, shoe);
            }
        }
    }
    // Bracinhos (o direito acena).
    let wave = ((t * 4.0).sin() > 0.0) as i32;
    let arm = Color32::from_rgb(60, 60, 84);
    put(0, 14, arm);
    put(1, 14, arm);
    put(22, 12 - wave, arm);
    put(23, 11 - 2 * wave, arm);
    // Disco com brilho de arco-íris que gira.
    for y in 0..24 {
        for x in 0..24 {
            let (dx, dy) = (x as f32 + 0.5 - 12.0, y as f32 + 0.5 - 12.0);
            let r = (dx * dx + dy * dy).sqrt();
            if !(1.2..=11.2).contains(&r) {
                continue;
            }
            let c = if r > 10.2 {
                Color32::from_rgb(43, 43, 64)
            } else if r < 2.3 {
                Color32::from_rgb(236, 238, 246)
            } else {
                let ang = dy.atan2(dx);
                let band = ((ang * 2.0 - t * 1.5).sin() * 0.5 + 0.5).powf(3.0);
                let base = Color32::from_rgb(214, 220, 234);
                base.lerp_to_gamma(hsv(ang / TAU + t * 0.05 + r * 0.03, 0.55, 1.0), band * 0.7)
            };
            put(x, y, c);
        }
    }
    // Rostinho.
    let ink = Color32::from_rgb(26, 26, 46);
    let blink = (t % 4.3) > 4.1;
    for ex in [5, 17] {
        if blink || happy {
            put(ex, 13, ink);
            put(ex + 1, 13, ink);
            if happy && !blink {
                put(ex, 12, ink);
                put(ex + 1, 12, ink);
                put(ex - 1, 13, ink);
                put(ex + 2, 13, ink);
            }
        } else {
            for dy in 0..3 {
                put(ex, 11 + dy, ink);
                put(ex + 1, 11 + dy, ink);
            }
            put(ex, 11, Color32::WHITE);
        }
    }
    let blush = Color32::from_rgb(255, 143, 177);
    for (x, y) in [(3, 15), (4, 15), (19, 15), (20, 15)] {
        put(x, y, blush);
    }
    let mouth = Color32::from_rgb(122, 32, 64);
    for (x, y) in [(10, 15), (11, 16), (12, 15), (13, 16), (14, 15)] {
        put(x, y, mouth);
    }
    // Lacinho.
    for (row, line) in BOW.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            let c = match ch {
                'P' => Color32::from_rgb(255, 113, 206),
                'K' => Color32::from_rgb(196, 40, 140),
                _ => continue,
            };
            put(15 + col as i32, row as i32, c);
        }
    }
}

/// Balãozinho de fala pixelado.
pub fn speech(painter: &egui::Painter, tip: Pos2, text: &str, bounds: Rect) {
    let galley = painter.layout(text.to_string(), FontId::proportional(18.0), Color32::from_rgb(26, 26, 46), (bounds.width() - 24.0).min(260.0));
    let size = galley.size() + vec2(16.0, 10.0);
    let x = (tip.x - size.x / 2.0).clamp(bounds.left() + 2.0, (bounds.right() - size.x - 5.0).max(bounds.left() + 2.0));
    let r = Rect::from_min_size(pos2(x, tip.y - size.y - 8.0), size);
    let bg = Color32::from_rgb(255, 253, 240);
    painter.rect_filled(r.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(90));
    painter.rect_filled(r, 0.0, bg);
    painter.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(26, 26, 46)), StrokeKind::Inside);
    let tail = [pos2(tip.x - 6.0, r.bottom() - 1.0), pos2(tip.x + 6.0, r.bottom() - 1.0), pos2(tip.x, tip.y)];
    painter.add(Shape::convex_polygon(tail.to_vec(), bg, Stroke::NONE));
    painter.line_segment([tail[0], tail[2]], Stroke::new(2.0_f32, Color32::from_rgb(26, 26, 46)));
    painter.line_segment([tail[1], tail[2]], Stroke::new(2.0_f32, Color32::from_rgb(26, 26, 46)));
    painter.galley(r.min + vec2(8.0, 5.0), galley, Color32::from_rgb(26, 26, 46));
}
