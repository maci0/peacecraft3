// A browser has no `std::env::args` and no process to exit, so it does not come
// through here: wasm32 boots from `browser::start` instead.
#[cfg(target_arch = "wasm32")]
pub mod browser;
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
#[cfg(all(test, not(target_arch = "wasm32")))]
mod menu_colour_gpu_tests;
mod menu_fog;
mod menu_material;
mod menu_panel;
mod menu_particle_order;
mod menu_screens;
mod menu_settings;
mod menu_skin;
mod menu_sprites;
mod menu_text;
#[cfg(not(target_arch = "wasm32"))]
mod startup_window;
mod viewer_args;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // A CLI path for staging and CI; with no arguments the window opens.
        Some("launch") => std::process::exit(launch::main_with(&args[1..])),
        // Explicit model preview with an optional caller-selected terrain tile.
        Some("view") => {
            let request = match viewer_args::ViewerArgs::parse(&args[1..]) {
                Ok(request) => request,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            };
            let install = pcx_gfx::viewer::resolve_install(request.install.as_deref());
            if let Err(e) =
                pcx_gfx::viewer::run_with_terrain(&install, &request.model, request.tile)
            {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        None => {
            launch_gui::app(launch_gui::window_plugin(None), |_| {})
                .add_plugins(pcx_gfx::performance::PerformanceOverlayPlugin)
                .run();
        }
        Some(other) => {
            eprintln!("unknown command {other:?}");
            eprintln!(
                "usage: peacecraft3 [launch <sources|plan|stage> ... | view [INSTALL] MODEL [--tile FOURCC]]"
            );
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
#[path = "../tests/support/browser_font_contract.rs"]
mod browser_font_contract;

#[cfg(all(test, target_arch = "wasm32"))]
#[path = "../tests/support/map_admission_cases.rs"]
mod admission;
#[cfg(all(test, target_arch = "wasm32"))]
#[path = "../tests/support/map_binding_cases.rs"]
mod binding;
#[cfg(all(test, target_arch = "wasm32"))]
#[path = "../tests/support/map_fixtures.rs"]
mod fixtures;
#[cfg(all(test, target_arch = "wasm32"))]
use crate::map_source_controls as source_controls;
#[cfg(all(test, target_arch = "wasm32"))]
#[path = "../tests/support/browser_application_gate.rs"]
mod browser_application_gate;
