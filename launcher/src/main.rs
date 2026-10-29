#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod ui;
fn main() {
    #[cfg(windows)]
    {
        if let Err(error) = ui::run() {
            ui::error_box(&error);
        }
    }
    #[cfg(not(windows))]
    {
        println!("Rusted Launcher {} : interface Windows x64. Le backend peut etre verifie avec cargo test -p rusted-launcher.",rusted_launcher::model::VERSION);
    }
}
