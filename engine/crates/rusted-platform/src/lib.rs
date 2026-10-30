//! Rusted's launcher integration. Reading metadata never executes Java or loads a mod.
use serde::Serialize;
pub mod resources;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const NAME: &str = "Rusted";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const UPSTREAM_REVISION: &str = "8993772de0bf4b0f92c9f76be64f7a1e6b21cf37";
pub const CREDITS: &str = "Based on Horizon (MIT), Ludovic Marie and contributors";
const MAX_METADATA: u64 = 256 * 1024;

#[derive(Debug, Serialize)]
pub struct JavaMod {
    pub id: Option<String>,
    pub file: String,
    pub name: String,
    pub loaders: Vec<String>,
    pub native_compatible: bool,
    pub diagnostic: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: u32,
    pub project: &'static str,
    pub version: &'static str,
    pub directory: PathBuf,
    pub mods: Vec<JavaMod>,
    pub errors: Vec<String>,
}

fn inspect(path: &Path) -> Result<JavaMod, String> {
    let file = fs::File::open(path).map_err(|_| "Cannot read archive")?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "Invalid JAR/ZIP archive")?;
    let mut loaders = Vec::new();
    let mut name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    for (entry, loader, name_key) in [
        ("fabric.mod.json", "Fabric", Some("name")),
        (
            "quilt.mod.json",
            "Quilt",
            Some("quilt_loader.metadata.name"),
        ),
        ("META-INF/mods.toml", "Forge", None),
        ("META-INF/neoforge.mods.toml", "NeoForge", None),
    ] {
        let Ok(mut metadata) = archive.by_name(entry) else {
            continue;
        };
        if metadata.size() > MAX_METADATA {
            return Err("Mod metadata exceeds 256 KiB".into());
        }
        loaders.push(loader.into());
        if let Some(key) = name_key {
            let mut body = Vec::new();
            metadata
                .by_ref()
                .take(MAX_METADATA + 1)
                .read_to_end(&mut body)
                .map_err(|_| "Unreadable mod metadata")?;
            if body.len() as u64 > MAX_METADATA {
                return Err("Mod metadata exceeds 256 KiB".into());
            }
            let json: serde_json::Value =
                serde_json::from_slice(&body).map_err(|_| "Invalid JSON mod metadata")?;
            let value = key.split('.').try_fold(&json, |v, k| v.get(k));
            if let Some(value) = value
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                name = value
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(120)
                    .collect();
            }
        }
    }
    let compatible = resources::inspect(path);
    if let Ok(resource_mod) = &compatible {
        name = resource_mod.name.clone();
    }
    Ok(JavaMod {
        id: compatible.as_ref().ok().map(|m| m.id.clone()),
        file: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        name,
        loaders,
        native_compatible: compatible.is_ok(),
        diagnostic: compatible.map_or_else(
            |error| error,
            |m| {
                format!(
                    "Native resources/data: {} assets, {} data files; no Java executed.",
                    m.assets.len(),
                    m.data.len()
                )
            },
        ),
    })
}

/// Only immediate .jar files, including .JAR. No extraction, traversal or network access.
pub fn scan(directory: &Path) -> Report {
    let mut report = Report {
        schema: 1,
        project: NAME,
        version: VERSION,
        directory: directory.into(),
        mods: Vec::new(),
        errors: Vec::new(),
    };
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return report,
        Err(_) => {
            report.errors.push("Cannot read mods directory".into());
            return report;
        }
    };
    for entry in entries {
        let Ok(entry) = entry else {
            report.errors.push("Cannot read directory entry".into());
            continue;
        };
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|v| v.eq_ignore_ascii_case("jar"))
        {
            continue;
        }
        let file = entry.file_name().to_string_lossy().into_owned();
        let result = if entry.file_type().is_ok_and(|kind| kind.is_file()) {
            inspect(&path)
        } else {
            Err("Mod is not a regular file".into())
        };
        match result {
            Ok(item) => report.mods.push(item),
            Err(reason) => {
                report.errors.push(format!("{file}: {reason}"));
                report.mods.push(JavaMod {
                    id: None,
                    file: file.clone(),
                    name: file,
                    loaders: Vec::new(),
                    native_compatible: false,
                    diagnostic: reason,
                });
            }
        }
    }
    report.mods.sort_by(|a, b| a.file.cmp(&b.file));
    let mut counts = std::collections::BTreeMap::new();
    for item in &report.mods {
        if let Some(id) = &item.id {
            *counts.entry(id.clone()).or_insert(0usize) += 1;
        }
    }
    for item in &mut report.mods {
        if item.id.as_ref().is_some_and(|id| counts[id] > 1) {
            item.native_compatible = false;
            item.diagnostic =
                "Duplicate native mod identifier; remove one version before starting".into();
            report
                .errors
                .push(format!("{}: {}", item.file, item.diagnostic));
        }
    }
    report.errors.sort();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn jar(path: &Path, files: &[(&str, &[u8])]) {
        let mut out = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, body) in files {
            out.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            out.write_all(body).unwrap();
        }
        out.finish().unwrap();
    }
    #[test]
    fn detects_multiple_loader_metadata_without_claiming_native_execution() {
        let dir = tempfile::tempdir().unwrap();
        jar(
            &dir.path().join("priority.JAR"),
            &[
                ("fabric.mod.json", br#"{"name":"Sodium"}"#),
                ("META-INF/neoforge.mods.toml", b"modLoader=\"javafml\""),
            ],
        );
        jar(
            &dir.path().join("quilt.jar"),
            &[(
                "quilt.mod.json",
                br#"{"quilt_loader":{"metadata":{"name":"Example"}}}"#,
            )],
        );
        let report = scan(dir.path());
        assert!(report.errors.is_empty());
        assert_eq!(report.mods[0].loaders, ["Fabric", "NeoForge"]);
        assert_eq!(report.mods[0].name, "Sodium");
        assert_eq!(report.mods[1].name, "Example");
        assert!(report.mods.iter().all(|m| !m.native_compatible));
    }
    #[test]
    fn unreadable_and_oversized_mods_are_visible_and_files_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("broken.jar");
        fs::write(&bad, b"not a zip").unwrap();
        jar(
            &dir.path().join("large.jar"),
            &[("fabric.mod.json", &vec![b' '; MAX_METADATA as usize + 1])],
        );
        jar(&dir.path().join("unknown.jar"), &[]);
        let report = scan(dir.path());
        assert_eq!(report.mods.len(), 3);
        assert_eq!(report.errors.len(), 2);
        assert!(report.mods[2].loaders.is_empty());
        assert_eq!(fs::read(bad).unwrap(), b"not a zip");
        assert!(scan(&dir.path().join("absent")).mods.is_empty());
    }
}
