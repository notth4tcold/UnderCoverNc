//! Arquivos de imagem: abrir, achar a capa par (frente ↔ verso) e montar lotes.

use anyhow::{Context, Result};
use image::RgbImage;
use std::path::{Path, PathBuf};

pub const EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "gif", "webp", "tif", "tiff"];

pub fn is_image(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

pub fn load_image(p: &Path) -> Result<RgbImage> {
    let img = image::ImageReader::open(p)
        .with_context(|| format!("não foi possível abrir {}", p.display()))?
        .with_guessed_format()?
        .decode()
        .with_context(|| format!("formato de imagem não suportado: {}", p.display()))?;
    Ok(img.to_rgb8())
}

pub fn keywords(s: &str) -> Vec<String> {
    s.split(';').map(|k| k.trim().to_lowercase()).filter(|k| !k.is_empty()).collect()
}

fn stem_lower(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Procura a capa "par" na mesma pasta: troca a palavra-chave `from[i]` por `to[i]`
/// no nome (ex.: "Jogo front.jpg" → "Jogo back.jpg"). As listas são pareadas por posição.
pub fn find_match(path: &Path, from: &[String], to: &[String]) -> Option<PathBuf> {
    if to.is_empty() {
        return None;
    }
    let dir = path.parent()?;
    let stem = stem_lower(path);
    let mut pairs: Vec<(&String, &String)> = from.iter().enumerate().map(|(i, k)| (k, to.get(i).unwrap_or(to.last().unwrap()))).collect();
    pairs.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    let entries: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| is_image(p) && p != path).collect();
    for (k, t) in pairs {
        let Some(i) = stem.rfind(k.as_str()) else { continue };
        let want = format!("{}{}{}", &stem[..i], t, &stem[i + k.len()..]);
        if let Some(found) = entries.iter().find(|p| stem_lower(p) == want) {
            return Some(found.clone());
        }
    }
    None
}

/// Varre uma pasta e junta as capas em pares (esquerda, direita) pelas palavras-chave.
pub fn batch_pairs(dir: &Path, left: &[String], right: &[String]) -> Vec<(Option<PathBuf>, Option<PathBuf>)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).filter(|p| is_image(p)).collect()).unwrap_or_default();
    files.sort();
    let has = |p: &Path, ks: &[String]| {
        let s = stem_lower(p);
        ks.iter().any(|k| s.contains(k.as_str()))
    };
    let mut used = std::collections::HashSet::new();
    let mut out = Vec::new();
    for f in &files {
        if used.contains(f) || !has(f, left) {
            continue;
        }
        let m = find_match(f, left, right).filter(|m| !used.contains(m));
        used.insert(f.clone());
        if let Some(m) = &m {
            used.insert(m.clone());
        }
        out.push((Some(f.clone()), m));
    }
    for f in &files {
        if !used.contains(f) && has(f, right) {
            used.insert(f.clone());
            out.push((None, Some(f.clone())));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_front_back() {
        let d = std::env::temp_dir().join(format!("ucnc-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for n in ["Crash Front.jpg", "Crash Back.png", "Spyro -f.jpg", "Spyro -b.jpg", "solo front.jpg"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        let l = keywords("front;-f");
        let r = keywords("back;-b");
        assert_eq!(find_match(&d.join("Crash Front.jpg"), &l, &r), Some(d.join("Crash Back.png")));
        assert_eq!(find_match(&d.join("Spyro -b.jpg"), &r, &l), Some(d.join("Spyro -f.jpg")));
        let pairs = batch_pairs(&d, &l, &r);
        assert_eq!(pairs.len(), 3);
        assert!(pairs.contains(&(Some(d.join("solo front.jpg")), None)));
        std::fs::remove_dir_all(d).unwrap();
    }
}
