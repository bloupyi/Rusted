//! Normalize Prism/Modrinth's installed files for the upstream preparer.
//! Only metadata is fetched; the client, libraries and assets must already be installed.
use crate::{preparation::Preparation, Result};
use sha1::{Digest, Sha1};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::Duration,
};

const MANIFEST_SHA1: &str = "11f1aefe473eefba0af62e3286c33b692807b2eb";
const MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/v1/packages/11f1aefe473eefba0af62e3286c33b692807b2eb/26.3.json";
const CLIENT_SHA1: &str = "e877b6a07acd633fb3bb475002175cec036e7b87";

pub struct LauncherFiles {
    pub jars: Vec<PathBuf>,
    pub assets: PathBuf,
}

impl LauncherFiles {
    pub fn from_args(raw: &[String], assets: Option<&Path>) -> Result<Self> {
        let classpath = raw
            .windows(2)
            .find(|pair| matches!(pair[0].as_str(), "-cp" | "-classpath" | "--class-path"))
            .map(|pair| &pair[1])
            .ok_or("Le lanceur n'a pas fourni son classpath Java.")?;
        let mut jars = BTreeSet::new();
        for path in std::env::split_paths(classpath) {
            expand_classpath(&path, 0, &mut jars)?;
        }
        let assets = assets.ok_or("Le lanceur n'a pas fourni --assetsDir pour la preparation.")?;
        if !assets.is_dir() {
            return Err(
                "Les assets du lanceur sont absents ; terminer l'installation de Minecraft 26.3."
                    .into(),
            );
        }
        Ok(Self {
            jars: jars.into_iter().collect(),
            assets: assets.into(),
        })
    }
}

fn expand_classpath(path: &Path, depth: usize, result: &mut BTreeSet<PathBuf>) -> Result<()> {
    if !path.is_file() || !result.insert(path.to_owned()) {
        return Ok(());
    }
    if result.len() > 2048 || depth > 3 {
        return Err("Classpath du lanceur trop complexe.".into());
    }
    // Modrinth shortens its command with a JAR whose MANIFEST Class-Path lists the real files.
    let file = fs::File::open(path).map_err(|_| "Un fichier du classpath est illisible.")?;
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return Ok(());
    };
    let Ok(mut entry) = archive.by_name("META-INF/MANIFEST.MF") else {
        return Ok(());
    };
    if entry.size() > 1024 * 1024 {
        // Signed game/library manifests may list thousands of digests. They
        // are ordinary classpath members, not Modrinth's small proxy JAR.
        return Ok(());
    }
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|_| "Manifest du classpath invalide.")?;
    let mut unfolded = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            unfolded.push_str(rest);
        } else {
            unfolded.push('\n');
            unfolded.push_str(line);
        }
    }
    let base = url::Url::from_file_path(
        std::path::absolute(path).map_err(|_| "Chemin de classpath invalide.")?,
    )
    .map_err(|_| "Chemin du classpath invalide.")?;
    for line in unfolded.lines() {
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("Class-Path") {
                for value in value.split_whitespace() {
                    let value = base.join(value).map_err(|_| "URL du classpath invalide.")?;
                    if value.scheme() != "file" {
                        return Err("Un classpath distant n'est pas pris en charge.".into());
                    }
                    let file = value
                        .to_file_path()
                        .map_err(|_| "Fichier du classpath invalide.")?;
                    expand_classpath(&file, depth + 1, result)?;
                }
            }
        }
    }
    Ok(())
}

fn sha1(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).map_err(|_| "Un fichier Minecraft installe est inaccessible.")?;
    let mut digest = Sha1::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| "Lecture d'un fichier Minecraft impossible.")?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn install_file(source: &Path, destination: &Path, checksum: &str) -> Result<()> {
    if checksum.len() != 40 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Empreinte Minecraft invalide.".into());
    }
    if destination.is_file() && sha1(destination)? == checksum {
        return Ok(());
    }
    if sha1(source)? != checksum {
        return Err("Un fichier Minecraft du lanceur est corrompu ou d'une autre version ; reparer l'instance dans le lanceur.".into());
    }
    let parent = destination.parent().unwrap();
    fs::create_dir_all(parent).map_err(|_| "Cache d'import inaccessible.")?;
    // New hard links are atomic and never written through. Replacements use a private
    // temporary file, so repairing our cache cannot overwrite the launcher's source.
    if !destination.exists() && fs::hard_link(source, destination).is_ok() {
        return Ok(());
    }
    let temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cache d'import inaccessible.")?;
    fs::copy(source, temporary.path())
        .map_err(|_| "Copie d'un fichier installe impossible (espace disque ?).")?;
    temporary
        .persist(destination)
        .map_err(|_| "Impossible de conserver un fichier importe.")?;
    Ok(())
}

fn relative(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if value.contains('\\')
        || !path.components().all(|c| matches!(c, Component::Normal(_)))
        || path.as_os_str().is_empty()
    {
        return Err("Chemin de fichier officiel invalide.".into());
    }
    Ok(path)
}

pub fn import(preparation: &Preparation, files: &LauncherFiles) -> Result<PathBuf> {
    let root = preparation.root.join(".local/launcher-input");
    let versions = root.join("versions/26.3");
    fs::create_dir_all(&versions).map_err(|_| "Cache d'import inaccessible.")?;
    let manifest = versions.join("26.3.json");
    if !manifest.is_file() || sha1(&manifest)? != MANIFEST_SHA1 {
        let response = ureq::AgentBuilder::new()
            .try_proxy_from_env(true)
            .timeout(Duration::from_secs(60))
            .build()
            .get(MANIFEST_URL)
            .call()
            .map_err(|_| "Metadata officielles 26.3 inaccessibles ; verifier la connexion.")?;
        let mut body = Vec::new();
        response
            .into_reader()
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Metadata officielles illisibles.")?;
        if body.len() > 4 * 1024 * 1024 || format!("{:x}", Sha1::digest(&body)) != MANIFEST_SHA1 {
            return Err("Empreinte des metadata officielles incorrecte.".into());
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&versions)
            .map_err(|_| "Cache d'import inaccessible.")?;
        std::io::Write::write_all(temporary.as_file_mut(), &body)
            .map_err(|_| "Ecriture des metadata impossible.")?;
        temporary
            .persist(&manifest)
            .map_err(|_| "Publication des metadata impossible.")?;
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).map_err(|_| "Metadata officielles absentes.")?)
            .map_err(|_| "Metadata officielles invalides.")?;
    if metadata["id"] != "26.3" || metadata["downloads"]["client"]["sha1"] != CLIENT_SHA1 {
        return Err("Les metadata ne ciblent pas le client 26.3 attendu.".into());
    }
    let size = metadata["downloads"]["client"]["size"].as_u64();
    let client = files.jars.iter().find(|path| fs::metadata(path).ok().map(|m| m.len()) == size && sha1(path).is_ok_and(|h| h == CLIENT_SHA1))
        .ok_or("Le client 26.3 n'est pas installe dans le classpath du lanceur ; reparer l'instance vanilla 26.3.")?;
    install_file(client, &versions.join("26.3.jar"), CLIENT_SHA1)?;
    let mut libraries = BTreeSet::new();
    for jar in &files.jars {
        for parent in jar.ancestors() {
            if parent.file_name().is_some_and(|name| name == "libraries") {
                libraries.insert(parent.to_path_buf());
            }
        }
    }
    let mut copied = 0;
    for library in metadata["libraries"]
        .as_array()
        .ok_or("Liste des bibliotheques officielle absente.")?
    {
        let artifact = &library["downloads"]["artifact"];
        let (Some(path), Some(expected)) = (artifact["path"].as_str(), artifact["sha1"].as_str())
        else {
            continue;
        };
        let path = relative(path)?;
        let mut candidates: Vec<PathBuf> = libraries.iter().map(|base| base.join(&path)).collect();
        if let Some(name) = library["name"].as_str() {
            if let [group, name, version, classifier] = name.split(':').collect::<Vec<_>>()[..] {
                let alternate = relative(&format!(
                    "{}/{name}-{classifier}/{version}/{name}-{classifier}-{version}.jar",
                    group.replace('.', "/")
                ))?;
                candidates.extend(libraries.iter().map(|base| base.join(&alternate)));
            }
        }
        candidates.extend(
            files
                .jars
                .iter()
                .filter(|p| p.file_name() == path.file_name())
                .cloned(),
        );
        if let Some(source) = candidates
            .iter()
            .find(|p| p.is_file() && sha1(p).is_ok_and(|h| h == expected))
        {
            install_file(source, &root.join("libraries").join(&path), expected)?;
            copied += 1;
        }
        // Other platforms' native jars are absent normally; upstream decides which are required.
    }
    let index_id = metadata["assetIndex"]["id"]
        .as_str()
        .ok_or("Index des ressources absent.")?;
    let index_name = relative(&format!("indexes/{index_id}.json"))?;
    let index = files.assets.join(&index_name);
    let mut bytes = Vec::new();
    fs::File::open(&index)
        .map_err(|_| "Index des assets absent.")?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Index des assets illisible.")?;
    let parsed = parse_asset_index(&bytes)?;
    let checksum = format!("{:x}", Sha1::digest(&bytes));
    // Mojang republishes the shared index, and launchers may reserialize it.
    // Match the preparer's contract: validate its structure and each object's
    // content, instead of requiring a frozen hash of the entire JSON file.
    if metadata["assetIndex"]["sha1"].as_str() != Some(&checksum) {
        eprintln!("Rusted : index des assets mis a jour ou reformate ; verification individuelle de chaque fichier.");
    }
    install_file(&index, &root.join("assets").join(&index_name), &checksum)?;
    let index = parsed;
    let mut count = 0;
    let mut seen = BTreeSet::new();
    for value in index["objects"]
        .as_object()
        .ok_or("Liste des assets absente.")?
        .values()
    {
        let hash = value["hash"].as_str().ok_or("Empreinte d'asset absente.")?;
        if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Empreinte d'asset invalide.".into());
        }
        if seen.insert(hash) {
            let path = PathBuf::from(format!("objects/{}/{}", &hash[..2], hash));
            let source = files.assets.join(&path);
            if fs::metadata(&source)
                .map_err(|_| "Un asset du lanceur est absent ; terminer son installation.")?
                .len()
                != value["size"].as_u64().ok_or("Taille d'asset absente.")?
            {
                return Err(
                    "Un asset du lanceur est incomplet ; reparer les ressources de l'instance."
                        .into(),
                );
            }
            install_file(&source, &root.join("assets").join(path), hash)?;
            count += 1;
        }
    }
    eprintln!("Rusted : client 26.3, {copied} bibliotheques et {count} assets identifies depuis le lanceur (aucune sauvegarde importee).");
    Ok(root)
}

fn parse_asset_index(bytes: &[u8]) -> Result<serde_json::Value> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Index des assets trop volumineux.".into());
    }
    let index: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "Index des assets invalide.")?;
    let objects = index["objects"]
        .as_object()
        .filter(|o| !o.is_empty())
        .ok_or("Liste des assets absente ou vide.")?;
    for (name, object) in objects {
        relative(name)?;
        let hash = object["hash"]
            .as_str()
            .ok_or("Empreinte d'asset absente.")?;
        if hash.len() != 40
            || !hash.chars().all(|c| c.is_ascii_hexdigit())
            || object["size"].as_u64().is_none()
        {
            return Err("Empreinte ou taille d'asset invalide.".into());
        }
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn updated_and_reformatted_indexes_keep_per_object_integrity_checks() {
        let content = b"original fixture asset";
        let checksum = format!("{:x}", Sha1::digest(content));
        let index = serde_json::json!({"objects":{"minecraft/lang/example.json":{"hash":checksum,"size":content.len()}}});
        let compact = serde_json::to_vec(&index).unwrap();
        let pretty = serde_json::to_vec_pretty(&index).unwrap();
        assert_ne!(Sha1::digest(&compact), Sha1::digest(&pretty));
        assert_eq!(
            parse_asset_index(&compact).unwrap(),
            parse_asset_index(&pretty).unwrap()
        );
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::write(&source, content).unwrap();
        install_file(&source, &dir.path().join("copy"), &checksum).unwrap();
        fs::write(&source, b"tampered fixture").unwrap();
        assert!(install_file(&source, &dir.path().join("another-copy"), &checksum).is_err());
        for bad in [
            serde_json::json!({"objects":{}}),
            serde_json::json!({"objects":{"../outside":{"hash":checksum,"size":1}}}),
            serde_json::json!({"objects":{"safe":{"hash":"../invalid","size":1}}}),
        ] {
            assert!(parse_asset_index(&serde_json::to_vec(&bad).unwrap()).is_err());
        }
    }
    #[test]
    fn expands_modrinth_classpath_manifest_with_spaces_and_continuations() {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library with spaces.jar");
        fs::write(&library, b"original fixture, not code").unwrap();
        let jar = dir.path().join("classpath.jar");
        let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
        zip.start_file(
            "META-INF/MANIFEST.MF",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(
            b"Manifest-Version: 1.0\r\nClass-Path: library%20with%20\r\n spaces.jar\r\n\r\n",
        )
        .unwrap();
        zip.finish().unwrap();
        let mut paths = BTreeSet::new();
        expand_classpath(&jar, 0, &mut paths).unwrap();
        assert!(paths.contains(&library));
    }
    #[test]
    fn signed_game_manifest_is_not_a_proxy_classpath() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("signed-client.jar");
        let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
        zip.start_file(
            "META-INF/MANIFEST.MF",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&vec![b'x'; 1024 * 1024 + 1]).unwrap();
        zip.finish().unwrap();
        let mut paths = BTreeSet::new();
        expand_classpath(&jar, 0, &mut paths).unwrap();
        assert_eq!(paths, BTreeSet::from([jar]));
    }
    #[test]
    fn rejects_unsafe_metadata_paths_and_preserves_installed_sources() {
        assert!(relative("../personal.jar").is_err());
        assert!(relative("/absolute.jar").is_err());
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.jar");
        let destination = dir.path().join("cache.jar");
        fs::write(&source, b"original test bytes").unwrap();
        let hash = sha1(&source).unwrap();
        install_file(&source, &destination, &hash).unwrap();
        assert_eq!(fs::read(&source).unwrap(), fs::read(&destination).unwrap());
        assert!(install_file(&source, &destination, &"0".repeat(40)).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"original test bytes");
    }
}
