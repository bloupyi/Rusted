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
    let module = rusted_launch::platform::resources::inspect(source)?;
    let directory = store.game(instance)?.join("mods");
    let existing = rusted_launch::platform::scan(&directory);
    if existing
        .mods
        .iter()
        .any(|item| item.id.as_deref() == Some(&module.id))
    {
        return Err("Un module avec cet identifiant est deja installe.".into());
    }
    fs::create_dir_all(&directory).map_err(|_| "Dossier des mods inaccessible.")?;
    let destination = directory.join(format!("{}.jar", module.id));
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
    let copied = rusted_launch::platform::resources::inspect(temporary.path())?;
    if copied.id != module.id {
        return Err("Le module a change pendant son import.".into());
    }
    task.check()?;
    temporary
        .persist_noclobber(&destination)
        .map_err(|_| "Un module porte deja ce nom ou le dossier est inaccessible.")?;
    Ok(destination)
}
