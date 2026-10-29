//! Pack import/export never reads account stores or copies the Minecraft/Java cache.
use crate::{
    download::{self, Algorithm, Artifact},
    model::{self, Engine, Instance, Store},
    Result, Task,
};
use serde_json::{json, Value};
use std::{fs, io::Read, path::Path};
fn read_zip_json(z: &mut zip::ZipArchive<fs::File>, name: &str) -> Result<Value> {
    let mut file = z.by_name(name).map_err(|_| "Metadata du pack absentes.")?;
    if file.size() > 4 * 1024 * 1024 {
        return Err("Metadata du pack trop volumineuses.".into());
    }
    let mut s = Vec::new();
    file.read_to_end(&mut s).map_err(|_| "Pack illisible.")?;
    serde_json::from_slice(&s).map_err(|_| "Metadata du pack invalides.".into())
}
pub fn import(store: &Store, file: &Path, runtime: &Path, task: &Task) -> Result<Instance> {
    task.status("Analyse du pack");
    let mut z = zip::ZipArchive::new(fs::File::open(file).map_err(|_| "Pack inaccessible.")?)
        .map_err(|_| "Le fichier n'est pas un pack ZIP ou mrpack valide.")?;
    let mrpack = z.by_name("modrinth.index.json").is_ok();
    let (meta, version, mut engine, name) = if mrpack {
        let m = read_zip_json(&mut z, "modrinth.index.json")?;
        if ["forge", "neoforge", "quilt-loader"]
            .iter()
            .any(|k| m["dependencies"].get(*k).is_some())
        {
            return Err("Ce pack exige un chargeur non pris en charge par cette version. Garder cette instance dans Prism pour le moment.".into());
        }
        let version = m
            .pointer("/dependencies/minecraft")
            .and_then(Value::as_str)
            .ok_or("Version Minecraft du pack absente.")?
            .to_owned();
        let engine = if m.pointer("/dependencies/fabric-loader").is_some() {
            Engine::Fabric
        } else {
            Engine::Java
        };
        let name = m["name"].as_str().unwrap_or("Pack Modrinth").to_string();
        (m, version, engine, name)
    } else {
        let m = read_zip_json(&mut z, "mmc-pack.json")?;
        let c = m["components"]
            .as_array()
            .ok_or("Composants Prism absents.")?;
        let version = c
            .iter()
            .find(|v| v["uid"] == "net.minecraft")
            .and_then(|v| v["version"].as_str())
            .ok_or("Version Minecraft du pack Prism absente.")?
            .to_owned();
        if c.iter().any(|v| {
            matches!(
                v["uid"].as_str(),
                Some("net.minecraftforge" | "net.neoforged" | "org.quiltmc.quilt-loader")
            )
        }) {
            return Err("Cet import prend en charge vanilla et Fabric. Garder les autres chargeurs dans Prism pour le moment.".into());
        }
        let engine = if c.iter().any(|v| v["uid"] == "net.fabricmc.fabric-loader") {
            Engine::Fabric
        } else {
            Engine::Java
        };
        (
            m,
            version,
            engine,
            file.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )
    };
    let prefix = if mrpack { "overrides/" } else { "minecraft/" };
    if z.by_name(&(prefix.to_string() + "rusted/rusted-build.json"))
        .is_ok()
    {
        engine = Engine::Native;
    }
    let mut instance = store.create(&name, engine, &version)?;
    let destination = store.game(&instance)?;
    let result = (|| {
        if z.len() > 30000 {
            return Err("Pack trop volumineux.".into());
        }
        let mut total = 0u64;
        let bootstrap_hash: Option<String> =
            model::read_json::<Value>(&runtime.join("bootstrap.json"))
                .ok()
                .and_then(|v| v["jar_sha256"].as_str().map(str::to_owned));
        for i in 0..z.len() {
            task.check()?;
            let mut f = z.by_index(i).map_err(|_| "Pack illisible.")?;
            let name = f.name().to_string();
            let logical = if let Some(p) = name.strip_prefix(prefix) {
                p
            } else if mrpack {
                if let Some(p) = name.strip_prefix("client-overrides/") {
                    p
                } else {
                    continue;
                }
            } else {
                continue;
            };
            if logical.is_empty() {
                continue;
            }
            let p = download::path(&destination, logical.trim_end_matches('/'))?;
            if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
                return Err("Liens symboliques refuses dans le pack.".into());
            }
            total = total.checked_add(f.size()).ok_or("Pack trop volumineux.")?;
            if total > 2 * 1024 * 1024 * 1024 {
                return Err("Pack trop volumineux.".into());
            }
            if f.is_dir() {
                fs::create_dir_all(p).map_err(|_| "Extraction impossible.")?;
                continue;
            }
            // The standalone launcher replaces our old Fabric bootstrap; preserve unknown files.
            if logical == "mods/rusted-bootstrap.jar" {
                use sha2::{Digest, Sha256};
                let mut b = Vec::new();
                f.by_ref()
                    .take(8 * 1024 * 1024 + 1)
                    .read_to_end(&mut b)
                    .map_err(|_| "Bootstrap illisible.")?;
                if bootstrap_hash.as_deref() == Some(&format!("{:x}", Sha256::digest(&b))) {
                    continue;
                }
                return Err(
                    "Bootstrap d'une autre version : retirer ce module du pack avant import."
                        .into(),
                );
            }
            fs::create_dir_all(p.parent().unwrap()).map_err(|_| "Extraction impossible.")?;
            let mut out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(p)
                .map_err(|_| "Fichier duplique ou extraction impossible.")?;
            std::io::copy(&mut f, &mut out).map_err(|_| "Extraction interrompue.")?;
        }
        if mrpack {
            let mut jobs = vec![];
            for f in meta["files"].as_array().into_iter().flatten() {
                if f.pointer("/env/client").and_then(Value::as_str) == Some("unsupported") {
                    continue;
                }
                let name = f["path"].as_str().ok_or("Chemin du pack absent.")?;
                let url = f["downloads"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .ok_or("URL du pack absente.")?;
                let (algorithm, hash) =
                    if let Some(h) = f.pointer("/hashes/sha512").and_then(Value::as_str) {
                        (Algorithm::Sha512, h)
                    } else {
                        (
                            Algorithm::Sha1,
                            f.pointer("/hashes/sha1")
                                .and_then(Value::as_str)
                                .ok_or("Empreinte du pack absente.")?,
                        )
                    };
                jobs.push(Artifact {
                    url: url.into(),
                    path: download::path(&destination, name)?,
                    size: f["fileSize"]
                        .as_u64()
                        .ok_or("Taille du fichier du pack absente.")?,
                    hash: hash.into(),
                    algorithm,
                });
            }
            download::queue(jobs, 4, task)?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = store.trash(&instance);
        return Err(e);
    }
    if instance.engine == Engine::Native
        && destination.join("mods").is_dir()
        && fs::read_dir(destination.join("mods"))
            .map_err(|_| "Dossier des mods inaccessible.")?
            .flatten()
            .any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("jar"))
            })
    {
        instance.engine = Engine::Fabric;
        store.save(&instance)?;
    }
    Ok(instance)
}
pub fn export_prism(
    store: &Store,
    instance: &Instance,
    runtime: &Path,
    destination: &Path,
    task: &Task,
) -> Result<()> {
    task.status("Creation du pack Prism");
    if instance.engine == Engine::Native {
        rusted_launch::Config::load(&runtime.join("rusted-runtime.json"))?.check_integrity()?;
    }
    if destination.exists() {
        return Err("Le fichier de destination existe deja. Choisir un nouveau nom.".into());
    }
    let manifest: Value = model::read_json(&runtime.join("runtime-manifest.json"))?;
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| "Destination du pack inaccessible.")?;
    let mut z = zip::ZipWriter::new(file);
    let opt = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut put = |name: &str, bytes: &[u8]| -> Result<()> {
        std::io::Write::write_all(
            &mut {
                z.start_file(name, opt)
                    .map_err(|_| "Creation ZIP impossible.")?;
                &mut z
            },
            bytes,
        )
        .map_err(|_| "Ecriture du pack impossible.".to_owned())
    };
    let components = if instance.engine == Engine::Fabric || instance.engine == Engine::Native {
        vec![
            json!({"uid":"net.minecraft","version":instance.minecraft}),
            json!({"uid":"net.fabricmc.fabric-loader","version":"0.20.1"}),
        ]
    } else {
        vec![json!({"uid":"net.minecraft","version":instance.minecraft})]
    };
    put(
        "mmc-pack.json",
        &serde_json::to_vec(&json!({"formatVersion":1,"components":components})).unwrap(),
    )?;
    let clean = instance.name.replace(['\n', '\r'], " ");
    put("instance.cfg",format!("[General]\nname={clean}\niconKey=rusted_launcher\nInstanceType=OneSix\nOverrideCommands=true\nWrapperCommand=\n").as_bytes())?;
    let icon = runtime.join("runtime/rusted-icon.png");
    if icon.is_file() {
        put(
            "icon.png",
            &fs::read(icon).map_err(|_| "Icone inaccessible.")?,
        )?;
    }
    if instance.engine == Engine::Native {
        for name in manifest["files"]
            .as_object()
            .ok_or("Manifest du runtime invalide.")?
            .keys()
        {
            task.check()?;
            let p = download::path(runtime, name)?;
            put(
                &format!("minecraft/rusted/{name}"),
                &fs::read(p).map_err(|_| "Runtime incomplet.")?,
            )?;
        }
        for name in ["runtime-manifest.json", "rusted-runtime.json"] {
            put(
                &format!("minecraft/rusted/{name}"),
                &fs::read(runtime.join(name)).map_err(|_| "Runtime incomplet.")?,
            )?;
        }
        put(
            "minecraft/mods/rusted-bootstrap.jar",
            &fs::read(runtime.join("bootstrap/rusted-bootstrap.jar"))
                .map_err(|_| "Bootstrap absent.")?,
        )?;
    }
    // User mods/settings are copied; cache, logs, accounts and worlds are deliberately outside this export.
    let game = store.game(instance)?;
    let mut folders = ["mods", "config", "resourcepacks", "shaderpacks"]
        .into_iter()
        .map(|name| (game.join(name), name.to_owned()))
        .collect::<Vec<_>>();
    if instance.engine == Engine::Native {
        for name in ["horizon-mods", "shaderpacks", "resourcepacks"] {
            let own = game.join("rusted/native-data").join(name);
            let source = if own.is_dir() {
                own
            } else {
                runtime.join("native-data").join(name)
            };
            folders.push((source, format!("rusted/native-data/{name}")));
        }
        let marker = runtime.join("native-data/.rusted-native-instance.json");
        if marker.is_file() {
            put(
                "minecraft/rusted/native-data/.rusted-native-instance.json",
                &fs::read(marker).map_err(|_| "Marqueur natif inaccessible.")?,
            )?;
        }
    }
    for (src, folder) in folders {
        if !src.is_dir() {
            continue;
        }
        let mut todo = vec![src.clone()];
        while let Some(dir) = todo.pop() {
            for e in fs::read_dir(dir)
                .map_err(|_| "Dossier de l'instance inaccessible.")?
                .flatten()
            {
                task.check()?;
                let ty = e.file_type().map_err(|_| "Fichier inaccessible.")?;
                if ty.is_symlink() {
                    return Err("Liens refuses dans l'export.".into());
                }
                if ty.is_dir() {
                    todo.push(e.path())
                } else {
                    let path = e.path();
                    let name = path
                        .strip_prefix(&src)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    put(
                        &format!("minecraft/{folder}/{name}"),
                        &fs::read(path).map_err(|_| "Fichier inaccessible.")?,
                    )?;
                }
            }
        }
    }
    z.finish().map_err(|_| "Finalisation ZIP impossible.")?;
    Ok(())
}
