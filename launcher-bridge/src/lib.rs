//! Launcher protocol adapter, independent of Minecraft and Horizon game code.
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, Read},
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, String>;
pub const HORIZON_REVISION: &str = "8993772de0bf4b0f92c9f76be64f7a1e6b21cf37";
pub const MINECRAFT_VERSION: &str = "26.3";
pub mod bootstrap;
pub mod installer;
pub mod launcher_install;
pub mod preparation;
pub mod routing;
pub mod setup;

/// A literal command suitable for Prism's splitter, including paths with spaces.
pub fn wrapper_command(executable: &Path) -> Result<String> {
    let path = executable
        .to_str()
        .ok_or("Chemin du programme non Unicode.")?;
    if path.contains(['"', '\r', '\n']) {
        return Err("Le chemin contient un caractere incompatible avec le Wrapper.".into());
    }
    let path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!("//{}", unc.replace('\\', "/"))
    } else {
        path.strip_prefix(r"\\?\")
            .unwrap_or(path)
            .replace('\\', "/")
    };
    Ok(format!("\"{path}\" --"))
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Auto,
    Native,
    Java,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: u32,
    pub mode: Mode,
    pub horizon_revision: String,
    pub minecraft_version: String,
    pub executable: PathBuf,
    pub resources: PathBuf,
    pub data_directory: PathBuf,
    #[serde(default)]
    pub preparation: Option<preparation::Preparation>,
    #[serde(default)]
    pub integrity_manifest: Option<PathBuf>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let path = fs::canonicalize(path).map_err(|_| "Configuration introuvable.")?;
        let mut config: Self =
            serde_json::from_slice(&fs::read(&path).map_err(|_| "Configuration illisible.")?)
                .map_err(|_| "Configuration JSON invalide (schema attendu : 1).")?;
        if config.schema != 1 {
            return Err("Version de configuration non prise en charge.".into());
        }
        // Java mode deliberately retains the launcher's exact command and stdin.
        if config.mode != Mode::Java
            && (config.horizon_revision != HORIZON_REVISION
                || config.minecraft_version != MINECRAFT_VERSION)
        {
            return Err(
                "Cette passerelle native cible la revision Horizon epinglee et Minecraft 26.3."
                    .into(),
            );
        }
        for p in [
            &mut config.executable,
            &mut config.resources,
            &mut config.data_directory,
        ] {
            if p.is_relative() {
                *p = path.parent().unwrap().join(&*p);
            }
        }
        if let Some(preparation) = &mut config.preparation {
            preparation.resolve(path.parent().unwrap());
        }
        if let Some(manifest) = &mut config.integrity_manifest {
            if manifest.is_relative() {
                *manifest = path.parent().unwrap().join(&*manifest);
            }
        }
        Ok(config)
    }

    pub fn check_integrity(&self) -> Result<()> {
        use sha2::{Digest, Sha256};
        use std::path::Component;
        let Some(path) = &self.integrity_manifest else {
            return Ok(());
        };
        #[derive(Deserialize)]
        struct Integrity {
            schema: u32,
            files: BTreeMap<PathBuf, String>,
        }
        let mut manifest = fs::File::open(path)
            .map_err(|_| "Manifest d'integrite absent : extraire le pack complet.")?;
        let mut bytes = Vec::new();
        (&mut manifest)
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Manifest d'integrite illisible.")?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("Manifest d'integrite trop volumineux.".into());
        }
        let integrity: Integrity =
            serde_json::from_slice(&bytes).map_err(|_| "Manifest d'integrite invalide.")?;
        if integrity.schema != 1 || integrity.files.is_empty() {
            return Err("Manifest d'integrite non pris en charge.".into());
        }
        let base = path.parent().unwrap();
        for (name, expected) in &integrity.files {
            if name.as_os_str().is_empty()
                || !name.components().all(|c| matches!(c, Component::Normal(_)))
                || expected.len() != 64
                || !expected.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err("Chemin ou empreinte invalide dans le manifest d'integrite.".into());
            }
            let mut file = fs::File::open(base.join(name)).map_err(|_| {
                "Un fichier du runtime manque : extraire a nouveau le pack complet."
            })?;
            let mut digest = Sha256::new();
            let mut block = [0u8; 65536];
            loop {
                let n = file
                    .read(&mut block)
                    .map_err(|_| "Lecture d'un fichier du runtime impossible.")?;
                if n == 0 {
                    break;
                }
                digest.update(&block[..n]);
            }
            if format!("{:x}", digest.finalize()) != expected.to_ascii_lowercase() {
                return Err("Un fichier du runtime est corrompu ou modifie : extraire a nouveau le pack complette.".into());
            }
        }
        for executable in
            std::iter::once(&self.executable).chain(self.preparation.iter().map(|p| &p.executable))
        {
            let full =
                fs::canonicalize(executable).map_err(|_| "Executable du runtime introuvable.")?;
            if !integrity
                .files
                .keys()
                .any(|name| fs::canonicalize(base.join(name)).is_ok_and(|p| p == full))
            {
                return Err("Executable absent du manifest d'integrite.".into());
            }
        }
        Ok(())
    }

    pub fn check_native(&self) -> Result<()> {
        if !self.executable.is_file() {
            return Err(
                "Binaire Horizon absent : compiler ou installer Horizon avant de lancer.".into(),
            );
        }
        let manifest = self.resources.join("horizon-resources.json");
        let value: serde_json::Value = serde_json::from_slice(&fs::read(manifest)
            .map_err(|_| "Ressources absentes : preparer sa propre installation Minecraft 26.3 avec horizon-prepare.")?)
            .map_err(|_| "Manifest de ressources Horizon invalide.")?;
        if value.get("minecraft").and_then(|v| v.as_str()) != Some(MINECRAFT_VERSION) {
            return Err("Les ressources ne ciblent pas Minecraft 26.3.".into());
        }
        Ok(())
    }
}

// Credentials deliberately have no Debug or Serialize implementation.
pub struct Launch {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: Option<String>,
    pub version: String,
    pub game_directory: PathBuf,
    pub native_args: Vec<String>,
    pub assets_directory: Option<PathBuf>,
}

fn put(fields: &mut BTreeMap<String, String>, key: &str, value: &str) -> Result<()> {
    if fields.insert(key.to_owned(), value.to_owned()).is_some() {
        return Err("Parametre de lancement duplique.".into());
    }
    Ok(())
}

impl Launch {
    pub fn from_game_args(args: &[String], cwd: &Path) -> Result<Self> {
        let mut fields = BTreeMap::new();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if arg == "--demo" {
                return Err("La demo Java doit rester en mode Java : Horizon ne reproduit pas ses restrictions.".into());
            }
            // Unknown Java launcher options are not forwarded into Horizon.
            if arg.starts_with("--") {
                let value = args.next().ok_or("Argument de lancement sans valeur.")?;
                put(&mut fields, arg, value)?;
            } else {
                return Err("Arguments de jeu non reconnus.".into());
            }
        }
        let get = |k: &str| {
            fields.get(k).cloned().ok_or_else(|| "Session ou version absente : lancer depuis un compte Minecraft connecte dans le lanceur.".to_owned())
        };
        let name = get("--username")?;
        let uuid = get("--uuid")?;
        let access_token = get("--accessToken")?;
        let compact: String = uuid.chars().filter(|c| *c != '-').collect();
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || name.is_empty()
            || name.len() > 16
            || compact.len() != 32
            || !compact.chars().all(|c| c.is_ascii_hexdigit())
            || compact.chars().all(|c| c == '0')
            || access_token.len() < 16
            || matches!(access_token.as_str(), "null" | "undefined")
            // The official 26.3 manifest no longer supplies --userType.
            // Only this pinned version may omit it; explicit legacy/offline types
            // and missing/invalid account fields remain rejected.
            || fields.get("--userType").map_or_else(
                || fields.get("--version").is_none_or(|s| s != MINECRAFT_VERSION),
                |s| s != "msa",
            )
        {
            return Err("Session Microsoft absente ou invalide : se connecter dans Prism ou Modrinth (mode natif sans compte hors ligne).".into());
        }
        let directory = PathBuf::from(get("--gameDir")?);
        let game_directory = fs::canonicalize(if directory.is_absolute() {
            directory
        } else {
            cwd.join(directory)
        })
        .map_err(|_| "Le dossier de l'instance du lanceur est introuvable.")?;
        let mut native_args = Vec::new();
        for option in ["--width", "--height"] {
            if let Some(value) = fields.get(option) {
                if !value.parse::<u32>().is_ok_and(|n| (1..=16384).contains(&n)) {
                    return Err("Dimensions de fenetre invalides.".into());
                }
                native_args.extend([option.to_owned(), value.clone()]);
            }
        }
        // Never quick-play a Java save from the launcher's original instance.
        if fields.contains_key("--quickPlaySingleplayer")
            || fields.contains_key("--quickPlayRealms")
        {
            return Err("Ouvrir les mondes natifs depuis les menus Horizon ; Quick Play solo/Realms n'est pas pris en charge.".into());
        }
        if let Some(address) = fields
            .get("--quickPlayMultiplayer")
            .or_else(|| fields.get("--server"))
        {
            let address = if let Some(port) = fields.get("--port") {
                let _: u16 = port.parse().map_err(|_| "Port serveur invalide.")?;
                format!("{address}:{port}")
            } else {
                address.clone()
            };
            native_args.extend(["--server".into(), address]);
        }
        Ok(Self {
            name,
            uuid,
            access_token,
            xuid: fields.get("--xuid").cloned(),
            version: get("--version")?,
            game_directory,
            native_args,
            assets_directory: fields.get("--assetsDir").map(|s| {
                let p = PathBuf::from(s);
                if p.is_absolute() {
                    p
                } else {
                    cwd.join(p)
                }
            }),
        })
    }

    pub fn validate_native(&self) -> Result<()> {
        self.validate_native_except(None)
    }

    /// Only ignore the bootstrap after its digest has been verified by bootstrap::prepare.
    pub fn validate_native_except(&self, bootstrap: Option<&Path>) -> Result<()> {
        if self.version != MINECRAFT_VERSION {
            return Err("L'instance doit utiliser Minecraft 26.3 vanilla en mode natif ; les autres versions restent en mode Java.".into());
        }
        let mods = self.game_directory.join("mods");
        if mods.is_dir() {
            for entry in
                fs::read_dir(mods).map_err(|_| "Impossible de verifier le dossier mods.")?
            {
                let path = entry.map_err(|_| "Impossible de verifier un mod.")?.path();
                if bootstrap.is_some_and(|b| fs::canonicalize(&path).is_ok_and(|p| p == b)) {
                    continue;
                }
                if path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("jar"))
                {
                    let report = rusted_platform::scan(&self.game_directory.join("mods"));
                    let details = report
                        .mods
                        .iter()
                        .take(5)
                        .map(|m| {
                            format!(
                                "{} [{}]",
                                m.name,
                                if m.loaders.is_empty() {
                                    "Java inconnu".into()
                                } else {
                                    m.loaders.join("/")
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(format!("Mods Java presents : {details}. Rusted natif ne peut pas les executer. Installer le chargeur requis dans Modifier > Version de Prism et activer le mode auto avec Configurer-Rusted.exe. Ne pas melanger Forge et Fabric."));
                }
            }
        }
        Ok(())
    }
}

/// Modern Prism writes its launch parameters on stdin, then `launch`.
/// Bounded reads: malformed launcher input must not allocate without limit.
pub fn read_prism(reader: &mut impl BufRead) -> Result<Option<Vec<String>>> {
    let mut params = Vec::new();
    let mut bytes = 0;
    let mut server = None;
    let mut port = None;
    let mut window = None;
    loop {
        let mut line = String::new();
        let n = (&mut *reader)
            .take(65537)
            .read_line(&mut line)
            .map_err(|_| "Lecture du protocole Prism impossible.")?;
        bytes += n;
        if n == 0 || n > 65536 || bytes > 2 * 1024 * 1024 {
            return Err("Protocole Prism incomplet ou trop volumineux.".into());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line == "abort" {
            return Ok(None);
        }
        if line == "launch" {
            break;
        }
        let (key, value) = line.split_once(' ').ok_or("Ligne Prism invalide.")?;
        match key {
            "param" => params.push(value.to_owned()),
            "launcher" if !matches!(value, "standard" | "modern") => return Err("Le mode Prism legacy n'est pas pris en charge ; utiliser une instance vanilla 26.3 standard.".into()),
            "mainClass" if value != "net.minecraft.client.main.Main" => return Err("Creer une instance vanilla pour Horizon ; Fabric/Forge reste disponible en mode Java.".into()),
            "worldName" => return Err("Quick Play solo Prism indisponible pour les mondes natifs.".into()),
            "serverAddress" => server = Some(value.to_owned()),
            "serverPort" => port = Some(value.to_owned()),
            "windowParams" => window = Some(value.to_owned()),
            _ => {}
        }
    }
    if let Some(server) = server {
        params.extend(["--server".into(), server]);
        if let Some(port) = port {
            params.extend(["--port".into(), port]);
        }
    }
    if !params.iter().any(|a| a == "--width" || a == "--height") {
        if let Some((w, h)) = window.as_deref().and_then(|s| s.split_once('x')) {
            params.extend(["--width".into(), w.into(), "--height".into(), h.into()]);
        }
    }
    Ok(Some(params))
}

pub fn game_args(raw: &[String]) -> Result<&[String]> {
    // In Modrinth the real game options follow the main class. JVM options,
    // classpath and the Java executable are never forwarded to the Rust game.
    let main = raw
        .iter()
        .position(|a| a == "net.minecraft.client.main.Main")
        .ok_or("En mode natif, choisir une instance vanilla dans le lanceur.")?;
    Ok(&raw[main + 1..])
}
