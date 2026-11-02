#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
mod ui;
fn main() {
    #[cfg(windows)]
    {
        use rusted_launcher::startup;
        if let Ok(root) = startup::data_root(std::env::args_os().skip(1)) {
            startup::initialize(&root);
        }
        std::panic::set_hook(Box::new(|panic| {
            // Deliberately omit the panic payload: it might contain private data.
            let location = panic
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_default();
            startup::record(&format!("Erreur interne : {location}"));
        }));
        if let Err(error) = ui::run() {
            startup::record(&format!("Echec du demarrage : {error}"));
            let message = if let Some(path) = startup::journal_path() {
                format!("{error}\n\nJournal de demarrage : {}", path.display())
            } else {
                error
            };
            ui::error_box(&message);
            std::process::exit(1);
        }
        startup::record("Fermeture normale.");
    }
    #[cfg(not(windows))]
    {
        println!("Rusted Launcher {} : interface Windows x64. Le backend peut etre verifie avec cargo test -p rusted-launcher.",rusted_launcher::model::VERSION);
    }
}
