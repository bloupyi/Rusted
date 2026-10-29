//! First native Fabric adapter: vanilla textures/languages only, never Java execution.
use crate::{
    download,
    model::{self, Engine, Instance, Store},
    Result, Task,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Convert a strictly resource-only Fabric archive into a resource pack for native 26.3.
/// Code, mixins, nested JARs, gameplay data and other namespaces require Java/Fabric.
pub fn import_resources(
    store: &Store,
    instance: &Instance,
    source: &Path,
    task: &Task,
) -> Result<PathBuf> {
    if instance.engine != Engine::Native || instance.minecraft != "26.3" {
        return Err("Cet adaptateur cible les ressources du moteur Rusted natif 26.3.".into());
    }
    let mut archive =
        zip::ZipArchive::new(fs::File::open(source).map_err(|_| "Mod inaccessible.")?)
            .map_err(|_| "Archive de mod invalide.")?;
    if archive.len() > 20000 {
        return Err("Trop de fichiers dans ce mod.".into());
    }
    let meta: Value = {
        let mut file = archive
            .by_name("fabric.mod.json")
            .map_err(|_| "Ce fichier ne declare pas un mod Fabric.".to_owned())?;
        if file.size() > 256 * 1024 {
            return Err("Metadonnees du mod trop volumineuses.".into());
        }
        let mut body = vec![];
        file.by_ref()
            .take(256 * 1024 + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Metadonnees illisibles.")?;
        serde_json::from_slice(&body).map_err(|_| "Metadonnees Fabric invalides.")?
    };
    if meta["schemaVersion"] != 1 {
        return Err(
            "Seuls les mods Fabric de schema 1 sont pris en charge par cet adaptateur.".into(),
        );
    }
    let nonempty = |key: &str| {
        meta.get(key)
            .is_some_and(|v| !v.is_null() && v != &json!({}) && v != &json!([]))
    };
    if [
        "entrypoints",
        "mixins",
        "jars",
        "accessWidener",
        "languageAdapters",
    ]
    .iter()
    .any(|k| nonempty(k))
    {
        return Err("Ce mod contient du code ou des hooks Java. Utiliser Java + Fabric ; cet adaptateur ne porte que les textures et langues vanilla.".into());
    }
    if meta["depends"].as_object().is_some_and(|v| {
        v.keys().any(|k| {
            !matches!(
                k.as_str(),
                "minecraft" | "java" | "fabricloader" | "fabric-resource-loader-v0"
            )
        })
    }) {
        return Err("Ce mod depend d'une API Java non disponible dans le moteur natif.".into());
    }
    if let Some(version) = meta["depends"].get("minecraft") {
        let accepts = |v: &Value| matches!(v.as_str(), Some("*" | "26.3"));
        if !accepts(version) && !version.as_array().is_some_and(|a| a.iter().any(accepts)) {
            return Err("Pour cette premiere version, le mod doit declarer explicitement Minecraft 26.3 ou * ; sinon utiliser Java/Fabric.".into());
        }
    }
    let id = meta["id"].as_str().ok_or("Identifiant Fabric absent.")?;
    model::safe_id(id)?;
    let mut selected = vec![];
    let mut total = 0u64;
    let mut names = BTreeSet::new();
    // Validate every archive member before creating any installed file.
    for i in 0..archive.len() {
        task.check()?;
        let file = archive.by_index(i).map_err(|_| "Archive illisible.")?;
        let name = file.name().trim_end_matches('/');
        download::path(Path::new("validation"), name)?;
        if !names.insert(name.to_owned())
            || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Doublon ou lien symbolique dans le mod.".into());
        }
        total = total
            .checked_add(file.size())
            .ok_or("Mod trop volumineux.")?;
        if total > 256 * 1024 * 1024 {
            return Err("Mod trop volumineux.".into());
        }
        if file.is_dir() {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if [".class", ".jar", ".wasm"]
            .iter()
            .any(|suffix| lower.ends_with(suffix))
            || lower.starts_with("data/")
        {
            return Err(
                "Code executable ou donnees de gameplay detectes : Java + Fabric est necessaire."
                    .into(),
            );
        }
        if name.starts_with("assets/") {
            if (name.starts_with("assets/minecraft/textures/")
                && (name.ends_with(".png") || name.ends_with(".mcmeta")))
                || (name.starts_with("assets/minecraft/lang/") && name.ends_with(".json"))
            {
                selected.push(i);
            } else if meta["icon"].as_str() != Some(name) {
                return Err("Ressources non prises en charge par cet adaptateur : seules les textures et langues minecraft sont acceptees.".into());
            }
        } else if name != "fabric.mod.json"
            && name != "pack.png"
            && !name.starts_with("META-INF/")
            && !lower.starts_with("license")
            && !lower.starts_with("readme")
        {
            return Err(
                "Fichier non reconnu : ce mod exige une analyse avant son portage natif.".into(),
            );
        }
    }
    if selected.is_empty() {
        return Err("Aucune texture ou langue vanilla a adapter.".into());
    }
    let hash = download::digest(source, download::Algorithm::Sha256)?;
    let folder = store
        .game(instance)?
        .join("rusted/native-data/resourcepacks");
    fs::create_dir_all(&folder).map_err(|_| "Dossier des packs inaccessible.")?;
    let output = folder.join(format!("rusted-fabric-{id}-{}.zip", &hash[..12]));
    if output.exists() {
        return Err("Ce pack adapte est deja installe.".into());
    }
    let temp =
        tempfile::NamedTempFile::new_in(&folder).map_err(|_| "Preparation du pack impossible.")?;
    let mut zip = zip::ZipWriter::new(temp);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("pack.mcmeta", options)
        .map_err(|_| "Creation du pack impossible.")?;
    let name = meta["name"].as_str().unwrap_or(id);
    let pack = json!({"pack":{"pack_format":97,"min_format":[97,1],"max_format":[97,1],"description":format!("Rusted / {name} (ressources Fabric)")}});
    zip.write_all(&serde_json::to_vec(&pack).unwrap())
        .map_err(|_| "Ecriture du pack impossible.")?;
    for index in selected {
        task.check()?;
        let mut file = archive.by_index(index).map_err(|_| "Archive illisible.")?;
        zip.start_file(file.name(), options)
            .map_err(|_| "Creation du pack impossible.")?;
        std::io::copy(&mut file, &mut zip).map_err(|_| "Copie des ressources interrompue.")?;
    }
    let temp = zip
        .finish()
        .map_err(|_| "Finalisation du pack impossible.")?;
    task.check()?;
    temp.persist_noclobber(&output)
        .map_err(|_| "Ce pack existe deja ou n'est pas accessible.")?;
    model::atomic_json(
        &output.with_extension("rusted.json"),
        &json!({"schema":1,"adapter":"fabric-vanilla-resources-v1","mod_id":id,"source_sha256":hash,"target":"Rusted native 26.3","executes_java":false}),
    )?;
    Ok(output)
}
