mod files;
mod format;
mod gui;
mod i18n;
mod layout;
mod paint;
mod pdf;
mod print;
mod settings;
mod template;
mod theme;
#[cfg(test)]
mod tests;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("UnderCoverNc {} — impressão de capas de CD/DVD/Blu-ray\n\nUso: undercovernc [imagem-esquerda] [imagem-direita]", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("undercovernc {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let files: Vec<PathBuf> = args.into_iter().map(PathBuf::from).filter(|p| p.exists()).collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("UnderCoverNc")
            .with_app_id("undercovernc")
            .with_inner_size([1080.0, 740.0])
            .with_min_inner_size([960.0, 680.0]),
        event_loop_builder: Some(Box::new(|builder| {
            // O winit não recebe arquivos arrastados no Wayland; pelo XWayland o arrastar funciona.
            // UNDERCOVERNC_WAYLAND=1 força o Wayland nativo.
            let has_x = std::env::var_os("DISPLAY").is_some();
            if has_x && std::env::var_os("UNDERCOVERNC_WAYLAND").is_none() {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }
        })),
        ..Default::default()
    };
    eframe::run_native("UnderCoverNc", options, Box::new(move |cc| Ok(Box::new(gui::App::new(cc, files)))))
}
