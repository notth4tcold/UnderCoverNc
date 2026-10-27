use crate::format::{self, Edge, LineStyle, Marker, default_formats, parse_marker};
use crate::layout::{self, DrawOpts, PageSetup, Prim, Side};
use image::{Rgb, RgbImage};

const A4: PageSetup = PageSetup { paper: (210.0, 297.0), margin: 3.0, correct_area: true, tile: true };

fn fmt(name: &str) -> format::CoverFormat {
    default_formats().into_iter().find(|f| f.name == name).unwrap()
}

#[test]
fn markers_roundtrip() {
    for f in default_formats() {
        let text = f.markers_text();
        assert_eq!(format::parse_markers(&text).unwrap(), f.markers, "{}", f.name);
    }
    assert_eq!(parse_marker("LINE = LEFT").unwrap(), Marker::Edge(Edge::Left, LineStyle::Dashed));
    assert_eq!(
        parse_marker("line = (6.5, -10.0)(6.5, 0.0), solid").unwrap(),
        Marker::Line { a: (6.5, -10.0), b: (6.5, 0.0), style: LineStyle::Solid }
    );
    assert!(parse_marker("RECT = (1, 2)").is_err());
    assert!(parse_marker("FOO = LEFT").is_err());
}

#[test]
fn landscape_dvd_is_rotated_front_up() {
    // Capa de DVD deitada: metade esquerda vermelha (verso), direita azul (frente).
    let mut img = RgbImage::new(274, 184);
    for (x, _, p) in img.enumerate_pixels_mut() {
        *p = if x < 137 { Rgb([255, 0, 0]) } else { Rgb([0, 0, 255]) };
    }
    let p = layout::prepare(&img, &fmt("Caixa de DVD"), 0, true);
    assert_eq!(p.img.dimensions(), (184, 274));
    assert_eq!(p.img.get_pixel(90, 10)[2], 255, "frente (azul) deve ficar em cima");
    assert_eq!(p.img.get_pixel(90, 260)[0], 255);
}

#[test]
fn slimline_remap_shrinks_spine() {
    let f = fmt("DVD impresso como Slim 7mm");
    let img = RgbImage::from_pixel(1830, 2743, Rgb([10, 20, 30]));
    let p = layout::prepare(&img, &f, 0, true);
    // 10 px/mm: (268 - 7)/2 = 130.5 → 131 (arredondado) ×2 + 70.
    let h = p.img.height() as i32;
    assert!((h - 2680).abs() <= 2, "altura {h}");
}

#[test]
fn detects_formats_by_ratio() {
    let list = default_formats();
    let name = |w, h| layout::detect_format(w, h, &list, true).map(|i| list[i].name.clone());
    assert_eq!(name(1000, 1000).as_deref(), Some("CD Frente"));
    assert_eq!(name(1510, 1176).as_deref(), Some("CD Verso"));
    assert_eq!(name(2743, 1842).as_deref(), Some("Caixa de DVD"));
    assert_eq!(name(1000, 300), None);
}

#[test]
fn single_page_stacks_cd_front_and_back() {
    let (a, b) = (fmt("CD Frente"), fmt("CD Verso"));
    let (pages, warn) = layout::plan(&[(Side::Left, &a, None), (Side::Right, &b, None)], true, &A4);
    assert!(warn.is_empty());
    assert_eq!(pages.len(), 1);
    let it = &pages[0].items;
    assert!(it[1].y >= it[0].y + a.height);
    // DVD + CD não cabem numa A4: vira duas páginas com aviso.
    let d = fmt("Caixa de DVD");
    let (pages, warn) = layout::plan(&[(Side::Left, &d, None), (Side::Right, &b, None)], true, &A4);
    assert_eq!(pages.len(), 2);
    assert_eq!(warn.len(), 1);
}

#[test]
fn manual_position_is_kept_inside_printable_area() {
    let mut f = fmt("CD Frente");
    f.left_pos = Some((150.0, -20.0));
    let (pages, _) = layout::plan(&[(Side::Left, &f, None)], false, &A4);
    let it = &pages[0].items[0];
    assert!((it.x - (210.0 - 3.0 - 120.5)).abs() < 0.01 && (it.y - 3.0).abs() < 0.01);
}

#[test]
fn edge_lines_avoid_covers() {
    let f = fmt("Caixa de DVD");
    let (pages, _) = layout::plan(&[(Side::Left, &f, None)], false, &A4);
    let it = &pages[0].items[0];
    let prims = layout::page_prims(&pages[0], &DrawOpts { cut_lines: true, border: false, margin: 3.0 });
    for p in &prims {
        if let (_, Prim::Line { a, b, dashed: true }) = p {
            let inside = |q: (f32, f32)| q.0 > it.x + 0.1 && q.0 < it.x + f.width - 0.1 && q.1 > it.y + 0.1 && q.1 < it.y + f.height - 0.1;
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            assert!(!inside(mid), "linha passando por cima da capa: {a:?} {b:?}");
        }
    }
    // 4 bordas × 2 pedaços + 4 marcas de lombada.
    assert_eq!(prims.len(), 12);
}

#[test]
fn pdf_is_well_formed() {
    let f = fmt("Rótulo de CD (A4)");
    let p = layout::prepare(&RgbImage::from_pixel(50, 50, Rgb([200, 0, 0])), &f, 0, true);
    let (pages, _) = layout::plan(&[(Side::Left, &f, Some(&p))], false, &A4);
    let pdf = crate::pdf::render(&pages, &DrawOpts { cut_lines: true, border: true, margin: 3.0 }, (0.0, 0.0), "t(e)st").unwrap();
    let s = String::from_utf8_lossy(&pdf);
    assert!(s.starts_with("%PDF-1.4"));
    assert!(s.contains("/MediaBox [0 0 595.276 841.890]"));
    assert!(s.contains("W* n"));
    assert!(s.trim_end().ends_with("%%EOF"));
    // A tabela xref aponta para os objetos certos.
    let find = |needle: &[u8]| pdf.windows(needle.len()).rposition(|w| w == needle).unwrap();
    let xref = find(b"startxref\n");
    let tail = String::from_utf8_lossy(&pdf[xref + 10..]);
    let off: usize = tail.lines().next().unwrap().parse().unwrap();
    assert!(pdf[off..].starts_with(b"xref"));
    let table = String::from_utf8_lossy(&pdf[off..]).into_owned();
    let mut n = 0;
    for line in table.lines().skip(3).take_while(|l| l.ends_with(" n ")) {
        let o: usize = line[..10].parse().unwrap();
        let head = String::from_utf8_lossy(&pdf[o..o + 12]).into_owned();
        assert!(head.split_whitespace().nth(1) == Some("0") && head.contains("obj"), "{head}");
        n += 1;
    }
    assert_eq!(n, 6);
}


#[test]
fn big_vhs_covers_are_tiled_on_a4() {
    // Caixa de locadora cabe inteira numa A4.
    let caixa = fmt("VHS caixa de locadora");
    let (pages, warn) = layout::plan(&[(Side::Left, &caixa, None)], false, &A4);
    assert_eq!((pages.len(), warn.len()), (1, 0));
    // Clamshell (211 mm de largura) não cabe: vira 2 folhas com emenda.
    let clam = fmt("VHS clamshell");
    let (pages, warn) = layout::plan(&[(Side::Left, &clam, None)], false, &A4);
    // Vai deitada (294,5 × 211,1): 2 folhas em vez de 4.
    assert_eq!(pages.len(), 2);
    assert!(pages[0].items[0].fmt.width > pages[0].items[0].fmt.height);
    assert_eq!(warn.len(), 1);
    let (t0, t1) = (pages[0].items[0].tile.clone().unwrap(), pages[1].items[0].tile.clone().unwrap());
    assert_eq!(t0.seams, t1.seams);
    assert_eq!(t0.seams.len(), 1);
    // Cada pedaço cabe na área imprimível e juntos cobrem a capa com sobreposição.
    for p in &pages {
        let t = p.items[0].tile.as_ref().unwrap();
        assert!(t.clip.0 >= 2.99 && t.clip.0 + t.clip.2 <= 207.01, "{:?}", t.clip);
    }
    let it0 = &pages[0].items[0];
    let it1 = &pages[1].items[0];
    let right0 = t0.clip.0 + t0.clip.2 - it0.x;
    let left1 = t1.clip.0 - it1.x;
    assert!(right0 - left1 >= layout::OVERLAP_MM - 0.01);
    // Sem dividir: uma página só, com aviso de que não cabe.
    let no_tile = PageSetup { tile: false, ..A4 };
    let (pages, warn) = layout::plan(&[(Side::Left, &clam, None)], false, &no_tile);
    assert_eq!((pages.len(), warn.len()), (1, 1));
    // O PDF recorta cada pedaço.
    let pdf = crate::pdf::render(&layout::plan(&[(Side::Left, &clam, None)], false, &A4).0, &DrawOpts { cut_lines: true, border: false, margin: 3.0 }, (0.0, 0.0), "vhs").unwrap();
    assert!(String::from_utf8_lossy(&pdf).contains("re W n"));
}

#[test]
fn template_has_exact_size_and_folds() {
    let f = fmt("VHS caixa de locadora");
    let img = crate::template::render(&f, 254.0); // 10 px/mm, deitado para desenhar
    assert_eq!(img.dimensions(), (2572, 1969));
    assert_eq!(crate::template::render(&fmt("CD Frente"), 254.0).dimensions(), (1205, 1205));
    assert_eq!(crate::template::folds(&f), vec![(false, 114.3), (false, 142.9)]);
    assert_eq!(crate::template::folds(&fmt("CD Verso")).len(), 2);
    assert_eq!(crate::template::folds(&fmt("Caixa de DVD")).len(), 2);
}

