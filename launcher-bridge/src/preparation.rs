//! First-run preparation from the launcher's installed Minecraft files.
//! The public server artifact is downloaded only when the configuration opts in.
use crate::{Config, Result, MINECRAFT_VERSION};
use fs2::FileExt;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

pub const SERVER_SHA1: &str = "33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c";
pub const SERVER_URL: &str =
    "https://piston-data.mojang.com/v1/objects/33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c/server.jar";
const MAX_SERVER_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preparation {
    pub root: PathBuf,
    pub executable: PathBuf,
    #[serde(default)]
    pub server_jar: Option<PathBuf>,
    #[serde(default)]
    pub minecraft_directory: Option<PathBuf>,
    #[serde(default)]
    pub download_server: bool,
    #[serde(default)]
    pub import_launcher: bool,
}

impl Preparation {
    pub fn resolve(&mut self, base: &Path) {
        for path in [&mut self.root, &mut self.executable] {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        }
        for path in [&mut self.server_jar, &mut self.minecraft_directory]
            .into_iter()
            .flatten()
        {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        }
    }
}

fn checksum(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|_| "JAR serveur illisible.")?;
    let mut hash = Sha1::new();
    let mut block = [0u8; 65536];
    let mut size = 0;
    loop {
        let n = file
            .read(&mut block)
            .map_err(|_| "Lecture du JAR serveur impossible.")?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > MAX_SERVER_BYTES {
            return Err("JAR serveur trop volumineux.".into());
        }
        hash.update(&block[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn verified_server(preparation: &Preparation) -> Result<PathBuf> {
    if let Some(path) = &preparation.server_jar {
        if checksum(path)? != SERVER_SHA1 {
            return Err("L'empreinte du serveur ne correspond pas a Minecraft 26.3.".into());
        }
        return Ok(path.clone());
    }
    let path = preparation.root.join(".local/server-download/server.jar");
    if path.is_file() && checksum(&path)? == SERVER_SHA1 {
        return Ok(path);
    }
    if !preparation.download_server {
        return Err(format!("Serveur 26.3 absent. Renseigner preparation.server_jar, ou activer preparation.download_server pour le fichier officiel : {SERVER_URL}"));
    }
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent).map_err(|_| "Impossible de creer le cache serveur.")?;
    eprintln!("Rusted : telechargement du serveur officiel 26.3 pour la preparation locale (empreinte verifiee). Aucun serveur n'est demarre.");
    let response = ureq::AgentBuilder::new().try_proxy_from_env(true)
        .timeout_connect(Duration::from_secs(30)).timeout_read(Duration::from_secs(60))
        .timeout(Duration::from_secs(600)).user_agent("Rusted/0.2.0")
        .build().get(SERVER_URL).call()
        .map_err(|_| "Telechargement du serveur impossible ; verifier la connexion ou fournir preparation.server_jar.")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cache serveur inaccessible.")?;
    copy_verified(response.into_reader(), temporary.as_file_mut(), SERVER_SHA1)?;
    temporary
        .persist(&path)
        .map_err(|_| "Impossible de conserver le JAR serveur verifie.")?;
    Ok(path)
}

fn copy_verified(mut input: impl Read, output: &mut fs::File, expected: &str) -> Result<()> {
    let mut hash = Sha1::new();
    let mut buffer = [0u8; 65536];
    let mut bytes = 0u64;
    loop {
        let n = input
            .read(&mut buffer)
            .map_err(|_| "Telechargement du serveur interrompu.")?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > MAX_SERVER_BYTES {
            return Err("Telechargement du serveur trop volumineux.".into());
        }
        hash.update(&buffer[..n]);
        output
            .write_all(&buffer[..n])
            .map_err(|_| "Ecriture du cache serveur impossible.")?;
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err("Serveur telecharge corrompu : empreinte incorrecte, fichier refuse.".into());
    }
    output
        .sync_all()
        .map_err(|_| "Synchronisation du cache serveur impossible.")?;
    Ok(())
}

fn java_home(java: &Path) -> Option<PathBuf> {
    let actual = if java.is_absolute() {
        java.to_owned()
    } else if java.components().count() > 1 {
        std::env::current_dir().ok()?.join(java)
    } else {
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|dir| dir.join(java))
            .find(|p| p.is_file())?
    };
    // Do not canonicalize away Windows' drive spelling into a verbatim path;
    // the Java runtime's own launchers need normal Windows paths.
    actual.parent()?.parent().map(Path::to_owned)
}

/// Does not receive credentials. Preparation can run before any session file is created.
pub fn prepare(
    config: &Config,
    minecraft: &Path,
    java: &Path,
    repair: bool,
    launcher: Option<&crate::launcher_install::LauncherFiles>,
) -> Result<()> {
    if !repair && ready(config) {
        return Ok(());
    }
    let preparation = config.preparation.as_ref().ok_or("Ressources absentes : utiliser le pack natif complet ou configurer preparation dans le fichier JSON.")?;
    if !config.executable.is_file() || !preparation.executable.is_file() {
        return Err("Le pack est incomplet : moteur ou horizon-prepare absent.".into());
    }
    let lock: serde_json::Value = serde_json::from_slice(
        &fs::read(preparation.root.join("research-lock.json"))
            .map_err(|_| "Preparation absente du pack : research-lock.json introuvable.")?,
    )
    .map_err(|_| "research-lock.json invalide.")?;
    if lock["minecraft"]["version"] != MINECRAFT_VERSION
        || lock["minecraft"]["server_sha1"] != SERVER_SHA1
    {
        return Err("La preparation fournie ne correspond pas a Minecraft 26.3.".into());
    }
    let local = preparation.root.join(".local");
    fs::create_dir_all(&local).map_err(|_| "Le dossier du pack n'est pas accessible en ecriture ; extraire l'archive dans un dossier utilisateur.")?;
    let guard = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(local.join("rusted-prepare.lock"))
        .map_err(|_| "Verrou de preparation inaccessible.")?;
    guard
        .try_lock_exclusive()
        .map_err(|_| "Une preparation est deja en cours dans ce dossier ; attendre sa fin.")?;
    if !repair && ready(config) {
        return Ok(());
    }
    let _ = fs::remove_file(local.join("rusted-ready.json"));
    let server = verified_server(preparation)?;
    let imported;
    let minecraft = preparation
        .minecraft_directory
        .as_deref()
        .unwrap_or(minecraft);
    let minecraft = if preparation.import_launcher {
        imported = match launcher {
            Some(files) => crate::launcher_install::import(preparation, files)?,
            None => {
                let cached = preparation.root.join(".local/launcher-input");
                if !cached.join("versions/26.3/26.3.jar").is_file() {
                    return Err("Premiere preparation : utiliser le bouton Jouer de Prism/Modrinth pour identifier leurs fichiers installes.".into());
                }
                cached
            }
        };
        &imported
    } else {
        minecraft
    };
    if !minecraft.is_dir() {
        return Err(
            "Dossier Minecraft introuvable ; installer et lancer Minecraft 26.3 dans le lanceur."
                .into(),
        );
    }
    let mut command = Command::new(&preparation.executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    command
        .current_dir(&preparation.root)
        .env_remove("INST_JAVA_ARGS")
        .env_remove("HORIZON_SESSION_FILE")
        .args([
            "--root".as_ref(),
            preparation.root.as_os_str(),
            "--prepare".as_ref(),
            "--minecraft-dir".as_ref(),
            minecraft.as_os_str(),
            "--server-jar".as_ref(),
            server.as_os_str(),
        ]);
    if let Some(home) = java_home(java) {
        command.env("JAVA_HOME", home);
    }
    eprintln!("Rusted : premiere preparation des ressources depuis l'installation Minecraft 26.3 du lanceur. Cela peut prendre plusieurs minutes.");
    let status = command
        .status()
        .map_err(|_| "Impossible de demarrer horizon-prepare.")?;
    if !status.success() {
        return Err("Preparation inachevee. Consulter les messages precedents ; Minecraft 26.3 doit etre installe et Java 25 disponible. Relancer reprendra les fichiers verifies.".into());
    }
    // Sound is part of a normal player build, rather than an optional silent result.
    let mut sounds = Command::new(&preparation.executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        sounds.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    sounds
        .current_dir(&preparation.root)
        .env_remove("INST_JAVA_ARGS")
        .env_remove("HORIZON_SESSION_FILE")
        .args([
            "sounds".as_ref(),
            "--root".as_ref(),
            preparation.root.as_os_str(),
            "--minecraft-dir".as_ref(),
            minecraft.as_os_str(),
        ]);
    if let Some(home) = java_home(java) {
        sounds.env("JAVA_HOME", home);
    }
    if !sounds
        .status()
        .map_err(|_| "Impossible de preparer les sons.")?
        .success()
    {
        return Err("Preparation des sons incomplete ; lancer une fois Minecraft 26.3 depuis le lanceur puis reessayer --prepare.".into());
    }
    config.check_native()?;
    let marker = serde_json::json!({"schema": 1, "horizon_revision": config.horizon_revision,
        "resources_sha256": manifest_hash(config)?, "sounds": true});
    let mut saved = tempfile::NamedTempFile::new_in(&local)
        .map_err(|_| "Cache de preparation inaccessible.")?;
    serde_json::to_writer(saved.as_file_mut(), &marker)
        .map_err(|_| "Impossible de marquer la preparation terminee.")?;
    saved
        .persist(local.join("rusted-ready.json"))
        .map_err(|_| "Impossible de conserver le statut de preparation.")?;
    eprintln!("Rusted : ressources preparees. Le moteur natif peut demarrer.");
    Ok(())
}

fn manifest_hash(config: &Config) -> Result<String> {
    let bytes = fs::read(config.resources.join("horizon-resources.json"))
        .map_err(|_| "Manifest des ressources absent.")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn ready(config: &Config) -> bool {
    if config.check_native().is_err() {
        return false;
    }
    let Some(preparation) = &config.preparation else {
        return true;
    };
    let marker = fs::read(preparation.root.join(".local/rusted-ready.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    let Some(marker) = marker else {
        return false;
    };
    marker["schema"] == 1
        && marker["horizon_revision"] == config.horizon_revision
        && marker["sounds"] == true
        && manifest_hash(config).is_ok_and(|hash| marker["resources_sha256"] == hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_complete_verified_downloads() {
        let mut file = tempfile::tempfile().unwrap();
        let data = b"original test fixture";
        let expected = format!("{:x}", Sha1::digest(data));
        copy_verified(&data[..], &mut file, &expected).unwrap();
        assert!(copy_verified(&b"corrupted"[..], &mut file, &expected).is_err());
    }
    #[test]
    fn manual_server_is_checked_before_execution() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("server.jar");
        fs::write(&jar, b"not the game").unwrap();
        let preparation = Preparation {
            root: dir.path().into(),
            executable: "unused".into(),
            server_jar: Some(jar),
            minecraft_directory: None,
            download_server: false,
            import_launcher: false,
        };
        assert!(verified_server(&preparation).is_err());
    }
}
