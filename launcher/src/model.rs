use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const VERSION: &str = "0.2.1";
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Native,
    Java,
    Fabric,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Instance {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub group: String,
    pub notes: String,
    pub minecraft: String,
    pub engine: Engine,
    pub memory_mb: u32,
    pub java_path: String,
    pub java_args: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub graphics_api: String,
    pub render_distance: u32,
    pub auto_prepare: bool,
    pub last_played: u64,
    pub play_seconds: u64,
}
impl Default for Instance {
    fn default() -> Self {
        Self {
            schema: 1,
            id: String::new(),
            name: "Nouvelle instance".into(),
            group: "Mes instances".into(),
            notes: String::new(),
            minecraft: "26.3".into(),
            engine: Engine::Native,
            memory_mb: 4096,
            java_path: String::new(),
            java_args: vec![],
            width: 1280,
            height: 720,
            fullscreen: false,
            graphics_api: "auto".into(),
            render_distance: 8,
            auto_prepare: true,
            last_played: 0,
            play_seconds: 0,
        }
    }
}
impl Instance {
    pub fn validate(&self) -> Result<()> {
        safe_id(&self.id)?;
        if self.schema != 1
            || self.name.trim().is_empty()
            || self.name.len() > 120
            || !(512..=65536).contains(&self.memory_mb)
            || !(320..=7680).contains(&self.width)
            || !(240..=4320).contains(&self.height)
            || !(2..=64).contains(&self.render_distance)
            || !matches!(
                self.graphics_api.as_str(),
                "auto" | "vulkan" | "dx12" | "opengl"
            )
        {
            return Err("Reglages d'instance invalides.".into());
        }
        if self.engine == Engine::Native && self.minecraft != "26.3" {
            return Err(
                "Le moteur natif cible Minecraft 26.3. Les autres versions utilisent Java.".into(),
            );
        }
        if self.minecraft.is_empty()
            || self.minecraft.len() > 80
            || self.minecraft.contains(['/', '\\', ':'])
        {
            return Err("Version Minecraft invalide.".into());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub microsoft_client_id: String,
    pub selected: String,
    pub selected_account: String,
    pub hide_while_playing: bool,
    pub show_snapshots: bool,
    pub download_threads: usize,
}
#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        for dir in ["instances", "cache", "accounts", "trash", "locks"] {
            fs::create_dir_all(root.join(dir))
                .map_err(|_| "Creation du dossier du lanceur impossible.")?;
        }
        Ok(Self {
            root: canonical(&root)?,
        })
    }
    pub fn settings(&self) -> Result<Settings> {
        read_or_default(&self.root.join("settings.json"))
    }
    pub fn save_settings(&self, v: &Settings) -> Result<()> {
        atomic_json(&self.root.join("settings.json"), v)
    }
    pub fn path(&self, id: &str) -> Result<PathBuf> {
        safe_id(id)?;
        Ok(self.root.join("instances").join(id))
    }
    pub fn list(&self) -> Result<Vec<Instance>> {
        let mut out = vec![];
        for item in fs::read_dir(self.root.join("instances"))
            .map_err(|_| "Liste des instances inaccessible.")?
            .flatten()
        {
            if !item.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let p = item.path().join("instance.json");
            if !p.is_file() {
                continue;
            }
            let v: Instance = read_json(&p)?;
            v.validate()?;
            out.push(v);
        }
        out.sort_by_key(|v| (v.group.to_lowercase(), v.name.to_lowercase()));
        Ok(out)
    }
    pub fn create(&self, name: &str, engine: Engine, minecraft: &str) -> Result<Instance> {
        let temp = tempfile::Builder::new()
            .prefix("instance-")
            .tempdir_in(self.root.join("instances"))
            .map_err(|_| "Creation de l'instance impossible.")?;
        let id = temp
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let v = Instance {
            id,
            name: name.trim().into(),
            engine,
            minecraft: minecraft.into(),
            ..Instance::default()
        };
        v.validate()?;
        fs::create_dir(temp.path().join("minecraft"))
            .map_err(|_| "Dossier Minecraft inaccessible.")?;
        atomic_json(&temp.path().join("instance.json"), &v)?;
        let _ = temp.keep();
        Ok(v)
    }
    pub fn save(&self, v: &Instance) -> Result<()> {
        v.validate()?;
        let path = self.path(&v.id)?;
        if !path.join("instance.json").is_file() {
            return Err("Instance absente.".into());
        }
        atomic_json(&path.join("instance.json"), v)
    }
    pub fn duplicate(&self, v: &Instance) -> Result<Instance> {
        let _source_lock = crate::launch::profile_lock(self, v)?;
        let mut copy = self.create(&(v.name.clone() + " - copie"), v.engine, &v.minecraft)?;
        let id = copy.id.clone();
        copy = v.clone();
        copy.id = id;
        copy.name.push_str(" - copie");
        copy.last_played = 0;
        copy.play_seconds = 0;
        if let Err(error) = copy_tree(
            &self.path(&v.id)?.join("minecraft"),
            &self.path(&copy.id)?.join("minecraft"),
        ) {
            let _ = self.trash(&copy);
            return Err(error);
        }
        self.save(&copy)?;
        Ok(copy)
    }
    pub fn trash(&self, v: &Instance) -> Result<()> {
        let _lock = crate::launch::profile_lock(self, v)?;
        let src = self.path(&v.id)?;
        let dst = self.root.join("trash").join(format!("{}-{}", v.id, now()));
        fs::rename(src, dst)
            .map_err(|_| "Impossible de deplacer l'instance vers la corbeille Rusted.".into())
    }
    pub fn game(&self, v: &Instance) -> Result<PathBuf> {
        Ok(self.path(&v.id)?.join("minecraft"))
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn safe_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        Err("Identifiant d'instance invalide.".into())
    } else {
        Ok(())
    }
}
pub fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T> {
    let b = fs::read(p).map_err(|_| "Fichier de configuration illisible.")?;
    if b.len() > 4 * 1024 * 1024 {
        return Err("Configuration trop volumineuse.".into());
    }
    serde_json::from_slice(&b).map_err(|_| "Configuration JSON invalide.".into())
}
pub fn read_or_default<T: serde::de::DeserializeOwned + Default>(p: &Path) -> Result<T> {
    if !p.exists() {
        Ok(T::default())
    } else {
        read_json(p)
    }
}
pub fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().ok_or("Dossier de configuration absent.")?;
    fs::create_dir_all(parent).map_err(|_| "Dossier non accessible en ecriture.")?;
    let mut t = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "Creation de la configuration impossible.")?;
    serde_json::to_writer_pretty(t.as_file_mut(), value)
        .map_err(|_| "Ecriture JSON impossible.")?;
    t.flush()
        .and_then(|_| t.as_file().sync_all())
        .map_err(|_| "Ecriture de la configuration incomplete.")?;
    t.persist(path)
        .map_err(|_| "Enregistrement de la configuration impossible.")?;
    Ok(())
}
pub fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|_| "Copie impossible.")?;
    for e in fs::read_dir(src).map_err(|_| "Source inaccessible.")? {
        let e = e.map_err(|_| "Lecture de la source impossible.")?;
        let t = e.file_type().map_err(|_| "Type de fichier inaccessible.")?;
        if t.is_symlink() {
            return Err("La copie refuse les liens symboliques.".into());
        }
        if t.is_dir() {
            copy_tree(&e.path(), &dst.join(e.file_name()))?
        } else if t.is_file() {
            fs::copy(e.path(), dst.join(e.file_name()))
                .map_err(|_| "Copie d'un fichier impossible.")?;
        }
    }
    Ok(())
}

pub fn canonical(path: &Path) -> Result<PathBuf> {
    let p = fs::canonicalize(path).map_err(|_| "Dossier du lanceur inaccessible.")?;
    #[cfg(windows)]
    {
        if let Some(s) = p.to_str() {
            if let Some(u) = s.strip_prefix(r"\\?\UNC\") {
                return Ok(PathBuf::from(format!(r"\\{u}")));
            }
            if let Some(s) = s.strip_prefix(r"\\?\") {
                return Ok(PathBuf::from(s));
            }
        }
    }
    Ok(p)
}

pub fn seed_tree(source: &Path, dest: &Path) -> Result<()> {
    if !source.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(dest).map_err(|_| "Initialisation des donnees impossible.")?;
    for e in fs::read_dir(source)
        .map_err(|_| "Donnees par defaut absentes.")?
        .flatten()
    {
        let t = e
            .file_type()
            .map_err(|_| "Donnees par defaut inaccessibles.")?;
        let target = dest.join(e.file_name());
        if t.is_symlink() {
            return Err("Lien interdit dans les donnees par defaut.".into());
        }
        if t.is_dir() {
            seed_tree(&e.path(), &target)?
        } else if !target.exists() {
            fs::copy(e.path(), target).map_err(|_| "Initialisation des donnees impossible.")?;
        }
    }
    Ok(())
}
