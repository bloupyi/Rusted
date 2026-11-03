//! Shared discovery for unchanged Fabric JAR entrypoints; execution lives in rusted-java.
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
#[derive(Debug)]
pub struct Entrypoint {
    pub value: String,
    pub adapter: String,
}
#[derive(Debug)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub entrypoints: BTreeMap<String, Vec<Entrypoint>>,
    pub adapters: BTreeMap<String, String>,
}

/// Eligibility is deliberately narrower than Fabric: no mixins, access wideners,
/// or nested JARs. Eligibility does not establish API compatibility.
pub fn inspect(path: &Path) -> Result<Module> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if zip.len() > 20000 {
        return Err("Too many Java mod entries".into());
    }
    if zip.file_names().any(|n| n == crate::native::MARKER) {
        return Err("Native companion takes precedence over Java initialization".into());
    }
    let mut f = zip
        .by_name("fabric.mod.json")
        .map_err(|_| "Fabric metadata is required")?;
    if f.size() > 256 << 10 {
        return Err("Fabric metadata too large".into());
    }
    let mut body = Vec::new();
    f.by_ref()
        .take((256 << 10) + 1)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    if body.len() > 256 << 10 {
        return Err("Fabric metadata too large".into());
    }
    drop(f);
    let metadata: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    if metadata["schemaVersion"] != 1 || metadata["environment"] == "server" {
        return Err("Unsupported Fabric schema or side".into());
    }
    if let Some(required) = metadata.pointer("/depends/minecraft") {
        if !crate::resources::metadata::version_matches(required) {
            return Err("Java mod targets a different Minecraft version".into());
        }
    }
    for key in ["mixins", "accessWidener", "jars"] {
        if metadata.get(key).is_some_and(|v| {
            !v.is_null() && v != &serde_json::json!([]) && v != &serde_json::json!({})
        }) {
            return Err(format!(
                "Fabric {key} is not implemented by the native Java bridge"
            ));
        }
    }
    let id = metadata["id"].as_str().ok_or("Missing Fabric id")?;
    let version = metadata["version"]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 256 && !v.chars().any(char::is_control))
        .ok_or("Invalid or missing Fabric version")?;
    if !(2..=64).contains(&id.len())
        || !id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_-".contains(&c))
    {
        return Err("Invalid Fabric id".into());
    }
    let mut adapters = BTreeMap::new();
    if let Some(values) = metadata.get("languageAdapters") {
        for (name, value) in values
            .as_object()
            .ok_or("Invalid Fabric language adapters")?
        {
            let class = value.as_str().ok_or("Invalid Fabric adapter class")?;
            if name == "default"
                || name.is_empty()
                || name.len() > 128
                || class.is_empty()
                || class.len() > 512
                || !class
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_$.".contains(&c))
            {
                return Err("Invalid Fabric language adapter".into());
            }
            adapters.insert(name.clone(), class.to_owned());
        }
    }
    let mut entrypoints = BTreeMap::new();
    if let Some(entries) = metadata.get("entrypoints") {
        for (kind, values) in entries.as_object().ok_or("Invalid Fabric entrypoints")? {
            if !matches!(kind.as_str(), "preLaunch" | "main" | "client") {
                continue;
            }
            let mut classes = Vec::new();
            for value in values.as_array().ok_or("Invalid Fabric entrypoint list")? {
                let adapter = match value.get("adapter") {
                    None => "default",
                    Some(adapter) => adapter
                        .as_str()
                        .ok_or("Invalid Fabric entrypoint adapter")?,
                };
                let class = if let Some(class) = value.as_str() {
                    class
                } else {
                    value["value"].as_str().ok_or("Invalid Fabric class")?
                };
                let (entry_class, member) = class
                    .split_once("::")
                    .map_or((class, None), |(class, member)| (class, Some(member)));
                if entry_class.is_empty()
                    || class.len() > 512
                    || !entry_class
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_$.".contains(&c))
                    || member.is_some_and(|member| {
                        member.is_empty()
                            || !member
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || b"_$".contains(&c))
                    })
                {
                    return Err("Invalid Fabric entrypoint class or member reference".into());
                }
                classes.push(Entrypoint {
                    value: class.to_owned(),
                    adapter: adapter.to_owned(),
                });
            }
            entrypoints.insert(kind.clone(), classes);
        }
    }
    if entrypoints.is_empty()
        && adapters.is_empty()
        && !zip.file_names().any(|n| n.ends_with(".class"))
    {
        return Err("No executable Java code or initializer".into());
    }
    Ok(Module {
        id: id.into(),
        name: metadata["name"].as_str().unwrap_or(id).to_owned(),
        version: version.to_owned(),
        path: path.canonicalize().map_err(|e| e.to_string())?,
        entrypoints,
        adapters,
    })
}

pub fn discover(directory: &Path) -> (Vec<Module>, Vec<String>) {
    let mut modules = Vec::new();
    let mut errors = Vec::new();
    let mut paths: Vec<_> = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")))
        .collect();
    paths.sort();
    for path in paths {
        if crate::native::inspect(&path).is_ok_and(|p| p.is_some())
            || crate::resources::inspect(&path).is_ok()
        {
            continue;
        }
        match inspect(&path) {
            Ok(module) => modules.push(module),
            Err(e) => errors.push(format!(
                "{}: {e}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
    let mut counts = BTreeMap::new();
    for m in &modules {
        *counts.entry(m.id.clone()).or_insert(0) += 1;
    }
    modules.retain(|m| {
        if counts[&m.id] > 1 {
            errors.push(format!("{}: duplicate Java mod id", m.id));
            false
        } else {
            true
        }
    });
    (modules, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn jar(path: &Path, mut metadata: serde_json::Value) {
        if metadata.get("version").is_none() {
            metadata["version"] = serde_json::json!("1.0.0");
        }
        let mut out = zip::ZipWriter::new(fs::File::create(path).unwrap());
        out.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(&serde_json::to_vec(&metadata).unwrap())
            .unwrap();
        out.start_file(
            "example/Client.class",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        out.write_all(b"metadata inspection does not execute this fixture")
            .unwrap();
        out.finish().unwrap();
    }
    #[test]
    fn unchanged_fabric_entrypoints_are_eligible_but_not_claimed_as_native_wasm() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("client.jar");
        jar(
            &file,
            serde_json::json!({"schemaVersion":1,"id":"example_client","entrypoints":{"client":["example.Client"],"server":["example.Server"]}}),
        );
        let module = inspect(&file).unwrap();
        assert_eq!(module.entrypoints["client"][0].value, "example.Client");
        assert_eq!(module.version, "1.0.0");
        assert!(!module.entrypoints.contains_key("server"));
        let report = crate::scan(dir.path());
        assert!(report.mods[0].java_candidate);
        assert!(!report.mods[0].native_compatible);
        assert!(crate::native_id(&file).is_err());
    }
    #[test]
    fn mixins_access_wideners_nested_jars_and_invalid_metadata_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("client.jar");
        for (field, value) in [
            ("mixins", serde_json::json!(["x.json"])),
            ("accessWidener", serde_json::json!("a.accesswidener")),
            ("jars", serde_json::json!([{"file":"nested.jar"}])),
        ] {
            let mut metadata = serde_json::json!({"schemaVersion":1,"id":"example_client","entrypoints":{"client":["example.Client"]}});
            metadata[field] = value;
            jar(&file, metadata);
            assert!(inspect(&file).unwrap_err().contains(field));
        }
        for entry in [
            serde_json::json!(null),
            serde_json::json!({"value":"a.Class","adapter":42}),
            serde_json::json!("a.Class::method::extra"),
            serde_json::json!("a.Class::"),
        ] {
            jar(
                &file,
                serde_json::json!({"schemaVersion":1,"id":"example_client","entrypoints":{"client":[entry]}}),
            );
            assert!(inspect(&file).is_err());
        }
    }
    #[test]
    fn disabled_and_duplicate_jars_are_not_executed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("client.jar");
        jar(
            &file,
            serde_json::json!({"schemaVersion":1,"id":"example_client","entrypoints":{"client":["example.Client"]}}),
        );
        fs::copy(&file, dir.path().join("duplicate.JAR")).unwrap();
        assert!(discover(dir.path()).0.is_empty());
        assert!(!discover(dir.path()).1.is_empty());
        fs::rename(
            dir.path().join("duplicate.JAR"),
            dir.path().join("duplicate.jar.disabled"),
        )
        .unwrap();
        assert_eq!(discover(dir.path()).0.len(), 1);
    }
    #[test]
    fn language_adapter_initialization_and_custom_entrypoints_are_retained() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("client.jar");
        jar(
            &file,
            serde_json::json!({"schemaVersion":1,"id":"example_client","languageAdapters":{"custom":"example.Adapter"},"entrypoints":{"preLaunch":[{"adapter":"custom","value":"example.Client"}]}}),
        );
        let module = inspect(&file).unwrap();
        assert_eq!(module.adapters["custom"], "example.Adapter");
        assert_eq!(module.entrypoints["preLaunch"][0].adapter, "custom");
        jar(
            &file,
            serde_json::json!({"schemaVersion":1,"id":"example_client","entrypoints":{"client":["example.Client::initialize","example.Client::INSTANCE"]}}),
        );
        let module = inspect(&file).unwrap();
        assert_eq!(
            module.entrypoints["client"][0].value,
            "example.Client::initialize"
        );
        assert_eq!(
            module.entrypoints["client"][1].value,
            "example.Client::INSTANCE"
        );
    }

    #[test]
    fn invalid_versions_are_rejected_before_configuring_the_shared_jvm() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("invalid.jar");
        for version in [
            serde_json::json!(""),
            serde_json::json!(null),
            serde_json::json!(42),
            serde_json::json!("line\nbreak"),
        ] {
            jar(
                &file,
                serde_json::json!({"schemaVersion":1,"id":"invalid_version","version":version,"entrypoints":{"client":["example.Client"]}}),
            );
            assert!(inspect(&file).unwrap_err().contains("version"));
        }
    }
}
