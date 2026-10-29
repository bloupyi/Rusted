//! Original Rusted launch protocol for an imported Fabric pack. No launcher settings are edited.
use crate::{Config, Launch, Mode, Result, MINECRAFT_VERSION};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub schema: u32,
    pub parent_pid: u32,
    pub java: PathBuf,
    pub minecraft_version: String,
    pub game_directory: PathBuf,
    pub classpath: String,
    pub game_args: Vec<String>,
}
pub struct Native {
    pub input: Input,
    pub launch: Launch,
    pub raw: Vec<String>,
    pub bootstrap_jar: PathBuf,
}

pub fn read(reader: impl Read) -> Result<Input> {
    let mut bytes = Vec::new();
    reader
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Lecture du demarrage automatique impossible.")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Demarrage automatique trop volumineux.".into());
    }
    let input: Input = serde_json::from_slice(&bytes)
        .map_err(|_| "Protocole de demarrage automatique invalide.")?;
    if input.schema != 1 || input.parent_pid == 0 || input.game_args.len() > 4096 {
        return Err("Protocole de demarrage automatique non pris en charge.".into());
    }
    Ok(input)
}

pub fn prepare(mut input: Input, config: &Config, runtime: &Path) -> Result<Option<Native>> {
    config.check_integrity()?;
    #[derive(Deserialize)]
    struct Record {
        schema: u32,
        jar_sha256: String,
    }
    let body = fs::read(runtime.join("bootstrap.json"))
        .map_err(|_| "Bootstrap absent ; reimporter le pack complet.")?;
    if body.len() > 4096 {
        return Err("Metadata du bootstrap trop volumineuses.".into());
    }
    let record: Record =
        serde_json::from_slice(&body).map_err(|_| "Metadata du bootstrap invalides.")?;
    if record.schema != 1 || record.jar_sha256.len() != 64 {
        return Err("Version du bootstrap invalide.".into());
    }
    let game =
        fs::canonicalize(&input.game_directory).map_err(|_| "Dossier du pack inaccessible.")?;
    let bootstrap_jar = game.join("mods/rusted-bootstrap.jar");
    let file = fs::File::open(&bootstrap_jar).map_err(|_| "Module de demarrage absent du pack.")?;
    let mut bytes = Vec::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Module de demarrage illisible.")?;
    if bytes.len() > 8 * 1024 * 1024 || format!("{:x}", Sha256::digest(&bytes)) != record.jar_sha256
    {
        return Err("Module de demarrage modifie ou incomplet ; reimporter le pack.".into());
    }
    let bootstrap_jar =
        fs::canonicalize(bootstrap_jar).map_err(|_| "Module de demarrage inaccessible.")?;
    let other_mods = fs::read_dir(game.join("mods"))
        .map_err(|_| "Dossier mods inaccessible.")?
        .map(|entry| {
            entry
                .map_err(|_| "Dossier mods illisible.".to_owned())
                .map(|e| e.path())
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .any(|p| {
            p.extension().is_some_and(|s| s.eq_ignore_ascii_case("jar"))
                && fs::canonicalize(p).map_or(true, |p| p != bootstrap_jar)
        });
    if config.mode == Mode::Java || (config.mode == Mode::Auto && other_mods) {
        eprintln!(
            "Rusted : poursuite du jeu Java/Fabric pour les mods installes ; moteur natif inactif."
        );
        return Ok(None);
    }
    if input.minecraft_version != MINECRAFT_VERSION {
        return Err("Le moteur natif de ce pack cible Minecraft 26.3.".into());
    }
    // Fabric profile names differ by launcher. The public Minecraft mod metadata
    // supplies the actual game version; installed client hashes are still pinned.
    let mut count = 0;
    for i in 0..input.game_args.len().saturating_sub(1) {
        if input.game_args[i] == "--version" {
            input.game_args[i + 1] = MINECRAFT_VERSION.into();
            count += 1;
        }
    }
    if count != 1 {
        return Err("Version absente ou dupliquee dans le demarrage automatique.".into());
    }
    let launch = Launch::from_game_args(&input.game_args, &game)?;
    if launch.game_directory != game {
        return Err("Dossiers de lancement incoherents.".into());
    }
    let mut raw = vec![
        input.java.to_string_lossy().into_owned(),
        "-cp".into(),
        input.classpath.clone(),
        "net.minecraft.client.main.Main".into(),
    ];
    raw.extend(input.game_args.clone());
    Ok(Some(Native {
        input,
        launch,
        raw,
        bootstrap_jar,
    }))
}

/// The launcher's Stop button owns the JVM. If it is killed without running its
/// shutdown hook, terminate the native child tree as well.
pub fn watch_parent(pid: u32) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: prctl sets a signal on this process only; getppid verifies the
        // parent has not already exited between the JVM spawn and this call.
        unsafe {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0
                || libc::getppid() as u32 != pid
            {
                return Err("Le lanceur Java s'est arrete avant le demarrage natif.".into());
            }
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::*};
        // SAFETY: only request synchronization rights, never access the JVM's
        // memory or token. The owned handle is closed after waiting.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return Err("Le lanceur Java n'est plus accessible.".into());
        }
        let value = handle as usize;
        std::thread::spawn(move || unsafe {
            WaitForSingleObject(value as _, INFINITE);
            CloseHandle(value as _);
            // The wrapper's kill-on-close Windows job also stops the game and preparer.
            std::process::exit(1);
        });
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = pid;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HORIZON_REVISION;
    fn fixture(root: &Path, mode: &str) -> (Input, Config) {
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(
            root.join("mods/rusted-bootstrap.jar"),
            b"OWN_BOOTSTRAP_FIXTURE",
        )
        .unwrap();
        fs::write(root.join("bootstrap.json"), serde_json::json!({"schema":1,"jar_sha256":format!("{:x}",Sha256::digest(b"OWN_BOOTSTRAP_FIXTURE"))}).to_string()).unwrap();
        let path = root.join("rusted-runtime.json");
        fs::write(&path,serde_json::json!({"schema":1,"mode":mode,"horizon_revision":HORIZON_REVISION,"minecraft_version":"26.3","executable":"own-test-peer","resources":"resources","data_directory":"native"}).to_string()).unwrap();
        let args = vec![
            "--username".into(),
            "BridgeFixture".into(),
            "--uuid".into(),
            "123456781234123412341234567890ab".into(),
            "--accessToken".into(),
            "TEST_ONLY_NOT_A_REAL_SESSION_TOKEN".into(),
            "--version".into(),
            "fabric-loader-0.20.1-26.3".into(),
            "--gameDir".into(),
            root.to_str().unwrap().into(),
        ];
        (
            Input {
                schema: 1,
                parent_pid: 1,
                java: "java".into(),
                minecraft_version: "26.3".into(),
                game_directory: root.into(),
                classpath: "test cp".into(),
                game_args: args,
            },
            Config::load(&path).unwrap(),
        )
    }
    #[test]
    fn imported_native_pack_verifies_bootstrap_and_normalizes_fabric_profile_names() {
        let d = tempfile::tempdir().unwrap();
        let (input, config) = fixture(d.path(), "auto");
        let native = prepare(input, &config, d.path()).unwrap().unwrap();
        assert_eq!(native.launch.version, "26.3");
        native
            .launch
            .validate_native_except(Some(&native.bootstrap_jar))
            .unwrap();
        assert!(native.launch.validate_native().is_err());
        fs::write(d.path().join("mods/rusted-bootstrap.jar"), b"TAMPERED").unwrap();
        let mut input = native.input;
        input.game_directory = d.path().into();
        assert!(prepare(input, &config, d.path()).is_err());
    }
    #[test]
    fn other_mods_keep_original_jvm_without_native_session_or_data() {
        let d = tempfile::tempdir().unwrap();
        let (input, config) = fixture(d.path(), "auto");
        fs::write(d.path().join("mods/another.jar"), b"OWN_OTHER_MOD_FIXTURE").unwrap();
        assert!(prepare(input, &config, d.path()).unwrap().is_none());
        assert!(!d.path().join("native").exists());
        let (input, config) = fixture(d.path(), "java");
        assert!(prepare(input, &config, d.path()).unwrap().is_none());
    }
    #[test]
    fn protocol_errors_never_echo_credentials_and_wrong_version_is_refused() {
        let secret = "TEST_ONLY_NOT_A_REAL_SESSION_TOKEN";
        assert!(!read(format!("{{{secret}").as_bytes())
            .err()
            .unwrap()
            .contains(secret));
        assert!(read(vec![b' '; 2 * 1024 * 1024 + 1].as_slice()).is_err());
        let d = tempfile::tempdir().unwrap();
        let (mut input, config) = fixture(d.path(), "auto");
        input.minecraft_version = "1.21.1".into();
        assert!(prepare(input, &config, d.path()).is_err());
    }
}
