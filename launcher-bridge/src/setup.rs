//! Configure only the Prism instance containing this runtime. No account data is read.
use crate::{wrapper_command, Result, MINECRAFT_VERSION};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn instance_config(executable: &Path) -> Result<Option<PathBuf>> {
    let runtime = executable
        .parent()
        .ok_or("Dossier du runtime introuvable.")?;
    if !runtime
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("rusted"))
    {
        return Ok(None);
    }
    let instance = runtime
        .parent()
        .and_then(Path::parent)
        .ok_or("Dossier de l'instance introuvable.")?;
    let config = instance.join("instance.cfg");
    let pack = instance.join("mmc-pack.json");
    if !config.is_file() || !pack.is_file() {
        return Ok(None);
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(pack).map_err(|_| "Profil Prism illisible.")?)
            .map_err(|_| "Profil Prism invalide.")?;
    if !metadata["components"].as_array().is_some_and(|a| {
        a.iter()
            .any(|c| c["uid"] == "net.minecraft" && c["version"] == MINECRAFT_VERSION)
    }) {
        return Err(
            "Cette configuration automatique cible une instance Prism Minecraft 26.3.".into(),
        );
    }
    Ok(Some(config))
}

fn encode_setting(value: &str, modern: bool) -> String {
    if modern {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        // Prism's original INI reader consumes backslash escapes and # comments.
        value.replace('\\', "\\\\").replace('#', "\\#")
    }
}

pub fn configured_ini(original: &str, command: &str) -> Result<String> {
    if command.contains(['\n', '\r']) {
        return Err("Commande Wrapper invalide.".into());
    }
    let bom = original.starts_with('\u{feff}');
    let original = original.strip_prefix('\u{feff}').unwrap_or(original);
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let modern = original.lines().any(|l| {
        l.trim()
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == "ConfigVersion")
    });
    let wrapper = format!("WrapperCommand={}", encode_setting(command, modern));
    let mut out = Vec::new();
    let (mut general, mut found_general, mut seen_wrapper, mut seen_override) =
        (true, false, false, false);
    let append_missing =
        |out: &mut Vec<String>, seen_wrapper: &mut bool, seen_override: &mut bool| {
            if !*seen_override {
                out.push("OverrideCommands=true".into());
                *seen_override = true;
            }
            if !*seen_wrapper {
                out.push(wrapper.clone());
                *seen_wrapper = true;
            }
        };
    for line in original.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if general && (found_general || seen_wrapper || seen_override) {
                append_missing(&mut out, &mut seen_wrapper, &mut seen_override);
            }
            general = trimmed == "[General]";
            found_general |= general;
            out.push(line.into());
            continue;
        }
        if general {
            if let Some((key, _)) = trimmed.split_once('=') {
                match key.trim() {
                    "WrapperCommand" => {
                        if !seen_wrapper {
                            out.push(wrapper.clone());
                            seen_wrapper = true;
                        }
                        continue;
                    }
                    "OverrideCommands" => {
                        if !seen_override {
                            out.push("OverrideCommands=true".into());
                            seen_override = true;
                        }
                        continue;
                    }
                    _ => {}
                }
            }
        }
        out.push(line.into());
    }
    if !seen_wrapper || !seen_override {
        if !general {
            out.push("[General]".into());
        }
        append_missing(&mut out, &mut seen_wrapper, &mut seen_override);
    }
    Ok(format!(
        "{}{}{}",
        if bom { "\u{feff}" } else { "" },
        out.join(newline),
        newline
    ))
}

pub struct Installed {
    pub config: PathBuf,
    pub backup: Option<PathBuf>,
}

/// The caller waits for Prism to close before this atomic update.
pub fn install(executable: &Path) -> Result<Installed> {
    let config = instance_config(executable)?
        .ok_or("Placer le runtime dans le dossier minecraft/rusted de l'instance Prism.")?;
    let original = fs::read(&config).map_err(|_| "Configuration Prism illisible.")?;
    if original.len() > 4 * 1024 * 1024 {
        return Err("Configuration Prism trop volumineuse.".into());
    }
    let text = std::str::from_utf8(&original).map_err(|_| {
        "Encodage de configuration Prism non pris en charge ; aucun fichier modifie."
    })?;
    let command = wrapper_command(executable)?;
    let updated = configured_ini(text, &command)?;
    if updated.as_bytes() == original {
        return Ok(Installed {
            config,
            backup: None,
        });
    }
    let folder = config.parent().unwrap();
    let mut backup = tempfile::Builder::new()
        .prefix("instance.cfg.rusted-backup-")
        .suffix(".ini")
        .tempfile_in(folder)
        .map_err(|_| "Impossible de sauvegarder la configuration Prism.")?;
    backup
        .write_all(&original)
        .and_then(|_| backup.as_file().sync_all())
        .map_err(|_| "Sauvegarde Prism incomplete ; configuration non modifiee.")?;
    let (_, backup) = backup
        .keep()
        .map_err(|_| "Impossible de conserver la sauvegarde Prism.")?;
    let mut replacement = tempfile::NamedTempFile::new_in(folder)
        .map_err(|_| "Dossier Prism non accessible en ecriture.")?;
    replacement
        .write_all(updated.as_bytes())
        .and_then(|_| replacement.as_file().sync_all())
        .map_err(|_| "Ecriture de la configuration impossible.")?;
    if fs::read(&config).map_err(|_| "Configuration Prism inaccessible.")? != original {
        return Err(
            "Prism ou un editeur a modifie la configuration ; fermez-le puis relancez l'outil."
                .into(),
        );
    }
    replacement
        .persist(&config)
        .map_err(|_| "Impossible de remplacer la configuration ; fermez Prism puis reessayez.")?;
    Ok(Installed {
        config,
        backup: Some(backup),
    })
}

#[cfg(windows)]
fn prism_running() -> Result<bool> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::*,
    };

    // SAFETY: initialized snapshot structure; all snapshot handles are closed.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("Impossible de verifier si Prism est ferme.".into());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        let mut found = false;
        while more {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_ascii_lowercase();
            if matches!(
                name.as_str(),
                "prismlauncher.exe" | "prismlauncherconsole.exe" | "prism-launcher.exe"
            ) {
                found = true;
                break;
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        Ok(found)
    }
}

#[cfg(windows)]
pub fn require_prism_closed() -> Result<()> {
    if prism_running()? {
        return Err("Fermez Prism Launcher puis relancez cet outil. Aucun fichier modifie.".into());
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn require_prism_closed() -> Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn wait_for_prism() -> Result<()> {
    if !prism_running()? {
        return Ok(());
    }
    eprintln!("Rusted : fermez Prism Launcher. La configuration sera appliquee automatiquement apres sa fermeture (attente maximum : 10 minutes).");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    while prism_running()? {
        if std::time::Instant::now() >= deadline {
            return Err(
                "Prism est toujours ouvert ; fermez-le puis relancez Configurer-Rusted.cmd.".into(),
            );
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn wait_for_prism() -> Result<()> {
    Err("Configuration automatique disponible sous Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_keeps_other_preferences_and_is_idempotent() {
        let ini = "\u{feff}[General]\r\nConfigVersion=1.3\r\nname=My instance\r\nOverrideCommands=false\r\nWrapperCommand=old\r\nJavaPath=C:/Java/javaw.exe\r\n[Other]\r\nWrapperCommand=leave-this-alone\r\n";
        let command = "\"C:/Users/Jean Dupont/O'Neil #1/Rusted/rusted-launch.exe\" --";
        let result = configured_ini(ini, command).unwrap();
        assert!(result.contains("name=My instance\r\n"));
        assert!(result.contains("JavaPath=C:/Java/javaw.exe\r\n"));
        assert!(result.contains("[Other]\r\nWrapperCommand=leave-this-alone\r\n"));
        assert!(result.contains(
            "WrapperCommand=\"\\\"C:/Users/Jean Dupont/O'Neil #1/Rusted/rusted-launch.exe\\\" --\""
        ));
        assert!(result.starts_with('\u{feff}'));
        assert_eq!(configured_ini(&result, command).unwrap(), result);
        let legacy = configured_ini("[General]\nname=Legacy\n", command).unwrap();
        assert!(legacy.contains("O'Neil \\#1"));
    }
    #[test]
    fn install_only_updates_containing_instance_and_backs_up_exact_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let instance = temp.path().join("Prism instance with spaces");
        let runtime = instance.join("minecraft/rusted");
        fs::create_dir_all(&runtime).unwrap();
        let executable = runtime.join("rusted-launch.exe");
        let config = instance.join("instance.cfg");
        let original = b"[General]\nConfigVersion=1.3\nname=Keep me\n";
        fs::write(&config, original).unwrap();
        fs::write(
            instance.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"26.3"}]}"#,
        )
        .unwrap();
        let result = install(&executable).unwrap();
        assert_eq!(result.config, config);
        assert_eq!(fs::read(result.backup.unwrap()).unwrap(), original);
        assert!(fs::read_to_string(&config)
            .unwrap()
            .contains("OverrideCommands=true"));
        assert!(install(&executable).unwrap().backup.is_none());
        assert!(instance_config(&temp.path().join("unrelated.exe"))
            .unwrap()
            .is_none());
    }
}
