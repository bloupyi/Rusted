//! Installs a fresh Prism instance. Existing instances and saves are never overwritten.
use crate::{setup, wrapper_command, Config, Result};
use std::{
    collections::HashSet,
    fs,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};

pub fn enable_auto(runtime: &Path) -> Result<()> {
    let path = runtime.join("rusted-runtime.json");
    let original = fs::read(&path).map_err(|_| "Configuration Rusted absente.")?;
    let mut value: serde_json::Value =
        serde_json::from_slice(&original).map_err(|_| "Configuration Rusted invalide.")?;
    if value["mode"] == "auto" {
        return Ok(());
    }
    if !matches!(value["mode"].as_str(), Some("java" | "native")) {
        return Err("Mode Rusted inconnu.".into());
    }
    value["mode"] = "auto".into();
    let mut backup = tempfile::Builder::new()
        .prefix("rusted-runtime-backup-")
        .suffix(".json")
        .tempfile_in(runtime)
        .map_err(|_| "Sauvegarde Rusted impossible.")?;
    backup
        .write_all(&original)
        .and_then(|_| backup.as_file().sync_all())
        .map_err(|_| "Sauvegarde Rusted incomplete.")?;
    backup.keep().map_err(|_| "Sauvegarde Rusted impossible.")?;
    let mut replacement = tempfile::NamedTempFile::new_in(runtime)
        .map_err(|_| "Configuration Rusted non accessible en ecriture.")?;
    serde_json::to_writer_pretty(replacement.as_file_mut(), &value)
        .map_err(|_| "Ecriture Rusted impossible.")?;
    replacement
        .as_file()
        .sync_all()
        .map_err(|_| "Ecriture Rusted incomplete.")?;
    if fs::read(&path).map_err(|_| "Configuration Rusted inaccessible.")? != original {
        return Err("Configuration modifiee pendant l'installation ; reessayer.".into());
    }
    replacement
        .persist(path)
        .map_err(|_| "Remplacement de la configuration Rusted impossible.")?;
    Ok(())
}

pub fn install(payload: &[u8], instances: &Path) -> Result<PathBuf> {
    let instances = fs::canonicalize(instances)
        .map_err(|_| "Choisir le dossier instances existant de Prism.")?;
    if !instances.is_dir() {
        return Err("Le dossier instances est invalide.".into());
    }
    let destination = instances.join("Rusted-0.3.4");
    let mut archive = zip::ZipArchive::new(Cursor::new(payload))
        .map_err(|_| "Paquet d'installation invalide.")?;
    if archive.len() > 10000 {
        return Err("Trop de fichiers dans le paquet.".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|_| "Fichier du paquet illisible.")?;
        let name = file
            .name()
            .strip_prefix("Rusted/")
            .ok_or("Racine du paquet invalide.")?;
        if name.is_empty()
            || name.contains(['\\', ':'])
            || !Path::new(name)
                .components()
                .all(|p| matches!(p, Component::Normal(_)))
            || file.is_dir()
            || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            || !names.insert(name.to_ascii_lowercase())
        {
            return Err("Chemin du paquet invalide ou duplique.".into());
        }
        total = total
            .checked_add(file.size())
            .ok_or("Taille du paquet invalide.")?;
        if total > 512 * 1024 * 1024 {
            return Err("Paquet trop volumineux.".into());
        }
    }
    for required in [
        "rusted-launch.exe",
        "rusted-runtime.json",
        "runtime-manifest.json",
    ] {
        if !names.contains(required) {
            return Err("Paquet Rusted incomplet.".into());
        }
    }
    // Exclusive creation: never reuse a folder, even an empty one.
    fs::create_dir(&destination).map_err(|_| "Rusted-0.3.4 existe deja ou le dossier est inaccessible. Aucune instance existante n'a ete modifiee.")?;
    struct Rollback(PathBuf, bool);
    impl Drop for Rollback {
        fn drop(&mut self) {
            if !self.1 {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
    let mut rollback = Rollback(destination.clone(), false);
    let runtime = destination.join("minecraft/rusted");
    for i in 0..archive.len() {
        let mut input = archive
            .by_index(i)
            .map_err(|_| "Fichier du paquet illisible.")?;
        let path = runtime.join(input.name().strip_prefix("Rusted/").unwrap());
        fs::create_dir_all(path.parent().unwrap())
            .map_err(|_| "Creation du dossier Rusted impossible.")?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| "Creation d'un fichier Rusted impossible.")?;
        let size = input.size();
        let copied = std::io::copy(&mut (&mut input).take(size + 1), &mut output)
            .map_err(|_| "Extraction du paquet impossible.")?;
        if copied != size {
            return Err("Taille d'un fichier extraite incorrecte.".into());
        }
    }
    Config::load(&runtime.join("rusted-runtime.json"))?.check_integrity()?;
    enable_auto(&runtime)?;
    let command = wrapper_command(&runtime.join("rusted-launch.exe"))?;
    let ini = setup::configured_ini(
        "[General]\nConfigVersion=1.3\nname=Rusted 0.3.4\nInstanceType=OneSix\n",
        &command,
    )?;
    fs::write(destination.join("mmc-pack.json"), serde_json::json!({"formatVersion":1,"components":[{"uid":"net.minecraft","version":crate::MINECRAFT_VERSION}]}).to_string()).map_err(|_| "Creation du profil Prism impossible.")?;
    // Write discovery marker last; Prism cannot see a partially extracted instance.
    fs::write(destination.join("instance.cfg"), ini)
        .map_err(|_| "Creation de la configuration Prism impossible.")?;
    rollback.1 = true;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn payload() -> Vec<u8> {
        let config = serde_json::json!({"schema":1,"mode":"native","horizon_revision":crate::HORIZON_REVISION,"minecraft_version":crate::MINECRAFT_VERSION,"executable":"rusted-launch.exe","resources":"resources","data_directory":"native-data","integrity_manifest":"runtime-manifest.json"}).to_string();
        let manifest = serde_json::json!({"schema":1,"files":{"rusted-launch.exe":format!("{:x}",Sha256::digest(b"OWN_TEST_BINARY_ONLY"))}}).to_string();
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, body) in [
            ("rusted-launch.exe", "OWN_TEST_BINARY_ONLY"),
            ("rusted-runtime.json", &config),
            ("runtime-manifest.json", &manifest),
        ] {
            out.start_file(
                format!("Rusted/{path}"),
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            out.write_all(body.as_bytes()).unwrap();
        }
        out.finish().unwrap().into_inner()
    }
    #[test]
    fn fresh_instance_has_absolute_wrapper_auto_mode_and_never_overwrites() {
        let temp = tempfile::Builder::new()
            .prefix("Prism O'Neil # space ")
            .tempdir()
            .unwrap();
        let instance = install(&payload(), temp.path()).unwrap();
        let ini = fs::read_to_string(instance.join("instance.cfg")).unwrap();
        let runtime = instance.join("minecraft/rusted");
        assert_eq!(
            ini,
            setup::configured_ini(
                "[General]\nConfigVersion=1.3\nname=Rusted 0.3.4\nInstanceType=OneSix\n",
                &wrapper_command(&runtime.join("rusted-launch.exe")).unwrap()
            )
            .unwrap()
        );
        assert!(matches!(
            Config::load(&runtime.join("rusted-runtime.json"))
                .unwrap()
                .mode,
            crate::Mode::Auto
        ));
        fs::write(runtime.join("player-world-marker"), "preserve").unwrap();
        assert!(install(&payload(), temp.path()).is_err());
        assert_eq!(
            fs::read_to_string(runtime.join("player-world-marker")).unwrap(),
            "preserve"
        );
    }
    #[test]
    fn integrity_failure_rolls_back_only_new_instance_and_paths_are_validated() {
        let temp = tempfile::tempdir().unwrap();
        let mut bad = payload();
        // Corrupt the first stored member while leaving its expected digest unchanged.
        let offset = bad
            .windows(20)
            .position(|v| v == b"OWN_TEST_BINARY_ONLY")
            .unwrap();
        bad[offset] = b'X';
        assert!(install(&bad, temp.path()).is_err());
        assert!(!temp.path().join("Rusted-0.3.4").exists());
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        out.start_file(
            "Rusted/../escape",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        out.write_all(b"test").unwrap();
        assert!(install(&out.finish().unwrap().into_inner(), temp.path()).is_err());
        assert!(!temp.path().join("escape").exists());
        let existing = temp.path().join("Rusted-0.3.4");
        fs::create_dir(&existing).unwrap();
        assert!(install(&payload(), temp.path()).is_err());
        assert!(existing.is_dir());
    }
    #[test]
    fn auto_setup_backs_up_original_config_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("rusted-runtime.json");
        let original = "{\"mode\":\"native\",\"custom\":42}";
        fs::write(&path, original).unwrap();
        enable_auto(temp.path()).unwrap();
        enable_auto(temp.path()).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap()
                ["custom"],
            42
        );
        let backups: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .flatten()
            .filter(|p| {
                p.file_name()
                    .to_string_lossy()
                    .starts_with("rusted-runtime-backup-")
            })
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read_to_string(backups[0].path()).unwrap(), original);
    }
}
