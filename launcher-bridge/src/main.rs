#![cfg_attr(windows, windows_subsystem = "windows")]
use rusted_launch::{game_args, preparation, read_prism, Config, Launch, Mode, Result};
use std::{io::Write, path::Path, process::Command};

fn run() -> Result<i32> {
    let mut args = std::env::args().skip(1).peekable();
    if matches!(
        args.peek().map(String::as_str),
        Some("--setup" | "--configure-prism")
    ) {
        let explicit = args.next().as_deref() == Some("--configure-prism");
        if args.next().is_some() {
            return Err("La configuration automatique ne prend aucun chemin en argument.".into());
        }
        let executable =
            std::env::current_exe().map_err(|_| "Emplacement du runtime inaccessible.")?;
        if rusted_launch::setup::instance_config(&executable)?.is_some() {
            rusted_launch::setup::wait_for_prism()?;
            let result = rusted_launch::setup::install(&executable)?;
            println!("Rusted : instance Prism configuree automatiquement. Rouvrez Prism et cliquez sur Jouer.");
            if let Some(backup) = result.backup {
                println!("Sauvegarde des anciens reglages : {}", backup.display());
            }
            return Ok(0);
        }
        if explicit {
            return Err("Aucune instance Prism trouvee autour du runtime ; placer rusted dans minecraft/rusted.".into());
        }
        let command = rusted_launch::wrapper_command(&executable)?;
        println!("{command}");
        copy_wrapper(&command)?;
        println!("Rusted : instance hors Prism ; commande copiee pour le Wrapper de Modrinth.");
        return Ok(0);
    }
    if matches!(
        args.peek().map(String::as_str),
        Some("--help" | "-h") | None
    ) {
        println!("rusted-launch -- JAVA [JVM/GAME ARGUMENTS]\nrusted-launch --config FILE [--check] -- JAVA [JVM/GAME ARGUMENTS]\nrusted-launch --config FILE --prepare --minecraft-dir INSTANCE --java JAVA\nrusted-launch --setup | --configure-prism\nrusted-launch --print-wrapper | --copy-wrapper\nSans --config, rusted-runtime.json est lu a cote du programme. Wrapper Prism/Modrinth : moteur Rusted 26.3 ou commande Java conservee.");
        return Ok(0);
    }
    if matches!(
        args.peek().map(String::as_str),
        Some("--print-wrapper" | "--copy-wrapper")
    ) {
        let copy = args.next().as_deref() == Some("--copy-wrapper");
        if args.next().is_some() {
            return Err("Cette commande n'attend aucun argument supplementaire.".into());
        }
        let executable =
            std::env::current_exe().map_err(|_| "Emplacement du programme inaccessible.")?;
        let command = rusted_launch::wrapper_command(&executable)?;
        println!("{command}");
        if copy {
            copy_wrapper(&command)?;
            eprintln!("Rusted : commande copiee. Collez-la dans les commandes personnalisees de Prism, champ Wrapper.");
        }
        return Ok(0);
    }
    if args.peek().map(String::as_str) == Some("--bootstrap") {
        args.next();
        if args.next().is_some() {
            return Err("Le bootstrap ne prend aucun argument public.".into());
        }
        let executable = std::env::current_exe().map_err(|_| "Runtime inaccessible.")?;
        let config = Config::load(&executable.with_file_name("rusted-runtime.json"))?;
        let input = rusted_launch::bootstrap::read(std::io::stdin().lock())?;
        let Some(native) =
            rusted_launch::bootstrap::prepare(input, &config, executable.parent().unwrap())?
        else {
            println!("RUSTED_BOOTSTRAP_JAVA");
            std::io::stdout()
                .flush()
                .map_err(|_| "Pipe de controle fermee.")?;
            return Ok(0);
        };
        println!("RUSTED_BOOTSTRAP_NATIVE");
        std::io::stdout()
            .flush()
            .map_err(|_| "Pipe de controle fermee.")?;
        rusted_launch::bootstrap::watch_parent(native.input.parent_pid)?;
        let java = native
            .input
            .java
            .to_str()
            .ok_or("Chemin Java non Unicode.")?;
        return launch_native(
            &config,
            native.launch,
            &native.raw,
            java,
            Some(&native.bootstrap_jar),
        );
    }
    let path = if args.peek().map(String::as_str) == Some("--config") {
        args.next();
        std::path::PathBuf::from(args.next().ok_or("Fichier de configuration manquant.")?)
    } else if args.peek().map(String::as_str) == Some("--") {
        std::env::current_exe()
            .map_err(|_| "Emplacement du programme inaccessible.")?
            .with_file_name("rusted-runtime.json")
    } else {
        return Err("Utiliser -- JAVA ... ou --config FILE -- JAVA ...".into());
    };
    let config = Config::load(&path)?;
    let next = args.next();
    if next.as_deref() == Some("--prepare") {
        let mut minecraft = None;
        let mut java = None;
        while let Some(flag) = args.next() {
            let value = args.next().ok_or("Option de preparation sans valeur.")?;
            match flag.as_str() {
                "--minecraft-dir" => minecraft = Some(value),
                "--java" => java = Some(value),
                _ => return Err("Option de preparation inconnue.".into()),
            }
        }
        let minecraft =
            minecraft.ok_or("Utiliser --minecraft-dir INSTANCE pour la preparation.")?;
        let java = java.ok_or("Utiliser --java CHEMIN_JAVA_25 pour la preparation.")?;
        config.check_integrity()?;
        preparation::prepare(&config, Path::new(&minecraft), Path::new(&java), true, None)?;
        return Ok(0);
    }
    if next.as_deref() == Some("--check") && args.next().is_none() {
        if config.mode != Mode::Java {
            config.check_integrity()?;
            config.check_native()?;
        }
        println!("Configuration valide. Cela ne valide pas encore un lancement du jeu ou la connexion Microsoft.");
        return Ok(0);
    }
    if next.as_deref() != Some("--") {
        return Err("Separateur -- manquant avant la commande Java du lanceur.".into());
    }
    let raw: Vec<String> = args.collect();
    let java = raw
        .first()
        .ok_or("Le lanceur n'a pas fourni sa commande Java.")?;
    let cwd = std::env::current_dir().map_err(|_| "Dossier courant inaccessible.")?;
    let instance = std::env::var_os("INST_MC_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or(cwd);
    let route_java = config.mode == Mode::Java
        || (config.mode == Mode::Auto && rusted_launch::routing::java_loader(&raw, &instance)?);
    if route_java {
        eprintln!("Rusted : mode de compatibilite Java ; le lanceur et son chargeur executent les mods. Le moteur natif Rust est inactif pour cette partie.");
        let mut command = Command::new(java);
        command.args(&raw[1..]);
        return execute(command, None);
    }
    let params = if raw.iter().any(|a| a == "org.prismlauncher.EntryPoint") {
        match read_prism(&mut std::io::stdin().lock())? {
            Some(p) => p,
            None => return Ok(0),
        }
    } else {
        game_args(&raw)?.to_vec()
    };
    let launch = Launch::from_game_args(
        &params,
        &std::env::current_dir().map_err(|_| "Dossier courant inaccessible.")?,
    )?;
    launch_native(&config, launch, &raw, java, None)
}

fn launch_native(
    config: &Config,
    launch: Launch,
    raw: &[String],
    java: &str,
    bootstrap: Option<&Path>,
) -> Result<i32> {
    launch.validate_native_except(bootstrap)?;
    config.check_integrity()?;
    if config.preparation.is_some() {
        let imported = if config
            .preparation
            .as_ref()
            .is_some_and(|p| p.import_launcher)
            && !preparation::ready(config)
        {
            Some(rusted_launch::launcher_install::LauncherFiles::from_args(
                raw,
                launch.assets_directory.as_deref(),
            )?)
        } else {
            None
        };
        preparation::prepare(
            config,
            &launch.game_directory,
            Path::new(java),
            false,
            imported.as_ref(),
        )?;
    }
    config.check_native()?;
    std::fs::create_dir_all(&config.data_directory)
        .map_err(|_| "Impossible de creer le dossier natif isole.")?;
    let home =
        std::fs::canonicalize(&config.data_directory).map_err(|_| "Dossier natif inaccessible.")?;
    if home == launch.game_directory || home.starts_with(launch.game_directory.join("saves")) {
        return Err(
            "Le dossier natif doit etre distinct du dossier de jeu Java et de ses sauvegardes."
                .into(),
        );
    }
    let marker = home.join(".rusted-native-instance.json");
    if marker.exists() {
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&marker).map_err(|_| "Marqueur de l'instance native illisible.")?,
        )
        .map_err(|_| "Marqueur de l'instance native invalide.")?;
        if value.get("horizon_revision").and_then(|v| v.as_str())
            != Some(rusted_launch::HORIZON_REVISION)
        {
            return Err("Cette instance native appartient a une autre revision.".into());
        }
    } else {
        if std::fs::read_dir(&home)
            .map_err(|_| "Dossier natif illisible.")?
            .next()
            .is_some()
        {
            return Err("Choisir un dossier natif neuf : ce dossier contient deja des donnees sans marqueur Rusted.".into());
        }
        std::fs::write(
            marker,
            serde_json::json!({"schema": 1, "horizon_revision": rusted_launch::HORIZON_REVISION})
                .to_string(),
        )
        .map_err(|_| "Impossible de marquer le dossier natif.".to_owned())?;
    }
    // Horizon consumes and removes this private one-shot file on startup. The
    // credential never appears in the child command line or our diagnostics.
    let mut session = tempfile::Builder::new()
        .prefix("rusted-session-")
        .tempfile()
        .map_err(|_| "Impossible de creer le fichier de session prive.")?;
    let value = serde_json::json!({ "name": launch.name, "uuid": launch.uuid, "access_token": launch.access_token, "xuid": launch.xuid, "user_type": "msa" });
    serde_json::to_writer(session.as_file_mut(), &value)
        .map_err(|_| "Ecriture de la session impossible.")?;
    session
        .flush()
        .map_err(|_| "Ecriture de la session impossible.")?;
    let mut command = Command::new(&config.executable);
    command
        .current_dir(&home)
        .env("RUSTED_HOME", &home)
        .env("RUSTED_INSTANCE_DIR", &launch.game_directory)
        .env("HORIZON_HOME", &home)
        .env("HORIZON_SESSION_FILE", session.path())
        .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
        .env("HORIZON_LAUNCHER_HOME", home.join("launcher"))
        .env("XDG_DATA_HOME", home.join("xdg-data"))
        .env("XDG_CONFIG_HOME", home.join("xdg-config"))
        // INST_JAVA_ARGS can contain session data supplied by a launcher.
        .env_remove("INST_JAVA_ARGS")
        .args(["--resources".as_ref(), config.resources.as_os_str()])
        .args(&launch.native_args);
    eprintln!("Rusted : lancement du moteur natif ; sauvegardes dans son dossier isole.");
    execute(command, Some(session.into_temp_path()))
}

// Unix exec preserves the PID the launcher's Stop button owns. Windows uses
// a kill-on-close job so killing the wrapper also ends its game process.
fn execute(mut command: Command, session: Option<tempfile::TempPath>) -> Result<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Transfer cleanup to Horizon (it reads then deletes the file). On an
        // exec error TempPath still removes it before returning.
        let error = command.exec();
        drop(session);
        let _ = error;
        Err("Impossible de demarrer le programme configure.".into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        let status = command
            .status()
            .map_err(|_| "Impossible de demarrer le programme configure.")?;
        drop(session);
        Ok(status.code().unwrap_or(1))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (&mut command, session);
        Err("Plateforme non prise en charge.".into())
    }
}

fn main() {
    #[cfg(windows)]
    if let Err(message) = protect_windows_process_tree() {
        eprintln!("Rusted : {message}");
        std::process::exit(1);
    }
    match run() {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("Rusted : {message}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(windows))]
fn copy_wrapper(_: &str) -> Result<()> {
    Err("Copie automatique disponible sous Windows ; la commande est affichee ci-dessus.".into())
}

#[cfg(windows)]
fn copy_wrapper(text: &str) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::{GlobalFree, HWND},
        System::{DataExchange::*, Memory::*},
        UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE},
    };
    struct ClipboardOwner(HWND);
    impl Drop for ClipboardOwner {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns the successfully created hidden window.
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }
    let value: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: allocate enough space for the terminated UTF-16 string, check every
    // handle, and transfer memory ownership only after SetClipboardData succeeds.
    unsafe {
        // A message-only window supplies an owner even without a console HWND.
        let class = [83u16, 84, 65, 84, 73, 67, 0]; // predefined STATIC class
        let owner = CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if owner.is_null() {
            return Err("Copie indisponible ; copier la commande affichee ci-dessus.".into());
        }
        let _owner = ClipboardOwner(owner);
        let memory = GlobalAlloc(GMEM_MOVEABLE, value.len() * std::mem::size_of::<u16>());
        if memory.is_null() {
            return Err("Memoire insuffisante pour copier la commande.".into());
        }
        let buffer = GlobalLock(memory).cast::<u16>();
        if buffer.is_null() {
            GlobalFree(memory);
            return Err("Impossible de preparer la commande pour le presse-papiers.".into());
        }
        std::ptr::copy_nonoverlapping(value.as_ptr(), buffer, value.len());
        GlobalUnlock(memory);
        if OpenClipboard(owner) == 0 {
            GlobalFree(memory);
            return Err("Presse-papiers occupe ; reessayer ou copier la commande affichee.".into());
        }
        const CF_UNICODETEXT: u32 = 13;
        let copied = EmptyClipboard() != 0 && !SetClipboardData(CF_UNICODETEXT, memory).is_null();
        CloseClipboard();
        if !copied {
            GlobalFree(memory);
            return Err("Copie impossible ; copier la commande affichee ci-dessus.".into());
        }
    }
    Ok(())
}

#[cfg(windows)]
fn protect_windows_process_tree() -> Result<()> {
    use std::{
        mem::{size_of, zeroed},
        ptr::null,
    };
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{JobObjects::*, Threading::GetCurrentProcess},
    };
    // The job owns the wrapper before any subprocess starts. All descendants
    // (including preparer -> Java) inherit it. Its single handle intentionally
    // stays open until process exit; closing it earlier would kill this process.
    // SAFETY: pointers have the Win32 layouts; handles/results are checked.
    unsafe {
        let job = CreateJobObjectW(null(), null());
        if job.is_null() {
            return Err("Impossible de creer le groupe du lanceur.".into());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as _,
        ) == 0
            || AssignProcessToJobObject(job, GetCurrentProcess()) == 0
        {
            CloseHandle(job);
            return Err("Impossible de proteger l'arbre des processus du lanceur.".into());
        }
    }
    Ok(())
}
