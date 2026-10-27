//! Impressão via CUPS (`lp`, `lpstat`, `lpoptions`).

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).env("LC_ALL", "C").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Impressoras disponíveis e a padrão do sistema.
pub fn printers() -> (Vec<String>, Option<String>) {
    let list = run("lpstat", &["-e"]).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let default = run("lpstat", &["-d"])
        .and_then(|s| s.split_once(':').map(|(_, d)| d.trim().to_string()))
        .filter(|d| !d.is_empty() && !d.contains("no system default"));
    (list, default)
}

/// Tipos de mídia e bandejas que o driver da impressora aceita.
pub struct PrinterOptions {
    pub media_types: Vec<String>,
    pub sources: Vec<String>,
    /// Nome da opção de tipo de mídia (normalmente MediaType).
    pub media_key: String,
    pub source_key: String,
}

pub fn printer_options(printer: &str) -> PrinterOptions {
    let mut o = PrinterOptions { media_types: vec![], sources: vec![], media_key: "MediaType".into(), source_key: "InputSlot".into() };
    for line in run("lpoptions", &["-p", printer, "-l"]).unwrap_or_default().lines() {
        let Some((key, vals)) = line.split_once(':') else { continue };
        let key = key.split('/').next().unwrap_or("").trim();
        let vals = || vals.split_whitespace().map(|v| v.trim_start_matches('*').to_string()).collect::<Vec<_>>();
        match key {
            "MediaType" | "MediaQuality" => {
                o.media_key = key.into();
                o.media_types = vals();
            }
            "InputSlot" | "MediaSource" => {
                o.source_key = key.into();
                o.sources = vals();
            }
            _ => {}
        }
    }
    o
}

/// Nome de mídia do CUPS para um papel em mm.
pub fn media_name(paper: (f32, f32)) -> String {
    let near = |a: f32, b: f32| (a - b).abs() < 0.6;
    if near(paper.0, 210.0) && near(paper.1, 297.0) {
        "A4".into()
    } else if near(paper.0, 215.9) && near(paper.1, 279.4) {
        "Letter".into()
    } else if near(paper.0, 148.0) && near(paper.1, 210.0) {
        "A5".into()
    } else {
        format!("Custom.{:.1}x{:.1}mm", paper.0, paper.1)
    }
}

pub struct Job<'a> {
    pub printer: &'a str,
    pub title: &'a str,
    pub copies: u32,
    pub paper: (f32, f32),
    pub media_type: &'a str,
    pub paper_source: &'a str,
}

/// Manda o PDF para a impressora sem nenhuma escala (tamanho real).
pub fn print_pdf(pdf: &Path, job: &Job) -> Result<String> {
    let opts = printer_options(job.printer);
    let mut args: Vec<String> = vec![
        "-d".into(),
        job.printer.into(),
        "-t".into(),
        job.title.into(),
        "-n".into(),
        job.copies.max(1).to_string(),
        "-o".into(),
        format!("media={}", media_name(job.paper)),
        "-o".into(),
        "print-scaling=none".into(),
        "-o".into(),
        "fit-to-page=false".into(),
    ];
    if !job.media_type.is_empty() {
        args.extend(["-o".into(), format!("{}={}", opts.media_key, job.media_type)]);
    }
    if !job.paper_source.is_empty() {
        args.extend(["-o".into(), format!("{}={}", opts.source_key, job.paper_source)]);
    }
    args.push(pdf.to_string_lossy().into_owned());
    let out = Command::new("lp").args(&args).output().context("não foi possível executar `lp` (o CUPS está instalado?)")?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Abre a configuração de impressoras do sistema (gráfica, se houver; senão a página do CUPS).
pub fn open_system_settings() {
    for (cmd, args) in [
        ("system-config-printer", vec![]),
        ("kcmshell6", vec!["kcm_printer_manager"]),
        ("xdg-open", vec!["http://localhost:631/printers/"]),
    ] {
        if Command::new(cmd).args(&args).spawn().is_ok() {
            return;
        }
    }
}
