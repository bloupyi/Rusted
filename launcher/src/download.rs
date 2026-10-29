use crate::{Event, Result, Task};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        OnceLock,
    },
    time::Duration,
};
#[derive(Clone)]
pub struct Artifact {
    pub url: String,
    pub path: PathBuf,
    pub size: u64,
    pub hash: String,
    pub algorithm: Algorithm,
}
#[derive(Clone, Copy)]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}
pub fn agent() -> ureq::Agent {
    static METADATA: OnceLock<ureq::Agent> = OnceLock::new();
    METADATA.get_or_init(|| build_agent(90)).clone()
}
fn artifact_agent() -> ureq::Agent {
    static DOWNLOADS: OnceLock<ureq::Agent> = OnceLock::new();
    DOWNLOADS.get_or_init(|| build_agent(1800)).clone()
}
fn build_agent(seconds: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .try_proxy_from_env(true)
        .max_idle_connections(64)
        .max_idle_connections_per_host(32)
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(15))
        .timeout(Duration::from_secs(seconds))
        .user_agent("Rusted-Launcher/0.2.3")
        .build()
}
pub fn https(url: &str) -> Result<()> {
    let u = url::Url::parse(url).map_err(|_| "URL invalide.")?;
    if u.scheme() != "https" || !u.username().is_empty() || u.password().is_some() {
        return Err("Seuls les telechargements HTTPS sans identifiants sont acceptes.".into());
    }
    Ok(())
}
pub fn json(url: &str) -> Result<serde_json::Value> {
    https(url)?;
    let response = agent()
        .get(url)
        .call()
        .map_err(|_| "Serveur de metadonnees inaccessible. Verifier la connexion.")?;
    let mut data = Vec::new();
    response
        .into_reader()
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|_| "Lecture des metadonnees interrompue.")?;
    if data.len() > 32 * 1024 * 1024 {
        return Err("Metadonnees trop volumineuses.".into());
    }
    serde_json::from_slice(&data).map_err(|_| "Metadonnees JSON invalides.".into())
}
pub fn path(base: &Path, name: &str) -> Result<PathBuf> {
    if name.split('/').any(|part| {
        let n = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        matches!(n.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (n.len() == 4
                && (n.starts_with("COM") || n.starts_with("LPT"))
                && matches!(n.as_bytes()[3], b'1'..=b'9'))
    }) {
        return Err("Nom reserve par Windows dans le chemin.".into());
    }
    if name.is_empty()
        || name.contains(['\\', ':', '\0'])
        || name.chars().any(char::is_control)
        || !Path::new(name)
            .components()
            .all(|p| matches!(p, Component::Normal(_)))
        || name.split('/').any(|s| {
            s.is_empty()
                || s == "."
                || s == ".."
                || s.ends_with(['.', ' '])
                || s.contains(['<', '>', '|', '?', '*'])
        })
    {
        return Err("Chemin de telechargement invalide.".into());
    }
    Ok(base.join(name))
}
pub fn digest(path: &Path, algorithm: Algorithm) -> Result<String> {
    let mut f = fs::File::open(path).map_err(|_| "Fichier du cache inaccessible.")?;
    let mut b = [0u8; 65536];
    let mut a = Sha1::new();
    let mut c = Sha256::new();
    let mut d = Sha512::new();
    loop {
        let n = f
            .read(&mut b)
            .map_err(|_| "Verification du cache interrompue.")?;
        if n == 0 {
            break;
        }
        match algorithm {
            Algorithm::Sha1 => a.update(&b[..n]),
            Algorithm::Sha256 => c.update(&b[..n]),
            Algorithm::Sha512 => d.update(&b[..n]),
        }
    }
    Ok(match algorithm {
        Algorithm::Sha1 => format!("{:x}", a.finalize()),
        Algorithm::Sha256 => format!("{:x}", c.finalize()),
        Algorithm::Sha512 => format!("{:x}", d.finalize()),
    })
}
pub fn valid(a: &Artifact) -> bool {
    a.path
        .metadata()
        .is_ok_and(|m| a.size == 0 || m.len() == a.size)
        && digest(&a.path, a.algorithm).is_ok_and(|h| h.eq_ignore_ascii_case(&a.hash))
}
fn stream(a: &Artifact, task: &Task, bytes: &AtomicU64, total: u64) -> Result<()> {
    https(&a.url)?;
    if a.size > 512 * 1024 * 1024 {
        return Err("Taille de telechargement invalide.".into());
    }
    let expected = match a.algorithm {
        Algorithm::Sha1 => 40,
        Algorithm::Sha256 => 64,
        Algorithm::Sha512 => 128,
    };
    if a.hash.len() != expected || !a.hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Empreinte de telechargement invalide.".into());
    }
    let parent = a.path.parent().ok_or("Cache invalide.")?;
    fs::create_dir_all(parent).map_err(|_| "Creation du cache impossible.")?;
    let response = artifact_agent()
        .get(&a.url)
        .call()
        .map_err(|_| "Telechargement impossible. Les fichiers verifies restent dans le cache.")?;
    let mut input = response.into_reader();
    let mut temp =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cache non accessible en ecriture.")?;
    let (mut count, mut reported) = (0u64, 0u64);
    let mut block = [0; 65536];
    loop {
        task.check()?;
        let n = input
            .read(&mut block)
            .map_err(|_| "Telechargement interrompu ; reessayer pour reprendre.")?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count
            > if a.size == 0 {
                512 * 1024 * 1024
            } else {
                a.size
            }
        {
            return Err("Le serveur a envoye un fichier trop volumineux.".into());
        }
        temp.write_all(&block[..n])
            .map_err(|_| "Espace disque insuffisant ou fichier inaccessible.")?;
        if count - reported >= 256 * 1024 {
            task.send(Event::Progress {
                done: bytes.load(Ordering::Relaxed) + count,
                total,
                label: "Telechargement et verification".into(),
            });
            reported = count;
        }
    }
    if (a.size != 0 && count != a.size)
        || digest(temp.path(), a.algorithm)? != a.hash.to_lowercase()
    {
        return Err("Empreinte incorrecte : le fichier telecharge est refuse.".into());
    }
    temp.flush().map_err(|_| "Ecriture du cache interrompue.")?;
    temp.persist(&a.path)
        .map_err(|_| "Enregistrement du fichier verifie impossible.")?;
    bytes.fetch_add(
        if a.size == 0 {
            a.path.metadata().map(|m| m.len()).unwrap_or(0)
        } else {
            a.size
        },
        Ordering::Relaxed,
    );
    Ok(())
}
pub fn queue(items: Vec<Artifact>, threads: usize, task: &Task) -> Result<()> {
    let total = items.iter().map(|a| a.size).sum::<u64>();
    let bytes = AtomicU64::new(0);
    let next = AtomicUsize::new(0);
    let failure = std::sync::Mutex::new(None);
    task.status(format!("Verification de {} fichiers", items.len()));
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, 32) {
            let items = &items;
            let bytes = &bytes;
            let next = &next;
            let failure = &failure;
            scope.spawn(move || loop {
                if task.check().is_err() {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(a) = items.get(i) else { break };
                if valid(a) {
                    bytes.fetch_add(
                        if a.size == 0 {
                            a.path.metadata().map(|m| m.len()).unwrap_or(0)
                        } else {
                            a.size
                        },
                        Ordering::Relaxed,
                    );
                } else {
                    let mut result = Err("Telechargement interrompu.".into());
                    for _ in 0..3 {
                        if task.check().is_err() {
                            break;
                        }
                        result = stream(a, task, bytes, total);
                        if result.is_ok() {
                            break;
                        }
                    }
                    if let Err(e) = result {
                        *failure.lock().unwrap() = Some(e);
                        break;
                    }
                }
                task.send(Event::Progress {
                    done: bytes.load(Ordering::Relaxed),
                    total,
                    label: format!("{} / {} fichiers", i + 1, items.len()),
                });
            });
        }
    });
    task.check()?;
    if let Some(e) = failure.into_inner().unwrap() {
        return Err(e);
    }
    task.send(Event::Progress {
        done: total,
        total,
        label: "Fichiers verifies".into(),
    });
    Ok(())
}
pub fn extract_zip(archive: &Path, destination: &Path, max: u64) -> Result<()> {
    let f = fs::File::open(archive).map_err(|_| "Archive inaccessible.")?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "Archive ZIP invalide.")?;
    if z.len() > 30000 {
        return Err("Archive contenant trop de fichiers.".into());
    }
    let mut total = 0u64;
    for i in 0..z.len() {
        let file = z.by_index(i).map_err(|_| "Archive illisible.")?;
        let name = file.name().trim_end_matches('/');
        let _ = path(destination, name)?;
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("Liens symboliques interdits dans l'archive.".into());
        }
        total = total
            .checked_add(file.size())
            .ok_or("Archive trop volumineuse.")?;
        if total > max {
            return Err("Archive trop volumineuse.".into());
        }
    }
    fs::create_dir_all(destination).map_err(|_| "Extraction impossible.")?;
    for i in 0..z.len() {
        let mut file = z.by_index(i).map_err(|_| "Archive illisible.")?;
        let p = path(destination, file.name().trim_end_matches('/'))?;
        if file.is_dir() {
            fs::create_dir_all(p).map_err(|_| "Extraction impossible.")?;
            continue;
        }
        fs::create_dir_all(p.parent().unwrap()).map_err(|_| "Extraction impossible.")?;
        let mut out = fs::File::create(p).map_err(|_| "Creation du fichier impossible.")?;
        std::io::copy(&mut file, &mut out).map_err(|_| "Extraction interrompue.")?;
    }
    Ok(())
}
