use app::XApp;

use sdl2::{libc, log::log};

mod app;

#[no_mangle]
pub extern "C" fn SDL_main(_argc: libc::c_int, _argv: *const *const libc::c_char) -> libc::c_int {
    let game = XApp::new();

    if let Some(e) = game.init("WGPU Game").err() {
        log(&format!("Error on init XApp: {}", e));
        return 1;
    }

    if let Some(e) = game.run().err() {
        log(&format!("Error on run XApp: {}", e));
        return 2;
    }

    0
}
