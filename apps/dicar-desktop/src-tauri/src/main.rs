#![cfg_attr(
    all(not(debug_assertions), target_env = "msvc"),
    windows_subsystem = "windows"
)]

#[cfg(target_env = "msvc")]
fn main() {
    dicar_desktop_lib::run();
}

#[cfg(not(target_env = "msvc"))]
fn main() {
    eprintln!("DiCar Tauri shell requires the Windows MSVC target");
}
