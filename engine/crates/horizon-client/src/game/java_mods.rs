//! Opt-in Java entrypoints, with Rust retaining the game loop, world and renderer.
use super::{Game, Options};
#[cfg(feature = "mods")]
static RUNTIME: std::sync::Mutex<Option<rusted_java::Runtime>> = std::sync::Mutex::new(None);

pub(super) fn prepare(options: &Options) {
    #[cfg(not(feature = "mods"))]
    let _ = options;
    #[cfg(feature = "mods")]
    {
        if !options.execution_role.is_client()
            || std::env::var("RUSTED_JAVA_MODS").as_deref() != Ok("1")
        {
            return;
        }
        let (modules, errors) = rusted_java::discover(&horizon_client::rusted::mods_directory());
        for error in errors {
            report(format!("Java mod not loaded: {error}"));
        }
        if modules.is_empty() {
            return;
        }
        let result = (|| {
            let java = std::env::var_os("RUSTED_JAVA_HOME")
                .or_else(|| std::env::var_os("JAVA_HOME"))
                .map(std::path::PathBuf::from)
                .ok_or("Java runtime location is missing")?;
            let home = std::env::current_dir().map_err(|e| e.to_string())?;
            let libraries = if let Some(path) = std::env::var_os("RUSTED_JAVA_LIBRARIES_FILE") {
                let path = std::path::PathBuf::from(path);
                if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
                    return Err("Java library manifest too large".into());
                }
                let paths: Vec<std::path::PathBuf> =
                    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if paths.len() > 512 || paths.iter().any(|p| !p.is_absolute() || !p.is_file()) {
                    return Err("Invalid Java library manifest".into());
                }
                paths
            } else {
                Vec::new()
            };
            let runtime = rusted_java::Runtime::load(
                &java,
                &home,
                &home.join(".rusted-cache/java"),
                &modules,
                &libraries,
            )?;
            *RUNTIME.lock().unwrap_or_else(|e| e.into_inner()) = Some(runtime);
            report(format!(
                "Java entrypoints initialized for {} module(s); native world and renderer retained",
                modules.len()
            ));
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            report(format!("Java runtime unavailable: {error}"));
        }
    }
}
#[cfg(feature = "mods")]
fn report(line: String) {
    log::warn!("Rusted: {line}");
    horizon_client::rusted::record_java_status(line);
}
impl Game<'_> {
    pub(super) fn tick_java_mods(&mut self) {
        #[cfg(feature = "mods")]
        {
            if !self.options.execution_role.is_client() {
                return;
            }
            let result = {
                let mut runtime = RUNTIME.lock().unwrap_or_else(|e| e.into_inner());
                let Some(java) = runtime.as_mut() else {
                    return;
                };
                let health = self.player.as_ref().map_or(0.0, |p| p.health.health());
                match java.tick(self.world.game_time, health, self.player.is_some()) {
                    Ok(actions) => Ok(actions),
                    Err(error) => {
                        *runtime = None;
                        Err(error)
                    }
                }
            };
            match result {
                Ok(actions) => {
                    for action in actions {
                        match action {
                            rusted_java::Action::Chat { module, text } => {
                                self.add_mod_chat_line(horizon_client::text::Text::literal(
                                    format!("[{module}] {text}"),
                                ));
                            }
                            rusted_java::Action::Error(error) => {
                                report(format!("Java mod stopped: {error}"))
                            }
                        }
                    }
                }
                Err(error) => report(format!("Java runtime stopped: {error}")),
            }
        }
    }
}
