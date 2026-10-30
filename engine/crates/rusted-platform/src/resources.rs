//! Loader metadata and passive files only: never execute or inspect Java bytecode.
mod metadata;
#[cfg(test)]
mod tests;
use std::{collections::BTreeSet, fs, io::Read, path::Path};

const MAX_METADATA: u64 = 256 * 1024;
const MAX_FILE: u64 = 16 * 1024 * 1024;
const MAX_TOTAL: u64 = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct ResourceMod {
    pub id: String,
    pub name: String,
    pub assets: Vec<String>,
    pub data: Vec<String>,
}

/// Paths must have the same meaning on Unix and Windows, including case collisions.
fn safe_path(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', ':'])
        && !name.chars().any(char::is_control)
        && name.split('/').all(|part| {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !part.is_empty()
                && !matches!(part, "." | "..")
                && !part.ends_with(['.', ' '])
                && !matches!(
                    stem.as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
}

fn resource_path(name: &str) -> bool {
    name.bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_./-".contains(&c))
        && name.split('/').count() >= 3
}

pub fn read_entry(
    archive: &mut zip::ZipArchive<fs::File>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|_| format!("Missing mod file: {name}"))?;
    if file.size() > limit {
        return Err(format!("Mod file exceeds its limit: {name}"));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("Unreadable mod file: {name}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("Mod file exceeds its limit: {name}"));
    }
    Ok(bytes)
}

/// Fabric, Quilt, and lowcode Forge/NeoForge archives with supported resources/data.
/// A single code entry, Java hook, unsupported dependency or data kind rejects the archive.
pub fn inspect(path: &Path) -> Result<ResourceMod, String> {
    let file = fs::File::open(path).map_err(|_| "Cannot read mod archive")?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "Invalid mod archive")?;
    if archive.len() > 20000 {
        return Err("Too many files in mod archive".into());
    }
    let mut names = BTreeSet::new();
    let mut files = Vec::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| "Unreadable mod archive")?;
        let name = entry.name().trim_end_matches('/');
        if !safe_path(name)
            || !names.insert(name.to_ascii_lowercase())
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Unsafe, duplicate or symbolic mod archive path".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Mod archive too large")?;
        if entry.size() > MAX_FILE || total > MAX_TOTAL {
            return Err("Mod archive too large".into());
        }
        if !entry.is_dir() {
            let lower = name.to_ascii_lowercase();
            if [".class", ".jar", ".wasm", ".exe", ".dll", ".so", ".dylib"]
                .iter()
                .any(|suffix| lower.ends_with(suffix))
            {
                return Err("Java/native code detected; a native port or the original JVM loader is required".into());
            }
            files.push(name.to_owned());
        }
    }
    let (id, name, icon) = metadata::read(&mut archive, &files, MAX_METADATA)?;
    let mut result = ResourceMod {
        id,
        name,
        assets: vec![],
        data: vec![],
    };
    for name in files {
        if name.starts_with("assets/") {
            if !resource_path(&name)
                || name.split('/').nth(2) == Some("shaders")
                || ![".json", ".png", ".mcmeta", ".ogg", ".ttf"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
            {
                return Err(format!("Unsupported native resource: {name}"));
            }
            result.assets.push(name);
        } else if name.starts_with("data/") {
            let kind = name.splitn(3, '/').nth(2).unwrap_or("");
            let supported = (kind.starts_with("function/") && name.ends_with(".mcfunction"))
                || (["predicate/", "item_modifier/", "tags/function/"]
                    .iter()
                    .any(|prefix| kind.starts_with(prefix))
                    && name.ends_with(".json"));
            if !resource_path(&name) || !supported {
                return Err(format!("Unsupported native mod data: {name}; supported: functions, function tags, predicates, item modifiers"));
            }
            result.data.push(name);
        } else if name == "pack.mcmeta" {
            let bytes = read_entry(&mut archive, &name, MAX_METADATA)?;
            let metadata: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid pack metadata")?;
            if metadata.get("overlays").is_some() || metadata.get("filter").is_some() {
                return Err(
                    "Conditional pack overlays/filters require an explicit native adapter".into(),
                );
            }
        } else if !matches!(
            name.as_str(),
            "fabric.mod.json" | "quilt.mod.json" | "pack.png"
        ) && name != icon
            && !name.starts_with("META-INF/")
            && !name.to_ascii_lowercase().starts_with("license")
            && !name.to_ascii_lowercase().starts_with("readme")
        {
            return Err(format!(
                "Unrecognized mod file requires a native adapter: {name}"
            ));
        }
    }
    if result.assets.is_empty() && result.data.is_empty() {
        return Err("No supported native resources or data".into());
    }
    for file in result.assets.iter().chain(&result.data) {
        read_entry(&mut archive, file, MAX_FILE)?;
    }
    result.assets.sort();
    result.data.sort();
    Ok(result)
}

/// Reuse the inventory's duplicate/error decisions in game resource and data loaders.
pub fn discover(directory: &Path) -> Vec<(std::path::PathBuf, ResourceMod)> {
    crate::scan(directory)
        .mods
        .into_iter()
        .filter(|m| m.native_compatible)
        .filter_map(|m| {
            let path = directory.join(m.file);
            inspect(&path).ok().map(|resource_mod| (path, resource_mod))
        })
        .collect()
}
