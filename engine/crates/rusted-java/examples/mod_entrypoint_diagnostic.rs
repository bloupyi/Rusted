//! Developer-only component test of real JAR entrypoints. This does NOT apply mixins,
//! start a world or certify compatibility. Player startup keeps its eligibility guards.
use rusted_platform::java::{Entrypoint, Module};
use std::{collections::BTreeMap, io::Read, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let java = args.next().ok_or("expected JAVA_HOME")?;
    let home = args.next().ok_or("expected disposable game directory")?;
    let manifest = args.next().ok_or("expected library manifest JSON")?;
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    let libraries: Vec<PathBuf> =
        serde_json::from_slice(&std::fs::read(manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut modules = Vec::new();
    let mut scheduled = 0;
    for path in args {
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let mut body = String::new();
        archive
            .by_name("fabric.mod.json")
            .map_err(|e| e.to_string())?
            .take(262145)
            .read_to_string(&mut body)
            .map_err(|e| e.to_string())?;
        let metadata: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        let mut entrypoints = BTreeMap::new();
        for kind in ["preLaunch", "main", "client"] {
            let mut entries = Vec::new();
            for entry in metadata["entrypoints"][kind]
                .as_array()
                .into_iter()
                .flatten()
            {
                let value = entry
                    .as_str()
                    .or_else(|| entry["value"].as_str())
                    .ok_or("Invalid entrypoint metadata")?;
                entries.push(Entrypoint {
                    value: value.into(),
                    adapter: entry["adapter"].as_str().unwrap_or("default").into(),
                });
                scheduled += 1;
            }
            entrypoints.insert(kind.into(), entries);
        }
        modules.push(Module {
            id: metadata["id"].as_str().ok_or("missing id")?.into(),
            name: metadata["name"].as_str().unwrap_or("diagnostic").into(),
            version: metadata["version"]
                .as_str()
                .ok_or("missing version")?
                .into(),
            path: path.canonicalize().map_err(|e| e.to_string())?,
            entrypoints,
            adapters: BTreeMap::new(),
        });
    }
    eprintln!(
        "COMPONENT DIAGNOSTIC ONLY: no mixin execution, renderer or compatibility certification"
    );
    if scheduled == 0 {
        return Err("No standard entrypoints; mixin-driven startup was NOT exercised".into());
    }
    let mut runtime =
        rusted_java::Runtime::load(&java, &home, &home.join("cache"), &modules, &libraries)?;
    let mut failed = false;
    for action in runtime.tick(0, 0.0, false)? {
        failed |= matches!(action, rusted_java::Action::Error(_));
        println!("{action:?}");
    }
    if failed {
        return Err("Real mod entrypoint failed; compatibility is NOT established".into());
    }
    Ok(())
}
