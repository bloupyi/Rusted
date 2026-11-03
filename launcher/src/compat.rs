//! Install passive loader archives without rewriting their content or executing Java.
use crate::{
    model::{Engine, Instance, Store},
    Result, Task,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn import_resources(
    store: &Store,
    instance: &Instance,
    source: &Path,
    task: &Task,
) -> Result<PathBuf> {
    if instance.engine != Engine::Native || instance.minecraft != "26.3" {
        return Err("Cet import cible Rusted natif 26.3.".into());
    }
    task.check()?;
    let id = compatible_id(instance, source)?;
    let directory = store.game(instance)?.join("mods");
    let existing = rusted_launch::platform::scan(&directory);
    if existing
        .mods
        .iter()
        .any(|item| item.id.as_deref() == Some(&id))
    {
        return Err("Un module avec cet identifiant est deja installe.".into());
    }
    fs::create_dir_all(&directory).map_err(|_| "Dossier des mods inaccessible.")?;
    let destination = directory.join(format!("{}.jar", id));
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)
        .map_err(|_| "Preparation du module impossible.")?;
    std::io::copy(
        &mut fs::File::open(source).map_err(|_| "Mod inaccessible.")?,
        temporary.as_file_mut(),
    )
    .map_err(|_| "Copie du module impossible.")?;
    temporary
        .flush()
        .map_err(|_| "Ecriture du module impossible.")?;
    // Check the copied bytes too: the source could have changed during the copy.
    let copied = compatible_id(instance, temporary.path())?;
    if copied != id {
        return Err("Le module a change pendant son import.".into());
    }
    task.check()?;
    temporary
        .persist_noclobber(&destination)
        .map_err(|_| "Un module porte deja ce nom ou le dossier est inaccessible.")?;
    Ok(destination)
}

/// Experimental JVM eligibility is explicit and never changes the selected native engine.
pub fn compatible_id(instance: &Instance, path: &Path) -> Result<String> {
    match rusted_launch::platform::native_id(path) {
        Ok(id) => Ok(id),
        Err(error) if instance.experimental_java_mods => {
            rusted_launch::platform::java::inspect(path)
                .map(|m| m.id)
                .map_err(|java| format!("{error}; pont Java experimental : {java}"))
        }
        Err(error) => Err(error),
    }
}
