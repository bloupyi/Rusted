//! Rusted: native client derived from Horizon, with its own launcher integration.
#![forbid(unsafe_code)]
mod game;

use horizon_client::{crash, logging};

/// The mod API level this game serves, kept in the program for the launcher
/// to read without running it (`horizon_mods::api_level::MARK`). `main`
/// reads it, so the linker keeps it.
static MOD_API_LEVEL: [u8; 27] = horizon_mods::api_level::MARK;

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() == Some(std::ffi::OsStr::new("--inspect-mods")) {
        let Some(directory) = arguments.next() else {
            eprintln!("Rusted: --inspect-mods requires a mods directory");
            std::process::exit(2);
        };
        let report = rusted_platform::scan(std::path::Path::new(&directory));
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("serializable inventory")
        );
        return;
    }
    std::hint::black_box(&MOD_API_LEVEL);
    horizon_client::telemetry::mark_start();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let game_args = args.iter().take_while(|a| *a != "--dedicated");
    let help = game_args.clone().any(|a| a == "--help" || a == "-h");
    // The dedicated server logs in its own folder, as vanilla's does; the game in its home.
    let (home, side, thread) = if args.iter().any(|a| a == "--dedicated") {
        (std::path::PathBuf::from("."), "server", "Server thread")
    } else {
        (logging::game_home(), "client", "Render thread")
    };
    if !help {
        logging::init(&home, thread);
        // The log's first line names the build, so a bug report that carries the log says which one it was.
        log::info!("{}", horizon_client::rusted::title());
        log::info!("{}", horizon_client::rusted::CREDITS);
        crash::install(home.clone(), side);
    }
    if let Err(error) = game::run() {
        log::error!("Rusted: {error}");
        if !help {
            crash::report_error(&home, side, &*error);
        }
        // Started from Explorer, the console window closes with the game:
        // keep the message on screen until it is read.
        #[cfg(windows)]
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            eprintln!("Press Enter to close.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
    logging::flush();
}
