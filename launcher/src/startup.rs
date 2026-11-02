//! Startup diagnostics without account, session or command-line contents.
use crate::Result;
use std::{
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
};
static JOURNAL: OnceLock<PathBuf> = OnceLock::new();

pub fn data_root(arguments: impl IntoIterator<Item = OsString>) -> Result<PathBuf> {
    let mut arguments = arguments.into_iter();
    let mut root = None;
    while let Some(arg) = arguments.next() {
        if arg != "--data-dir" {
            return Err(
                "Argument inconnu. Utiliser --data-dir DOSSIER pour un profil portable.".into(),
            );
        }
        root = Some(PathBuf::from(
            arguments
                .next()
                .filter(|a| !a.is_empty())
                .ok_or("Le parametre --data-dir attend un dossier.")?,
        ));
    }
    Ok(root
        .or_else(|| std::env::var_os("RUSTED_LAUNCHER_HOME").map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("APPDATA").unwrap_or_else(|| OsString::from(".")))
                .join("RustedLauncher")
        }))
}
pub fn initialize(root: &Path) {
    let path = root.join("logs/startup.log");
    if fs::create_dir_all(path.parent().unwrap()).is_err() {
        return;
    }
    // Keep the previous attempt without growing the journal indefinitely.
    if path.is_file() {
        let _ = fs::copy(&path, path.with_extension("previous.log"));
    }
    if fs::write(
        &path,
        format!("Rusted Launcher {}\n", crate::model::VERSION),
    )
    .is_ok()
    {
        let _ = JOURNAL.set(path);
        record("Demarrage du processus.");
    }
}
pub fn record(message: &str) {
    if let Some(path) = JOURNAL.get() {
        if let Ok(mut file) = fs::OpenOptions::new().append(true).open(path) {
            let _ = writeln!(file, "{message}");
        }
    }
}
pub fn journal_path() -> Option<&'static Path> {
    JOURNAL.get().map(PathBuf::as_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_profile_keeps_unicode_and_spaces_and_rejects_missing_value() {
        assert_eq!(
            data_root(["--data-dir".into(), "profil cuivre é".into()]).unwrap(),
            PathBuf::from("profil cuivre é")
        );
        assert!(data_root(["--data-dir".into()]).is_err());
        assert!(data_root(["--data-dir".into(), "".into()]).is_err());
        assert!(data_root(["--unsupported".into()]).is_err());
    }
}
