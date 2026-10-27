//! Gerador de PDF mínimo: imagens JPEG, linhas, retângulos e círculos, em
//! tamanho real (1 mm no PDF = 1 mm no papel).

use crate::layout::{DrawOpts, Page, Prim, page_prims};
use anyhow::Result;
use image::codecs::jpeg::JpegEncoder;
use std::fmt::Write as _;

const PT: f32 = 72.0 / 25.4;
/// Constante para aproximar um quarto de círculo com Bézier.
const KAPPA: f32 = 0.552_284_8;

struct Writer {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl Writer {
    fn obj(&mut self, id: usize, dict: &str, stream: Option<&[u8]>) {
        if self.offsets.len() <= id {
            self.offsets.resize(id + 1, 0);
        }
        self.offsets[id] = self.buf.len();
        self.buf.extend_from_slice(format!("{id} 0 obj\n{dict}\n").as_bytes());
        if let Some(s) = stream {
            self.buf.extend_from_slice(b"stream\n");
            self.buf.extend_from_slice(s);
            self.buf.extend_from_slice(b"\nendstream\n");
        }
        self.buf.extend_from_slice(b"endobj\n");
    }
}

fn circle(s: &mut String, c: (f32, f32), r: f32) {
    let k = r * KAPPA;
    let (x, y) = c;
    let _ = write!(
        s,
        "{:.3} {:.3} m {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c \
         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c h ",
        x + r, y,
        x + r, y + k, x + k, y + r, x, y + r,
        x - k, y + r, x - r, y + k, x - r, y,
        x - r, y - k, x - k, y - r, x, y - r,
        x + k, y - r, x + r, y - k, x + r, y
    );
}

pub fn render(pages: &[Page], opts: &DrawOpts, offset: (f32, f32), title: &str) -> Result<Vec<u8>> {
    let mut w = Writer { buf: b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec(), offsets: vec![0] };
    // 1 = catálogo, 2 = árvore de páginas, 3 = info; o resto é alocado em sequência.
    let mut next = 4;
    let mut page_ids = Vec::new();
    for page in pages {
        let (pw, ph) = page.size;
        let to_pt = |(x, y): (f32, f32)| ((x + offset.0) * PT, (ph - y - offset.1) * PT);
        let mut content = String::new();
        let mut xobjects = String::new();
        let mut images = Vec::new();
        for (clip, prim) in page_prims(page, opts) {
            if let Some((cx, cy, cw, ch)) = clip {
                let (x, y) = to_pt((cx, cy + ch));
                let _ = write!(content, "q {:.3} {:.3} {:.3} {:.3} re W n ", x, y, cw * PT, ch * PT);
            }
            match prim {
                Prim::Image { item, rect, clip } => {
                    let Some(p) = page.items[item].prepared.as_deref() else { continue };
                    let mut jpg = Vec::new();
                    JpegEncoder::new_with_quality(&mut jpg, 94).encode_image(&p.img)?;
                    let id = next;
                    next += 1;
                    let name = format!("Im{id}");
                    let _ = write!(xobjects, "/{name} {id} 0 R ");
                    images.push((id, p.img.width(), p.img.height(), jpg));
                    content.push_str("q ");
                    if let Some((c, r, hole)) = clip {
                        let c = to_pt(c);
                        circle(&mut content, c, r * PT);
                        circle(&mut content, c, hole * PT);
                        content.push_str("W* n ");
                    }
                    let (x, y) = to_pt((rect.0, rect.1 + rect.3));
                    let _ = writeln!(content, "{:.3} 0 0 {:.3} {:.3} {:.3} cm /{name} Do Q", rect.2 * PT, rect.3 * PT, x, y);
                }
                Prim::Line { a, b, dashed } => {
                    let (a, b) = (to_pt(a), to_pt(b));
                    let _ = writeln!(content, "q {} {:.3} {:.3} m {:.3} {:.3} l S Q", stroke(dashed), a.0, a.1, b.0, b.1);
                }
                Prim::Rect { rect, dashed } => {
                    let (x, y) = to_pt((rect.0, rect.1 + rect.3));
                    let _ = writeln!(content, "q {} {:.3} {:.3} {:.3} {:.3} re S Q", stroke(dashed), x, y, rect.2 * PT, rect.3 * PT);
                }
                Prim::Circle { c, r, dashed } => {
                    let _ = write!(content, "q {} ", stroke(dashed));
                    circle(&mut content, to_pt(c), r * PT);
                    content.push_str("S Q\n");
                }
            }
            if clip.is_some() {
                content.push_str("Q\n");
            }
        }
        let page_id = next;
        let content_id = next + 1;
        next += 2;
        w.obj(
            page_id,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.3} {:.3}] /Resources << /XObject << {xobjects}>> >> /Contents {content_id} 0 R >>",
                pw * PT,
                ph * PT
            ),
            None,
        );
        w.obj(content_id, &format!("<< /Length {} >>", content.len()), Some(content.as_bytes()));
        for (id, iw, ih, jpg) in images {
            w.obj(
                id,
                &format!(
                    "<< /Type /XObject /Subtype /Image /Width {iw} /Height {ih} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>",
                    jpg.len()
                ),
                Some(&jpg),
            );
        }
        page_ids.push(page_id);
    }
    let kids: String = page_ids.iter().map(|id| format!("{id} 0 R ")).collect();
    w.obj(1, "<< /Type /Catalog /Pages 2 0 R >>", None);
    w.obj(2, &format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", page_ids.len()), None);
    let title: String = title.chars().filter(|c| !matches!(c, '(' | ')' | '\\')).collect();
    w.obj(3, &format!("<< /Title ({title}) /Producer (UnderCoverNc) >>"), None);

    let xref = w.buf.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", w.offsets.len());
    for off in &w.offsets[1..] {
        let _ = writeln!(tail, "{off:010} 00000 n ");
    }
    let _ = write!(tail, "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n", w.offsets.len());
    w.buf.extend_from_slice(tail.as_bytes());
    Ok(w.buf)
}

/// Linhas de corte: cinza, 0,2 mm; tracejadas ou contínuas.
fn stroke(dashed: bool) -> &'static str {
    if dashed { "0.35 G 0.57 w [6 4] 0 d" } else { "0.35 G 0.57 w [] 0 d" }
}
