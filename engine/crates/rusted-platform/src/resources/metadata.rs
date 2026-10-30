use super::read_entry;
use serde_json::{json, Value};
use std::fs;

fn version_matches(requirement: &Value) -> bool {
    if let Some(array) = requirement.as_array() {
        return array.iter().any(version_matches);
    }
    let Some(text) = requirement.as_str() else {
        return false;
    };
    text.split("||").any(|part| {
        let requirement = part
            .replace(',', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(",");
        semver::VersionReq::parse(&requirement)
            .is_ok_and(|req| req.matches(&semver::Version::new(26, 3, 0)))
    })
}
fn json_metadata(v: &Value, quilt: bool) -> Result<(String, String, String), String> {
    let m = if quilt { &v["quilt_loader"] } else { v };
    if !quilt && v["schemaVersion"] != 1 {
        return Err("Unsupported Fabric metadata schema".into());
    }
    for key in [
        "entrypoints",
        "mixins",
        "mixin",
        "jars",
        "accessWidener",
        "access_widener",
        "languageAdapters",
        "plugins",
    ] {
        for object in [v, m] {
            if object
                .get(key)
                .is_some_and(|v| !v.is_null() && v != &json!({}) && v != &json!([]))
            {
                return Err(format!(
                    "Java loader hook {key} requires a native port or its original JVM"
                ));
            }
        }
    }
    let allowed = |id: &str| {
        matches!(
            id,
            "minecraft"
                | "java"
                | "fabricloader"
                | "fabric-resource-loader-v0"
                | "quilt_loader"
                | "quilt_resource_loader"
        )
    };
    if quilt {
        if m.get("depends").is_some_and(|v| !v.is_array()) {
            return Err("Invalid Quilt dependencies".into());
        }
        for dependency in m["depends"].as_array().into_iter().flatten() {
            let id = dependency["id"]
                .as_str()
                .ok_or("Unsupported Quilt dependency")?;
            if !allowed(id) || (id == "minecraft" && !version_matches(&dependency["versions"])) {
                return Err(format!("Unsupported native dependency: {id}"));
            }
        }
    } else if let Some(dependencies) = m["depends"].as_object() {
        for (id, versions) in dependencies {
            if !allowed(id) || (id == "minecraft" && !version_matches(versions)) {
                return Err(format!("Unsupported native dependency/version: {id}"));
            }
        }
    } else if m.get("depends").is_some() {
        return Err("Invalid Fabric dependencies".into());
    }
    let id = m["id"].as_str().ok_or("Missing mod id")?.to_owned();
    if !(2..=64).contains(&id.len())
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
    {
        return Err("Invalid mod identifier".into());
    }
    let metadata = if quilt { &m["metadata"] } else { m };
    let name = metadata["name"]
        .as_str()
        .unwrap_or(&id)
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect();
    let icon = metadata["icon"].as_str().unwrap_or("").to_owned();
    Ok((id, name, icon))
}

pub(super) fn read(
    archive: &mut zip::ZipArchive<fs::File>,
    files: &[String],
    limit: u64,
) -> Result<(String, String, String), String> {
    for (path, quilt) in [("fabric.mod.json", false), ("quilt.mod.json", true)] {
        if files.iter().any(|name| name == path) {
            let bytes = read_entry(archive, path, limit)?;
            let metadata: Value =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid loader metadata")?;
            if metadata["environment"] == "client"
                && files.iter().any(|name| name.starts_with("data/"))
            {
                return Err("Client-only metadata cannot enable server data".into());
            }
            return json_metadata(&metadata, quilt);
        }
    }
    for path in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if !files.iter().any(|name| name == path) {
            continue;
        }
        let bytes = read_entry(archive, path, limit)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "Invalid loader metadata encoding")?;
        let metadata: toml::Value =
            toml::from_str(text).map_err(|_| "Invalid Forge/NeoForge metadata")?;
        if metadata.get("modLoader").and_then(toml::Value::as_str) != Some("lowcodefml") {
            return Err("Forge/NeoForge Java loader requires a native port; only lowcodefml data modules are supported".into());
        }
        let mods = metadata
            .get("mods")
            .and_then(toml::Value::as_array)
            .ok_or("Missing lowcode mods")?;
        if mods.len() != 1 {
            return Err("One lowcode module per archive is supported".into());
        }
        let id = mods[0]
            .get("modId")
            .and_then(toml::Value::as_str)
            .ok_or("Missing lowcode mod id")?;
        // Lowcode metadata alone does not establish a compatible dependency set.
        if metadata.get("dependencies").is_some() {
            return Err("Lowcode dependencies require an explicit native adapter".into());
        }
        return json_metadata(
            &json!({"schemaVersion":1,"id":id,"name":mods[0].get("displayName").and_then(toml::Value::as_str).unwrap_or(id)}),
            false,
        );
    }
    Err("No supported Fabric, Quilt, Forge or NeoForge metadata".into())
}
