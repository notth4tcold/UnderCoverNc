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

/// Opções do driver da impressora: (código, nome legível) e o código padrão.
#[derive(Default, Clone)]
pub struct PrinterOptions {
    /// Tipo de papel + qualidade (ex.: Epson "MediaType": PLAIN_NORMAL, PSGLOS_HIGH...).
    pub media_types: Vec<(String, String)>,
    pub media_default: String,
    pub sources: Vec<(String, String)>,
    pub media_key: String,
    pub source_key: String,
    /// Opção de cor: (chave, código colorido, código tons de cinza).
    pub color: Option<(String, String, String)>,
}

impl PrinterOptions {
    pub fn media_label(&self, code: &str) -> Option<&str> {
        self.media_types.iter().find(|(c, _)| c == code).map(|(_, l)| l.as_str())
    }
}

/// Baixa o PPD que o próprio CUPS serve em localhost:631 (o arquivo em /etc/cups/ppd só o root lê).
fn fetch_ppd(printer: &str) -> Option<String> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect_timeout(&"127.0.0.1:631".parse().ok()?, std::time::Duration::from_millis(800)).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(3))).ok()?;
    let name: String = printer.chars().filter(|c| c.is_ascii_alphanumeric() || "-_.".contains(*c)).collect();
    write!(s, "GET /printers/{name}.ppd HTTP/1.0\r\nHost: localhost\r\n\r\n").ok()?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let (head, body) = text.split_once("\r\n\r\n")?;
    head.lines().next()?.contains(" 200 ").then(|| body.to_string())
}

/// Lê as opções PickOne do PPD: chave → (padrão, [(código, nome)]).
fn parse_ppd(ppd: &str) -> std::collections::HashMap<String, (String, Vec<(String, String)>)> {
    let mut out: std::collections::HashMap<String, (String, Vec<(String, String)>)> = Default::default();
    for line in ppd.lines() {
        let Some(rest) = line.strip_prefix('*') else { continue };
        if let Some(d) = rest.strip_prefix("Default") {
            if let Some((k, v)) = d.split_once(':') {
                out.entry(k.trim().to_string()).or_default().0 = v.trim().to_string();
            }
            continue;
        }
        if rest.starts_with(['%', '?']) || rest.starts_with("OpenUI") || rest.starts_with("CloseUI") || rest.starts_with("Order") {
            continue;
        }
        // "*MediaType PLAIN_HIGH/Plain paper-High: "..."
        let Some((lhs, _)) = rest.split_once(':') else { continue };
        let Some((key, opt)) = lhs.split_once(' ') else { continue };
        let (code, label) = opt.split_once('/').unwrap_or((opt, opt));
        if matches!(key, "MediaType" | "MediaQuality" | "InputSlot" | "MediaSource" | "Ink" | "ColorModel" | "ColorMode") {
            out.entry(key.to_string()).or_default().1.push((code.trim().to_string(), label.trim().to_string()));
        }
    }
    out
}

pub fn printer_options(printer: &str) -> PrinterOptions {
    let mut o = PrinterOptions { media_key: "MediaType".into(), source_key: "InputSlot".into(), ..Default::default() };
    let mut opts = fetch_ppd(printer).map(|p| parse_ppd(&p)).unwrap_or_default();
    // Sem PPD: usa os códigos que o lpoptions mostra.
    if opts.values().all(|(_, v)| v.is_empty()) {
        for line in run("lpoptions", &["-p", printer, "-l"]).unwrap_or_default().lines() {
            let Some((key, vals)) = line.split_once(':') else { continue };
            let key = key.split('/').next().unwrap_or("").trim().to_string();
            let mut def = String::new();
            let list = vals
                .split_whitespace()
                .map(|v| {
                    if let Some(d) = v.strip_prefix('*') {
                        def = d.to_string();
                    }
                    let v = v.trim_start_matches('*').to_string();
                    (v.clone(), v)
                })
                .collect();
            opts.insert(key, (def, list));
        }
    }
    for key in ["MediaType", "MediaQuality"] {
        if let Some((d, v)) = opts.get(key).filter(|x| !x.1.is_empty()) {
            o.media_key = key.into();
            o.media_types = v.clone();
            o.media_default = d.clone();
            break;
        }
    }
    for key in ["InputSlot", "MediaSource"] {
        if let Some((_, v)) = opts.get(key).filter(|x| !x.1.is_empty()) {
            o.source_key = key.into();
            o.sources = v.clone();
            break;
        }
    }
    for key in ["Ink", "ColorModel", "ColorMode"] {
        let Some((d, v)) = opts.get(key) else { continue };
        let is_mono = |(c, l): &(String, String)| {
            let t = format!("{c} {l}").to_lowercase();
            ["mono", "gray", "grey", "black", "cinza"].iter().any(|w| t.contains(w))
        };
        if let (Some(mono), Some(color)) = (v.iter().find(|x| is_mono(x)), v.iter().find(|x| !is_mono(x) && (x.0 == *d || !is_mono(x)))) {
            o.color = Some((key.to_string(), color.0.clone(), mono.0.clone()));
            break;
        }
    }
    o
}

/// Nome do tipo de papel em português (os drivers costumam vir em inglês).
pub fn media_pt(label: &str) -> String {
    let mut s = label.to_string();
    for (a, b) in [
        ("Plain paper", "Papel comum"),
        ("Epson Photo Quality Ink Jet", "Epson jato de tinta fotográfico"),
        ("Photo Quality Ink Jet", "Jato de tinta fotográfico"),
        ("Epson Ultra Glossy", "Epson Ultra Brilhante (glossy)"),
        ("Epson Premium Glossy", "Epson Premium Brilhante (glossy)"),
        ("Epson Premium Semigloss", "Epson Premium Semibrilho (semigloss)"),
        ("Photo Paper Glossy", "Fotográfico Brilhante (glossy)"),
        ("Epson Matte", "Epson Fosco (matte)"),
        ("-Photo RPM", " — qualidade máxima"),
        ("-High", " — qualidade alta"),
        ("-Standard", " — qualidade normal"),
    ] {
        s = s.replace(a, b);
    }
    s
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
    pub grayscale: bool,
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
    if let Some((key, color, mono)) = &opts.color {
        args.extend(["-o".into(), format!("{key}={}", if job.grayscale { mono } else { color })]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_media_types_from_ppd() {
        let ppd = "*OpenUI *MediaType/Print Quality: PickOne\n*DefaultMediaType: PLAIN_NORMAL\n\
                   *MediaType PLAIN_NORMAL/Plain paper-Standard: \"<<>>setpagedevice\"\n\
                   *MediaType PSGLOS_HIGH/Epson Premium Semigloss-Photo RPM: \"x\"\n*CloseUI: *MediaType\n\
                   *DefaultInk: COLOR\n*Ink COLOR/Off: \"\"\n*Ink MONO/On: \"\"\n";
        let o = parse_ppd(ppd);
        let (def, list) = &o["MediaType"];
        assert_eq!(def, "PLAIN_NORMAL");
        assert_eq!(list[1], ("PSGLOS_HIGH".into(), "Epson Premium Semigloss-Photo RPM".into()));
        assert_eq!(o["Ink"].1.len(), 2);
        assert_eq!(media_pt("Epson Premium Semigloss-Photo RPM"), "Epson Premium Semibrilho (semigloss) — qualidade máxima");
    }
}
