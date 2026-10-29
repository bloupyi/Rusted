use crate::{
    download::{self, Algorithm, Artifact},
    model::{self, Engine, Instance, Store},
    Result, Task,
};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone)]
pub struct Ready {
    pub metadata: Value,
    pub fabric: Option<Value>,
    pub java: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub assets: PathBuf,
    pub natives: PathBuf,
    pub root: PathBuf,
}
fn s<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Metadata absente : {key}"))
}
fn raw_metadata(url: &str, hash: Option<&str>, path: &Path) -> Result<Value> {
    if path.is_file() {
        let bytes = fs::read(path).map_err(|_| "Cache de metadonnees inaccessible.")?;
        if hash.is_none_or(|h| format!("{:x}", Sha1::digest(&bytes)) == h) {
            if let Ok(v) = serde_json::from_slice(&bytes) {
                return Ok(v);
            }
        }
    }
    download::https(url)?;
    let mut data = Vec::new();
    download::agent()
        .get(url)
        .call()
        .map_err(|_| "Metadonnees Minecraft/Fabric inaccessibles.")?
        .into_reader()
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|_| "Metadonnees interrompues.")?;
    if data.len() > 32 * 1024 * 1024
        || hash.is_some_and(|h| format!("{:x}", Sha1::digest(&data)) != h)
    {
        return Err("Metadonnees non conformes a leur empreinte.".into());
    }
    let value = serde_json::from_slice(&data).map_err(|_| "Metadonnees JSON invalides.")?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|_| "Cache inaccessible.")?;
    let mut t = tempfile::NamedTempFile::new_in(path.parent().unwrap())
        .map_err(|_| "Cache inaccessible.")?;
    t.write_all(&data).map_err(|_| "Cache inaccessible.")?;
    t.persist(path).map_err(|_| "Cache inaccessible.")?;
    Ok(value)
}
pub fn versions(store: &Store, snapshots: bool) -> Result<Vec<String>> {
    Ok(crate::catalog::refresh(store)?.ids(snapshots))
}
pub fn metadata(store: &Store, version: &str) -> Result<Value> {
    let path = download::path(
        &store.root.join("cache/minecraft/versions"),
        &format!("{version}/{version}.json"),
    )?;
    let value = if crate::catalog::unobfuscated_id(version) {
        let mut catalogue = crate::catalog::cached(store);
        if catalogue.find(version).is_none() {
            catalogue = crate::catalog::refresh(store)?;
        }
        let entry = catalogue
            .find(version)
            .ok_or("Version absente du catalogue officiel Mojang.")?;
        let value = raw_metadata(&entry.url, Some(&entry.sha1), &path)?;
        crate::catalog::validate_metadata(version, &value)?;
        value
    } else {
        let all = download::json(crate::catalog::MANIFEST_URL)?;
        let v = all["versions"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == version))
            .ok_or("Version Minecraft inconnue.")?;
        raw_metadata(s(v, "url")?, Some(s(v, "sha1")?), &path)?
    };
    Ok(value)
}
pub fn allowed(rules: &Value) -> bool {
    let Some(rules) = rules.as_array() else {
        return true;
    };
    let mut allow = false;
    for r in rules {
        let os = &r["os"];
        let name = os["name"].as_str().unwrap_or("windows");
        let arch = os["arch"].as_str().unwrap_or("x86_64");
        if name != "windows" || !matches!(arch, "x86_64" | "amd64") {
            continue;
        }
        if r["features"]
            .as_object()
            .is_some_and(|m| m.values().any(|v| v == true))
        {
            continue;
        }
        allow = r["action"] == "allow";
    }
    allow
}
fn artifact(v: &Value, path: PathBuf) -> Result<Artifact> {
    Ok(Artifact {
        url: s(v, "url")?.into(),
        size: v["size"].as_u64().ok_or("Taille de fichier absente.")?,
        hash: s(v, "sha1")?.into(),
        algorithm: Algorithm::Sha1,
        path,
    })
}
pub fn java(store: &Store, instance: &Instance, task: &Task) -> Result<PathBuf> {
    let major = if instance.java_path.trim().is_empty() {
        java_major(&metadata(store, &instance.minecraft)?)?
    } else {
        25
    };
    java_for_major(store, instance, major, task)
}
fn java_major(metadata: &Value) -> Result<u64> {
    metadata["javaVersion"]["majorVersion"]
        .as_u64()
        .filter(|n| (17..=99).contains(n))
        .ok_or("Version Java officielle absente ou non prise en charge.")
        .map_err(str::to_owned)
}
fn java_for_major(store: &Store, instance: &Instance, major: u64, task: &Task) -> Result<PathBuf> {
    if !instance.java_path.trim().is_empty() {
        let p = PathBuf::from(&instance.java_path);
        if !p.is_file() {
            return Err("Le chemin Java personnalise est introuvable.".into());
        }
        return Ok(p);
    }
    let root = store.root.join(format!("cache/java/temurin{major}"));
    let ready = root.join("ready.json");
    if ready.is_file() {
        let v: Value = model::read_json(&ready)?;
        let p = download::path(&root, s(&v, "executable")?)?;
        if p.is_file() && download::digest(&p, Algorithm::Sha256)? == s(&v, "sha256")? {
            return Ok(p);
        }
    }
    task.status(format!("Preparation de Java {major} pour Windows x64"));
    let all=download::json(&format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture=x64&image_type=jre&os=windows&vendor=eclipse"))?;
    let package = all
        .pointer("/0/binary/package")
        .ok_or("Runtime Java Windows indisponible pour cette version.")?;
    let name = s(package, "name")?;
    if !name.ends_with(".zip") {
        return Err("Archive Java Windows inattendue.".into());
    }
    let archive = download::path(&store.root.join("cache/downloads"), name)?;
    download::queue(
        vec![Artifact {
            url: s(package, "link")?.into(),
            path: archive.clone(),
            size: package["size"].as_u64().ok_or("Taille Java absente.")?,
            hash: s(package, "checksum")?.into(),
            algorithm: Algorithm::Sha256,
        }],
        1,
        task,
    )?;
    let base = store.root.join("cache/java");
    fs::create_dir_all(&base).map_err(|_| "Dossier Java inaccessible.")?;
    let staging = tempfile::tempdir_in(&base).map_err(|_| "Preparation Java impossible.")?;
    task.status("Extraction du runtime Java verifie");
    download::extract_zip(&archive, staging.path(), 512 * 1024 * 1024)?;
    let mut executable = None;
    for e in fs::read_dir(staging.path())
        .map_err(|_| "Runtime Java incomplet.")?
        .flatten()
    {
        let p = e.path().join("bin/java.exe");
        if p.is_file() {
            executable = Some(p);
            break;
        }
    }
    let p = executable.ok_or("Archive Java sans bin/java.exe.")?;
    let relative = p
        .strip_prefix(staging.path())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let hash = download::digest(&p, Algorithm::Sha256)?;
    model::atomic_json(
        &staging.path().join("ready.json"),
        &json!({"executable":relative,"sha256":hash}),
    )?;
    if root.exists() {
        fs::rename(
            &root,
            store
                .root
                .join("trash")
                .join(format!("java-{}", model::now())),
        )
        .map_err(|_| "Ancien cache Java en cours d'utilisation.")?;
    }
    let p = staging.keep();
    fs::rename(&p, &root).map_err(|_| "Installation Java impossible.")?;
    download::path(&root, &relative)
}
pub fn prepare(store: &Store, instance: &Instance, runtime: &Path, task: &Task) -> Result<Ready> {
    use fs2::FileExt;
    instance.validate()?;
    task.check()?;
    if instance.engine == Engine::Native {
        let config = rusted_launch::Config::load(&runtime.join("rusted-runtime.json"))?;
        config.check_integrity()?;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.root.join("cache/prepare.lock"))
        .map_err(|_| "Verrou du cache inaccessible.")?;
    lock.try_lock_exclusive()
        .map_err(|_| "Une autre preparation utilise deja le cache.")?;
    task.status("Lecture de la version Minecraft");
    let meta = metadata(store, &instance.minecraft)?;
    if !meta["arguments"]["game"].is_array() {
        return Err("Cette version ancienne utilise un lancement legacy qui n'est pas encore pris en charge. Choisir une version Minecraft moderne.".into());
    }
    let root = store.root.join("cache/minecraft");
    let mut jobs = vec![];
    let mut classpath = vec![];
    let client = download::path(&root, &format!("versions/{0}/{0}.jar", instance.minecraft))?;
    jobs.push(artifact(&meta["downloads"]["client"], client.clone())?);
    for lib in meta["libraries"]
        .as_array()
        .ok_or("Liste de bibliotheques absente.")?
    {
        if !allowed(&lib["rules"]) {
            continue;
        }
        if let Some(a) = lib["downloads"].get("artifact") {
            let path = download::path(&root.join("libraries"), s(a, "path")?)?;
            jobs.push(artifact(a, path.clone())?);
            classpath.push(path);
        }
        if let Some(classifier) = lib.pointer("/natives/windows").and_then(Value::as_str) {
            let key = classifier.replace("${arch}", "64");
            if let Some(a) = lib["downloads"]["classifiers"].get(key) {
                let path = download::path(&root.join("libraries"), s(a, "path")?)?;
                jobs.push(artifact(a, path.clone())?);
                classpath.push(path);
            }
        }
    }
    classpath.push(client);
    let settings = store.settings()?;
    task.status("Minecraft et bibliotheques");
    download::queue(
        jobs,
        if settings.download_threads == 0 {
            24
        } else {
            settings.download_threads.clamp(1, 32)
        },
        task,
    )?;
    let assets = root.join("assets");
    let index = &meta["assetIndex"];
    let index_path = download::path(&assets, &format!("indexes/{}.json", s(index, "id")?))?;
    task.status("Index des ressources Minecraft");
    download::queue(vec![artifact(index, index_path.clone())?], 1, task)?;
    let objects: Value = model::read_json(&index_path)?;
    let mut jobs = vec![];
    for object in objects["objects"]
        .as_object()
        .ok_or("Liste des assets absente.")?
        .values()
    {
        let hash = s(object, "hash")?;
        if hash.len() != 40 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Empreinte d'asset invalide.".into());
        }
        jobs.push(Artifact {
            url: format!(
                "https://resources.download.minecraft.net/{}/{}",
                &hash[..2],
                hash
            ),
            path: assets.join(format!("objects/{}/{}", &hash[..2], hash)),
            size: object["size"].as_u64().ok_or("Taille d'asset absente.")?,
            hash: hash.into(),
            algorithm: Algorithm::Sha1,
        });
    }
    task.status("Prechargement des textures, sons et langues");
    download::queue(
        jobs,
        if settings.download_threads == 0 {
            24
        } else {
            settings.download_threads.clamp(1, 32)
        },
        task,
    )?;
    let java = java_for_major(store, instance, java_major(&meta)?, task)?;
    let natives = root.join("natives").join(&instance.minecraft);
    fs::create_dir_all(&natives).map_err(|_| "Dossier des bibliotheques natives inaccessible.")?;
    for path in &classpath {
        if !path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .contains("natives-windows")
        {
            continue;
        }
        let mut z = zip::ZipArchive::new(
            fs::File::open(path).map_err(|_| "Bibliotheque native inaccessible.")?,
        )
        .map_err(|_| "Bibliotheque native invalide.")?;
        for i in 0..z.len() {
            let mut f = z.by_index(i).map_err(|_| "Bibliotheque native invalide.")?;
            if f.name().ends_with(".dll") {
                let name = Path::new(f.name()).file_name().ok_or("DLL invalide.")?;
                let mut out = fs::File::create(natives.join(name))
                    .map_err(|_| "Extraction de DLL impossible.")?;
                std::io::copy(&mut f, &mut out).map_err(|_| "Extraction de DLL interrompue.")?;
            }
        }
    }
    let fabric = if instance.engine == Engine::Fabric {
        Some(fabric(store, &instance.minecraft, &mut classpath, task)?)
    } else {
        None
    };
    let ready = Ready {
        metadata: meta,
        fabric,
        java,
        classpath,
        assets,
        natives,
        root,
    };
    if instance.engine == Engine::Native {
        prepare_native(store, instance, runtime, &ready, task)?;
    }
    task.status("Fichiers de base prepares. Les telechargements du lanceur sont termines.");
    Ok(ready)
}
fn fabric(
    store: &Store,
    version: &str,
    classpath: &mut Vec<PathBuf>,
    task: &Task,
) -> Result<Value> {
    task.status("Installation de Fabric Loader");
    let profile = raw_metadata(
        &format!("https://meta.fabricmc.net/v2/versions/loader/{version}/0.20.1/profile/json"),
        None,
        &store.root.join(format!("cache/fabric-{version}.json")),
    )?;
    let mut jobs = vec![];
    let mut front = vec![];
    for lib in profile["libraries"]
        .as_array()
        .ok_or("Profil Fabric incomplet.")?
    {
        let name = s(lib, "name")?;
        let parts = name.split(':').collect::<Vec<_>>();
        if parts.len() != 3 {
            return Err("Nom de bibliotheque Fabric invalide.".into());
        }
        let relative = format!(
            "{}/{}/{}/{}-{}.jar",
            parts[0].replace('.', "/"),
            parts[1],
            parts[2],
            parts[1],
            parts[2]
        );
        let url = format!(
            "{}{}",
            lib["url"].as_str().unwrap_or("https://maven.fabricmc.net/"),
            relative
        );
        let path = download::path(&store.root.join("cache/minecraft/libraries"), &relative)?;
        let hash = if let Some(h) = lib["sha256"].as_str() {
            h.to_owned()
        } else {
            let mut s = String::new();
            download::agent()
                .get(&(url.clone() + ".sha256"))
                .call()
                .map_err(|_| "Empreinte Fabric indisponible.")?
                .into_reader()
                .take(1024)
                .read_to_string(&mut s)
                .map_err(|_| "Empreinte Fabric invalide.")?;
            s.split_whitespace().next().unwrap_or("").into()
        };
        let size = if let Some(n) = lib["size"].as_u64() {
            n
        } else {
            download::agent()
                .head(&url)
                .call()
                .ok()
                .and_then(|r| r.header("Content-Length").and_then(|s| s.parse().ok()))
                .unwrap_or(0)
        };
        jobs.push(Artifact {
            url,
            path: path.clone(),
            size,
            hash,
            algorithm: Algorithm::Sha256,
        });
        front.push(path);
    }
    download::queue(jobs, 4, task)?;
    front.append(classpath);
    *classpath = front;
    Ok(profile)
}
fn prepare_native(
    store: &Store,
    instance: &Instance,
    runtime: &Path,
    ready: &Ready,
    task: &Task,
) -> Result<()> {
    use rusted_launch::{
        preparation::{self, Preparation},
        Config, Mode,
    };
    task.status("Preparation des ressources du moteur Rusted avant ouverture du jeu");
    let prep = store.root.join("cache/native/preparation");
    let server = store.root.join("cache/minecraft/server-26.3.jar");
    let server_meta = &ready.metadata["downloads"]["server"];
    download::queue(vec![artifact(server_meta, server.clone())?], 1, task)?;
    let config = Config {
        schema: 1,
        mode: Mode::Native,
        horizon_revision: rusted_launch::HORIZON_REVISION.into(),
        minecraft_version: "26.3".into(),
        executable: runtime.join("runtime/rusted.exe"),
        resources: prep.join(".local/horizon-client/26.3"),
        data_directory: store.game(instance)?.join("rusted/native-data"),
        preparation: Some(Preparation {
            root: prep,
            executable: runtime.join("runtime/rusted-prepare.exe"),
            server_jar: Some(server),
            minecraft_directory: None,
            download_server: false,
            import_launcher: true,
        }),
        integrity_manifest: Some(runtime.join("runtime-manifest.json")),
    };
    config.check_integrity()?;
    task.check()?;
    let source = runtime.join("runtime/preparation");
    let fingerprint = download::digest(&source.join("exporters/manifest.json"), Algorithm::Sha256)?;
    let stamp = prep_stamp(&config)?;
    if fs::read_to_string(&stamp).unwrap_or_default() != fingerprint {
        model::copy_tree(&source, &config.preparation.as_ref().unwrap().root)?;
        let _ = fs::remove_file(
            config
                .preparation
                .as_ref()
                .unwrap()
                .root
                .join(".local/rusted-ready.json"),
        );
        fs::write(&stamp, &fingerprint).map_err(|_| "Cache natif inaccessible.")?;
    }
    if !preparation::ready(&config) {
        let p = config.preparation.as_ref().unwrap();
        task.status("Liaison des fichiers Minecraft verifies");
        let files = rusted_launch::launcher_install::LauncherFiles {
            jars: ready.classpath.clone(),
            assets: ready.assets.clone(),
        };
        rusted_launch::launcher_install::import(p, &files)?;
        let mut frame = tempfile::NamedTempFile::new_in(store.root.join("cache"))
            .map_err(|_| "Configuration de preparation inaccessible.")?;
        let value = json!({"schema":1,"mode":"native","horizon_revision":config.horizon_revision,"minecraft_version":"26.3","executable":config.executable,"resources":config.resources,"data_directory":config.data_directory,"integrity_manifest":config.integrity_manifest,
          "preparation":{"root":p.root,"executable":p.executable,"server_jar":p.server_jar,"download_server":false,"import_launcher":true}});
        serde_json::to_writer(frame.as_file_mut(), &value)
            .map_err(|_| "Configuration native invalide.")?;
        frame
            .flush()
            .map_err(|_| "Configuration native inaccessible.")?;
        let mut child = std::process::Command::new(runtime.join("rusted-launch.exe"));
        child
            .arg("--config")
            .arg(frame.path())
            .arg("--prepare")
            .arg("--minecraft-dir")
            .arg(store.game(instance)?)
            .arg("--java")
            .arg(&ready.java);
        task.status("Preparation native : blocs, modeles, recettes et sons");
        let status = crate::launch::run(child, task, &[])?;
        if status != 0 {
            return Err(
                "La preparation native a echoue. Consulter la console ; aucun jeu n'a ete ouvert."
                    .into(),
            );
        }
    }
    let home = store.game(instance)?.join("rusted/native-data");
    model::seed_tree(&runtime.join("native-data"), &home)?;
    fs::create_dir_all(&home).map_err(|_| "Donnees natives inaccessibles.")?;
    task.check()?;
    config.check_native()?;
    Ok(())
}

fn prep_stamp(config: &rusted_launch::Config) -> Result<PathBuf> {
    let p = config
        .preparation
        .as_ref()
        .ok_or("Preparation absente.")?
        .root
        .clone();
    fs::create_dir_all(&p).map_err(|_| "Cache natif inaccessible.")?;
    Ok(p.join(".rusted-exporter-source"))
}
