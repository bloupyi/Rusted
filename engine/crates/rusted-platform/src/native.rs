//! Explicit native companions for loader JARs. Java classes are never executed.
use crate::resources::{read_entry, safe_path};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

pub const MARKER: &str = "META-INF/rusted/native.json";
const LIMIT: u64 = 64 << 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    schema: u32,
    minecraft: String,
    id: String,
    package: String,
    sha256: String,
}

pub struct NativePort {
    pub id: String,
    pub name: String,
    pub package: Vec<u8>,
    pub sha256: String,
}

/// None means the author did not supply a native port. Invalid declarations fail closed.
pub fn inspect(path: &Path) -> Result<Option<NativePort>, String> {
    let mut jar = zip::ZipArchive::new(fs::File::open(path).map_err(|_| "Cannot read mod")?)
        .map_err(|_| "Invalid mod archive")?;
    if !jar.file_names().any(|p| p == MARKER) {
        return Ok(None);
    }
    if jar.len() > 20000 {
        return Err("Too many mod files".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut size = 0u64;
    for index in 0..jar.len() {
        let f = jar.by_index(index).map_err(|_| "Unreadable mod entry")?;
        if !safe_path(f.name().trim_end_matches('/'))
            || !seen.insert(f.name().trim_end_matches('/').to_ascii_lowercase())
            || f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Unsafe or duplicate native mod path".into());
        }
        size = size.checked_add(f.size()).ok_or("Mod too large")?;
        if f.size() > LIMIT || size > 512 << 20 {
            return Err("Native mod archive too large".into());
        }
    }
    let d: Declaration = serde_json::from_slice(&read_entry(&mut jar, MARKER, 256 << 10)?)
        .map_err(|_| "Invalid Rusted native declaration")?;
    if d.schema != 1
        || d.minecraft != "26.3"
        || !safe_path(&d.package)
        || !d.package.starts_with("META-INF/rusted/")
        || !d.package.ends_with(".hmod")
    {
        return Err("Unsupported Rusted native declaration".into());
    }
    // The replacement must identify the same mod, regardless of the original Java loader.
    let mut identities = Vec::new();
    for (file, quilt) in [("fabric.mod.json", false), ("quilt.mod.json", true)] {
        if seen.contains(file) {
            let v: serde_json::Value =
                serde_json::from_slice(&read_entry(&mut jar, file, 256 << 10)?)
                    .map_err(|_| "Invalid loader metadata")?;
            identities.push(
                if quilt {
                    v["quilt_loader"]["id"].as_str()
                } else {
                    v["id"].as_str()
                }
                .ok_or("Missing loader mod id")?
                .to_owned(),
            );
        }
    }
    for file in ["META-INF/mods.toml", "META-INF/neoforge.mods.toml"] {
        if seen.contains(&file.to_ascii_lowercase()) {
            let b = read_entry(&mut jar, file, 256 << 10)?;
            let v: toml::Value =
                toml::from_str(std::str::from_utf8(&b).map_err(|_| "Invalid loader metadata")?)
                    .map_err(|_| "Invalid loader metadata")?;
            let mods = v
                .get("mods")
                .and_then(toml::Value::as_array)
                .ok_or("Missing loader mods")?;
            if mods.len() != 1 {
                return Err("One source mod per native companion is supported".into());
            }
            identities.push(
                mods[0]
                    .get("modId")
                    .and_then(toml::Value::as_str)
                    .ok_or("Missing mod id")?
                    .to_owned(),
            );
        }
    }
    if identities.is_empty() || identities.iter().any(|id| id != &d.id) {
        return Err("Native companion must match every source loader mod id".into());
    }
    let package = read_entry(&mut jar, &d.package, LIMIT)?;
    let digest = format!("{:x}", Sha256::digest(&package));
    if digest != d.sha256 {
        return Err("Native companion checksum mismatch".into());
    }
    // Validate the inner container as well, without extracting any of its paths.
    let mut inner =
        zip::ZipArchive::new(Cursor::new(&package)).map_err(|_| "Invalid native package")?;
    if inner.len() > 20000 {
        return Err("Too many native package files".into());
    }
    let mut names = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for i in 0..inner.len() {
        let f = inner.by_index(i).map_err(|_| "Unreadable native package")?;
        if !safe_path(f.name().trim_end_matches('/'))
            || !names.insert(f.name().trim_end_matches('/').to_ascii_lowercase())
            || f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Unsafe native package path".into());
        }
        total = total
            .checked_add(f.size())
            .ok_or("Native package too large")?;
        if f.size() > 32 << 20 || total > LIMIT {
            return Err("Native package too large".into());
        }
    }
    use std::io::Read;
    let mut manifest = String::new();
    let f = inner
        .by_name("horizon-mod.toml")
        .map_err(|_| "Missing native manifest")?;
    if f.size() > 256 << 10 {
        return Err("Native manifest too large".into());
    }
    f.take((256 << 10) + 1)
        .read_to_string(&mut manifest)
        .map_err(|_| "Invalid native manifest")?;
    let m: toml::Value = toml::from_str(&manifest).map_err(|_| "Invalid native manifest")?;
    let id = m
        .get("id")
        .and_then(toml::Value::as_str)
        .ok_or("Missing native id")?;
    if id != d.id
        || id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        || !matches!(
            m.get("api").and_then(toml::Value::as_str),
            Some("0.1" | "0.2" | "0.3")
        )
    {
        return Err("Native identity or API mismatch".into());
    }
    let name = m
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or("Missing native name")?
        .to_owned();
    let mut wasm = inner
        .by_name("mod.wasm")
        .map_err(|_| "Missing native code")?;
    let mut header = [0; 8];
    wasm.read_exact(&mut header)
        .map_err(|_| "Invalid native component")?;
    if header != *b"\0asm\x0d\0\x01\0" {
        return Err("Native code must be a WASM component".into());
    }
    drop(wasm);
    drop(inner);
    Ok(Some(NativePort {
        id: d.id,
        name,
        package,
        sha256: digest,
    }))
}

/// Cache only the companion bytes. Do not extract Java classes or arbitrary archive paths.
/// Rebuild from the JAR after updates/removal/disable; never install a persistent loose port.
pub fn discover(directory: &Path, cache: &Path) -> Result<Vec<PathBuf>, String> {
    let report = crate::scan(directory);
    let mut paths = Vec::new();
    for entry in report.mods.into_iter().filter(|m| m.native_compatible) {
        let Some(port) = inspect(&directory.join(entry.file))? else {
            continue;
        };
        fs::create_dir_all(cache).map_err(|_| "Cannot create native port cache")?;
        let path = cache.join(format!("{}.hmod", port.sha256));
        // Repair a stale/corrupt cached copy, and publish complete bytes atomically.
        if fs::read(&path).ok().as_deref() != Some(port.package.as_slice()) {
            use std::io::Write;
            let mut tmp =
                tempfile::NamedTempFile::new_in(cache).map_err(|_| "Cannot stage native port")?;
            tmp.write_all(&port.package)
                .map_err(|_| "Cannot cache native port")?;
            tmp.persist(&path)
                .map_err(|_| "Cannot publish native port")?;
        }
        paths.push(path);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests;
