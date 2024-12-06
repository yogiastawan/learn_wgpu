use std::process::exit;

use app::XApp;

use sdl2::log::log;
mod app;

fn main() {
    let game = match XApp::new("WGPU Game") {
        Ok(x) => x,
        Err(e) => {
            log(&format!("Error on init XApp: {}", e));
            exit(1);
        }
    };

    if let Some(e) = game.run().err() {
        log(&format!("Error on run XApp: {}", e));
        exit(2);
    }
}
