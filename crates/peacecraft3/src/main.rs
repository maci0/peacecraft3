// A browser has no `std::env::args` and no process to exit, so it does not come
// through here: wasm32 boots from `browser::start` instead.
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
use peacecraft3::browser_read;
#[cfg(any(test, target_arch = "wasm32"))]
mod import_assets;

mod button_actions;
mod fdf;
mod glue_menu;
#[cfg(test)]
mod hud;
mod intro;
mod launch;
mod launch_gui;
mod map_source_controls;
use peacecraft3::map_requests;
mod local_demo;
mod mainmenu;
mod mainmenu_scene;
mod menu_audio;
mod menu_panel;
mod menu_skin;
mod menu_sprites;
mod menu_text;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // A CLI path for staging and CI; with no arguments the window opens.
        Some("launch") => std::process::exit(launch::main_with(&args[1..])),
        // The M2 gate, as a window rather than a test: it is meant to be looked at.
        Some("view") => {
            let rest: Vec<String> = args[1..].to_vec();
            let install = pcx_gfx::viewer::resolve_install(
                rest.first()
                    .filter(|s| !s.contains('\\') && !s.ends_with(".mdx"))
                    .map(String::as_str),
            );
            let model = rest
                .iter()
                .find(|s| {
                    s.to_ascii_lowercase().ends_with(".mdx")
                        || s.to_ascii_lowercase().ends_with(".mdl")
                })
                .cloned()
                .unwrap_or_else(|| "Units\\Human\\Footman\\Footman.mdx".to_string());
            if let Err(e) = pcx_gfx::viewer::run(&install, &model) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        _ => {
            launch_gui::app(launch_gui::window_plugin(None), |_| {})
                .add_plugins(pcx_gfx::performance::PerformanceOverlayPlugin)
                .run();
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/browser_font_contract.rs"]
mod browser_font_contract;
