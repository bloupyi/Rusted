//! Keep Java loaders on their original JVM. This is coexistence, not native JAR execution.
use crate::Result;
use std::{fs, io::Read, path::Path};

pub fn java_loader(raw: &[String], game: &Path) -> Result<bool> {
    if !raw.iter().any(|s| s == "org.prismlauncher.EntryPoint") {
        return Ok(raw.iter().any(|s| {
            matches!(
                s.as_str(),
                "net.fabricmc.loader.impl.launch.knot.KnotClient"
                    | "org.quiltmc.loader.impl.launch.knot.KnotClient"
                    | "cpw.mods.modlauncher.Launcher"
                    | "cpw.mods.bootstraplauncher.BootstrapLauncher"
            )
        }));
    }
    let Some(parent) = game.parent() else {
        return Ok(false);
    };
    let path = parent.join("mmc-pack.json");
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("Profil Prism inaccessible ; aucun lancement effectue.".into()),
    };
    let mut bytes = Vec::new();
    (&mut file)
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Lecture du profil Prism impossible.")?;
    if bytes.len() > 1024 * 1024 {
        return Err("Profil Prism trop volumineux.".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Profil Prism invalide ; aucun lancement effectue.")?;
    let components = value["components"]
        .as_array()
        .ok_or("Composants Prism absents.")?;
    Ok(components.iter().any(|c| {
        c["disabled"] != true
            && matches!(
                c["uid"].as_str(),
                Some(
                    "net.fabricmc.fabric-loader"
                        | "org.quiltmc.quilt-loader"
                        | "net.minecraftforge"
                        | "net.neoforged"
                )
            )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_prism_components_without_consuming_launch_protocol() {
        let d = tempfile::tempdir().unwrap();
        let game = d.path().join("minecraft");
        fs::create_dir(&game).unwrap();
        let raw = vec!["org.prismlauncher.EntryPoint".into()];
        assert!(!java_loader(&raw, &game).unwrap());
        for uid in [
            "net.fabricmc.fabric-loader",
            "org.quiltmc.quilt-loader",
            "net.minecraftforge",
            "net.neoforged",
        ] {
            fs::write(
                d.path().join("mmc-pack.json"),
                serde_json::json!({"components":[{"uid":uid}]}).to_string(),
            )
            .unwrap();
            assert!(java_loader(&raw, &game).unwrap());
        }
        fs::write(d.path().join("mmc-pack.json"), "broken").unwrap();
        assert!(java_loader(&raw, &game).is_err());
        assert!(java_loader(
            &["net.fabricmc.loader.impl.launch.knot.KnotClient".into()],
            &game
        )
        .unwrap());
        assert!(!java_loader(&["net.minecraft.client.main.Main".into()], &game).unwrap());
    }
}
