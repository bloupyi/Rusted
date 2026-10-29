use crate::{
    download::{self, Algorithm, Artifact},
    model::{self, Engine, Instance, Store},
    Result, Task,
};
use serde_json::Value;
use std::{fs, path::PathBuf};
#[derive(Clone, Debug)]
pub struct Hit {
    pub id: String,
    pub name: String,
    pub description: String,
    pub downloads: u64,
}
pub fn search(query: &str, version: &str) -> Result<Vec<Hit>> {
    let facets = serde_json::to_string(&vec![
        vec!["project_type:mod"],
        vec!["categories:fabric"],
        vec![&format!("versions:{version}")],
    ])
    .unwrap();
    let mut url = url::Url::parse("https://api.modrinth.com/v2/search").unwrap();
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("facets", &facets)
        .append_pair("limit", "12")
        .append_pair("index", "downloads");
    let v = download::json(url.as_str())?;
    Ok(v["hits"]
        .as_array()
        .ok_or("Recherche Modrinth invalide.")?
        .iter()
        .filter_map(|v| {
            Some(Hit {
                id: v["project_id"].as_str()?.into(),
                name: v["title"].as_str()?.into(),
                description: v["description"].as_str()?.chars().take(180).collect(),
                downloads: v["downloads"].as_u64().unwrap_or(0),
            })
        })
        .collect())
}
pub fn files(store: &Store, instance: &Instance) -> Result<Vec<String>> {
    let base = folder(store, instance)?;
    if !base.exists() {
        return Ok(vec![]);
    }
    let mut v = fs::read_dir(base)
        .map_err(|_| "Dossier des mods inaccessible.")?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.ends_with(".jar")
                || n.ends_with(".jar.disabled")
                || n.ends_with(".hmod")
                || n.ends_with(".hmod.disabled")
        })
        .collect::<Vec<_>>();
    v.sort();
    Ok(v)
}
fn folder(store: &Store, instance: &Instance) -> Result<PathBuf> {
    let game = store.game(instance)?;
    Ok(if instance.engine == Engine::Native {
        game.join("rusted/native-data/horizon-mods")
    } else {
        game.join("mods")
    })
}
pub fn toggle(store: &Store, instance: &Instance, name: &str) -> Result<()> {
    let base = folder(store, instance)?;
    let src = download::path(&base, name)?;
    if !src.is_file() {
        return Err("Mod introuvable.".into());
    }
    let target = if let Some(n) = name.strip_suffix(".disabled") {
        n.to_string()
    } else {
        format!("{name}.disabled")
    };
    let dest = download::path(&base, &target)?;
    if dest.exists() {
        return Err("Un fichier porte deja ce nom.".into());
    }
    fs::rename(src, dest)
        .map_err(|_| "Le mod ne peut pas etre modifie pendant son utilisation.".into())
}
pub fn trash(store: &Store, instance: &Instance, name: &str) -> Result<()> {
    let src = download::path(&folder(store, instance)?, name)?;
    fs::rename(
        src,
        store
            .root
            .join("trash")
            .join(format!("mod-{}-{name}", model::now())),
    )
    .map_err(|_| "Deplacement vers la corbeille impossible.".into())
}
pub fn install(store: &Store, instance: &mut Instance, project: &str, task: &Task) -> Result<()> {
    model::safe_id(project)?;
    task.status("Resolution des dependances Fabric");
    let mut url = url::Url::parse(&format!(
        "https://api.modrinth.com/v2/project/{project}/version"
    ))
    .map_err(|_| "Projet invalide.")?;
    url.query_pairs_mut()
        .append_pair("loaders", "[\"fabric\"]")
        .append_pair(
            "game_versions",
            &serde_json::to_string(&vec![&instance.minecraft]).unwrap(),
        );
    let versions = download::json(url.as_str())?;
    let version = versions
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|v| v["version_type"] == "release")
                .or_else(|| a.first())
        })
        .ok_or("Aucune version Fabric pour cette version Minecraft.")?
        .clone();
    let mut pending = vec![version];
    let mut resolved = std::collections::BTreeMap::<String, Value>::new();
    while let Some(v) = pending.pop() {
        task.check()?;
        if resolved.len() > 128 {
            return Err("Trop de dependances de mods.".into());
        }
        let project = v["project_id"]
            .as_str()
            .ok_or("Projet de mod absent.")?
            .to_owned();
        if let Some(old) = resolved.get(&project) {
            if old["id"] != v["id"] {
                return Err(
                    "Deux dependances demandent des versions incompatibles du meme mod.".into(),
                );
            }
            continue;
        }
        if !v["loaders"]
            .as_array()
            .is_some_and(|a| a.iter().any(|s| s == "fabric"))
            || !v["game_versions"]
                .as_array()
                .is_some_and(|a| a.iter().any(|s| s == &instance.minecraft))
        {
            return Err("Dependance incompatible avec le profil Fabric choisi.".into());
        }
        for dep in v["dependencies"].as_array().into_iter().flatten() {
            if dep["dependency_type"] != "required" {
                continue;
            }
            let next = if let Some(id) = dep["version_id"].as_str() {
                download::json(&format!("https://api.modrinth.com/v2/version/{id}"))?
            } else {
                let id = dep["project_id"]
                    .as_str()
                    .ok_or("Dependance de mod incomplete.")?;
                let mut u =
                    url::Url::parse(&format!("https://api.modrinth.com/v2/project/{id}/version"))
                        .unwrap();
                u.query_pairs_mut()
                    .append_pair("loaders", "[\"fabric\"]")
                    .append_pair(
                        "game_versions",
                        &serde_json::to_string(&vec![&instance.minecraft]).unwrap(),
                    );
                download::json(u.as_str())?
                    .as_array()
                    .and_then(|a| a.first())
                    .cloned()
                    .ok_or("Dependance Fabric absente.")?
            };
            pending.push(next);
        }
        resolved.insert(project, v);
    }
    let base = store.game(instance)?.join("mods");
    fs::create_dir_all(&base).map_err(|_| "Dossier des mods inaccessible.")?;
    let mut index: Value = if base.join(".rusted-modrinth.json").is_file() {
        model::read_json(&base.join(".rusted-modrinth.json"))?
    } else {
        serde_json::json!({})
    };
    let mut jobs = vec![];
    let mut records = vec![];
    for (project, v) in resolved {
        let f = v["files"]
            .as_array()
            .and_then(|a| {
                a.iter()
                    .find(|f| f["primary"] == true)
                    .or_else(|| a.first())
            })
            .ok_or("Fichier du mod absent.")?;
        let name = f["filename"].as_str().ok_or("Nom du mod absent.")?;
        if !name.ends_with(".jar") || name.contains('/') {
            return Err("Nom de mod invalide.".into());
        }
        let a = Artifact {
            url: f["url"].as_str().ok_or("URL du mod absente.")?.into(),
            path: download::path(&base, name)?,
            size: f["size"].as_u64().ok_or("Taille du mod absente.")?,
            hash: f
                .pointer("/hashes/sha512")
                .and_then(Value::as_str)
                .ok_or("Empreinte du mod absente.")?
                .into(),
            algorithm: Algorithm::Sha512,
        };
        jobs.push(a);
        records.push((project, name.to_string(), v["id"].clone(), f.clone()));
    }
    download::queue(jobs, 4, task)?;
    for (project, name, version, file) in records {
        if let Some(old) = index[&project]["filename"].as_str() {
            if old != name {
                let path = download::path(&base, old)?;
                if path.is_file() {
                    fs::rename(
                        path,
                        store
                            .root
                            .join("trash")
                            .join(format!("mod-{}-{old}", model::now())),
                    )
                    .map_err(|_| "Ancienne version de mod utilisee par le jeu.")?;
                }
            }
        }
        index[project] = serde_json::json!({"filename":name,"version_id":version,"file":file});
    }
    model::atomic_json(&base.join(".rusted-modrinth.json"), &index)?;
    instance.engine = Engine::Fabric;
    store.save(instance)?;
    task.status("Mods et dependances installes. Cette instance utilisera Java / Fabric.");
    Ok(())
}
