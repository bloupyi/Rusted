use crate::{
    auth::Session,
    install::{self, Ready},
    model::{self, Engine, Instance, Store},
    Event, Result, Task,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub fn prepare(store: &Store, instance: &Instance, runtime: &Path, task: &Task) -> Result<Ready> {
    install::prepare(store, instance, runtime, task)
}
pub fn play(
    store: &Store,
    instance: &Instance,
    runtime: &Path,
    session: &Session,
    task: &Task,
) -> Result<()> {
    if instance.engine != Engine::Fabric {
        let report = rusted_launch::platform::scan(&store.game(instance)?.join("mods"));
        if !report.errors.is_empty() {
            return Err(report.errors.join("; "));
        }
        let rejected: Vec<_> = report
            .mods
            .iter()
            .filter(|item| instance.engine != Engine::Native || !item.native_compatible)
            .map(|item| format!("{}: {}", item.name, item.diagnostic))
            .collect();
        if !rejected.is_empty() {
            return Err(format!(
                "Mods incompatibles avec ce moteur : {}. Choisir Java + Fabric pour les mods Java.",
                rejected.join("; ")
            ));
        }
    }
    let _instance_lock = profile_lock(store, instance)?;
    let ready = prepare(store, instance, runtime, task)?;
    task.check()?;
    let game = store.game(instance)?;
    let home = game.join("rusted/native-data");
    let mut secret = tempfile::Builder::new()
        .prefix("rusted-session-")
        .tempfile_in(&store.root)
        .map_err(|_| "Creation de la session privee impossible.")?;
    let mut command;
    if instance.engine == Engine::Native {
        if !home.exists() {
            model::copy_tree(&runtime.join("native-data"), &home)?;
            fs::create_dir_all(&home).map_err(|_| "Donnees natives inaccessibles.")?;
        }
        let value = json!({"name":session.account.name,"uuid":session.account.id,"access_token":session.access_token,"xuid":session.xuid,"user_type":"msa"});
        serde_json::to_writer(secret.as_file_mut(), &value)
            .map_err(|_| "Session privee invalide.")?;
        secret.flush().map_err(|_| "Session privee inaccessible.")?;
        command = Command::new(runtime.join("runtime/rusted.exe"));
        command
            .current_dir(&home)
            .env("RUSTED_HOME", &home)
            .env("RUSTED_INSTANCE_DIR", &game)
            .env("HORIZON_HOME", &home)
            .env("HORIZON_SESSION_FILE", secret.path())
            .env("HORIZON_LAUNCHER_HOME", home.join("launcher"))
            .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
            .env("XDG_DATA_HOME", home.join("xdg-data"))
            .env("XDG_CONFIG_HOME", home.join("xdg-config"))
            .env_remove("INST_JAVA_ARGS")
            .arg("--resources")
            .arg(
                store
                    .root
                    .join("cache/native/preparation/.local/horizon-client/26.3"),
            )
            .args([
                "--render-distance",
                &instance.render_distance.to_string(),
                "--width",
                &instance.width.to_string(),
                "--height",
                &instance.height.to_string(),
            ]);
        // Use the native options file, preserving every unrelated setting.
        update_options(
            &home.join("options.txt"),
            &[(
                "fullscreen",
                if instance.fullscreen { "true" } else { "false" },
            )],
        )?;
        update_options(
            &home.join("horizon-graphics.txt"),
            &[("graphicsApi", instance.graphics_api.as_str())],
        )?;
    } else {
        let mut cp = ready.classpath.clone();
        cp.insert(0, runtime.join("launcher-entry.jar"));
        let proxy = classpath_jar(&store.path(&instance.id)?.join("classpath.jar"), &cp)?;
        let cp = proxy.to_string_lossy().into_owned();
        let values = std::collections::BTreeMap::from([
            (
                "natives_directory",
                ready.natives.to_string_lossy().into_owned(),
            ),
            ("launcher_name", "Rusted Launcher".into()),
            ("launcher_version", model::VERSION.into()),
            ("classpath", cp.clone()),
            ("auth_player_name", session.account.name.clone()),
            ("version_name", instance.minecraft.clone()),
            ("game_directory", game.to_string_lossy().into_owned()),
            ("assets_root", ready.assets.to_string_lossy().into_owned()),
            (
                "assets_index_name",
                ready.metadata["assetIndex"]["id"]
                    .as_str()
                    .unwrap_or("")
                    .into(),
            ),
            ("auth_uuid", session.account.id.clone()),
            ("auth_access_token", session.access_token.clone()),
            ("clientid", session.client_id.clone()),
            ("auth_xuid", session.xuid.clone().unwrap_or_default()),
            ("version_type", "release".into()),
            ("user_type", "msa".into()),
            ("user_properties", "{}".into()),
            (
                "auth_session",
                format!("token:{}:{}", session.access_token, session.account.id),
            ),
        ]);
        let mut jvm = expand(&ready.metadata["arguments"]["jvm"], &values)?;
        // Replace metadata's classpath/main handover: the own entrypoint reads private game arguments.
        let mut filtered = vec![];
        let mut skip = false;
        for a in jvm.drain(..) {
            if skip {
                skip = false;
                continue;
            }
            if matches!(a.as_str(), "-cp" | "-classpath" | "--class-path") {
                skip = true;
                continue;
            }
            filtered.push(a)
        }
        filtered.extend(instance.java_args.clone());
        if let Some(f) = &ready.fabric {
            filtered.extend(expand(&f["arguments"]["jvm"], &values)?);
        }
        let mut args = expand(&ready.metadata["arguments"]["game"], &values)?;
        args.extend([
            "--width".into(),
            instance.width.to_string(),
            "--height".into(),
            instance.height.to_string(),
        ]);
        if instance.fullscreen {
            args.push("--fullscreen".into())
        }
        let main = if instance.engine == Engine::Fabric {
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        } else {
            "net.minecraft.client.main.Main"
        };
        write_string(secret.as_file_mut(), main)?;
        secret
            .write_all(&(args.len() as u32).to_be_bytes())
            .map_err(|_| "Session Java inaccessible.")?;
        for arg in &args {
            write_string(secret.as_file_mut(), arg)?
        }
        secret.flush().map_err(|_| "Session Java inaccessible.")?;
        command = Command::new(&ready.java);
        command
            .current_dir(&game)
            .arg("-Xms512M")
            .arg(format!("-Xmx{}M", instance.memory_mb))
            .args(filtered)
            .arg("-cp")
            .arg(cp)
            .arg("dev.rusted.launcher.Entrypoint")
            .arg(secret.path())
            .env_remove("INST_JAVA_ARGS");
    }
    task.send(Event::Status("Jeu en cours".into()));
    task.send(Event::GameStarted);
    let began = Instant::now();
    let code = run(
        command,
        task,
        &[&session.access_token, &session.refresh_token],
    )?;
    drop(secret);
    let mut latest = instance.clone();
    latest.last_played = model::now();
    latest.play_seconds += began.elapsed().as_secs();
    store.save(&latest)?;
    task.send(Event::GameStopped(code));
    if code != 0 && !task.cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(format!(
            "Le jeu s'est arrete avec le code {code}. Consulter la console."
        ));
    }
    Ok(())
}
fn update_options(path: &Path, values: &[(&str, &str)]) -> Result<()> {
    let old = fs::read_to_string(path).unwrap_or_default();
    let mut out = old
        .lines()
        .filter(|l| !values.iter().any(|(k, _)| l.starts_with(&format!("{k}:"))))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    out.extend(values.iter().map(|(k, v)| format!("{k}:{v}")));
    fs::write(path, out.join("\n") + "\n").map_err(|_| "Reglages natifs inaccessibles.".into())
}
fn write_string(out: &mut fs::File, v: &str) -> Result<()> {
    if v.len() > 131072 {
        return Err("Argument Java trop long.".into());
    }
    out.write_all(&(v.len() as u32).to_be_bytes())
        .and_then(|_| out.write_all(v.as_bytes()))
        .map_err(|_| "Session Java inaccessible.".into())
}
fn expand(v: &Value, values: &std::collections::BTreeMap<&str, String>) -> Result<Vec<String>> {
    let mut out = vec![];
    for item in v.as_array().into_iter().flatten() {
        let value = if item.is_string() {
            item
        } else {
            if !install::allowed(&item["rules"]) {
                continue;
            }
            &item["value"]
        };
        let parts = if let Some(s) = value.as_str() {
            vec![s]
        } else {
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect()
        };
        for part in parts {
            let mut s = part.to_string();
            for (k, v) in values {
                s = s.replace(&format!("${{{k}}}"), v)
            }
            if s.contains("${") {
                return Err(
                    "Option du lanceur non prise en charge par cette version Minecraft.".into(),
                );
            }
            out.push(s)
        }
    }
    Ok(out)
}
pub fn run(mut command: Command, task: &Task, secrets: &[&str]) -> Result<i32> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "Impossible de demarrer le programme. Verifier Java et le runtime Rusted.")?;
    let _job = job(&mut child)?;
    let mut threads = vec![];
    let outputs: Vec<Box<dyn std::io::Read + Send>> = vec![
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    for output in outputs {
        let task = task.clone();
        let secrets = secrets
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        threads.push(std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(mut line) = line else { break };
                for secret in &secrets {
                    line = line.replace(secret, "<session>")
                }
                if line.len() > 4096 {
                    line = line.chars().take(4096).collect()
                }
                task.send(Event::Log(line));
            }
        }));
    }
    let result = loop {
        if task.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = child.kill();
            break child.wait().map_err(|_| "Arret du processus impossible.")?;
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|_| "Etat du processus inaccessible.")?
        {
            break status;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    #[cfg(windows)]
    drop(_job);
    for t in threads {
        let _ = t.join();
    }
    Ok(result.code().unwrap_or(1))
}
#[cfg(windows)]
struct Job(usize);
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
        }
    }
}
#[cfg(windows)]
fn job(child: &mut Child) -> Result<Job> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
    unsafe {
        let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if handle.is_null() {
            let _ = child.kill();
            return Err("Groupe de processus Windows indisponible.".into());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            (&info as *const _) as _,
            std::mem::size_of_val(&info) as u32,
        ) == 0
            || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
        {
            CloseHandle(handle);
            let _ = child.kill();
            return Err("Protection de l'arbre de processus Windows indisponible.".into());
        }
        Ok(Job(handle as usize))
    }
}
#[cfg(not(windows))]
struct Job;
#[cfg(not(windows))]
fn job(_: &mut Child) -> Result<Job> {
    Ok(Job)
}

/// A manifest classpath keeps Windows CreateProcess arguments below 32767 UTF-16 units.
pub fn classpath_jar(path: &Path, files: &[std::path::PathBuf]) -> Result<std::path::PathBuf> {
    let mut value = String::from("Class-Path: ");
    for (i, file) in files.iter().enumerate() {
        if i > 0 {
            value.push(' ')
        }
        let absolute = std::path::absolute(file).map_err(|_| "Classpath invalide.")?;
        value.push_str(
            url::Url::from_file_path(absolute)
                .map_err(|_| "Classpath invalide.")?
                .as_str(),
        );
    }
    let mut manifest = String::from("Manifest-Version: 1.0\r\n");
    for (i, chunk) in value.as_bytes().chunks(70).enumerate() {
        if i > 0 {
            manifest.push(' ')
        }
        manifest.push_str(std::str::from_utf8(chunk).map_err(|_| "Classpath non ASCII invalide.")?);
        manifest.push_str("\r\n");
    }
    manifest.push_str("\r\n");
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("Dossier de classpath absent.")?)
            .map_err(|_| "Classpath inaccessible.")?;
    {
        let mut zip = zip::ZipWriter::new(temporary.as_file_mut());
        zip.start_file(
            "META-INF/MANIFEST.MF",
            zip::write::SimpleFileOptions::default(),
        )
        .map_err(|_| "Classpath inaccessible.")?;
        zip.write_all(manifest.as_bytes())
            .map_err(|_| "Classpath inaccessible.")?;
        zip.finish().map_err(|_| "Classpath inaccessible.")?;
    }
    temporary
        .persist(path)
        .map_err(|_| "Classpath inaccessible.")?;
    Ok(path.to_owned())
}

pub fn profile_lock(store: &Store, instance: &Instance) -> Result<fs::File> {
    use fs2::FileExt;
    let directory = store.path(&instance.id)?;
    let locks = store.root.join("locks");
    fs::create_dir_all(&locks).map_err(|_| "Dossier des verrous inaccessible.")?;
    // Windows refuses to rename a directory containing a locked file, even when no game runs.
    // Keep the operation lock outside the directory while retaining exclusivity during rename.
    let f = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(locks.join(format!("{}.lock", instance.id)))
        .map_err(|_| "Verrou d'instance inaccessible.")?;
    f.try_lock_exclusive()
        .map_err(|_| "Cette instance est deja utilisee par une partie ou une autre operation.")?;
    if !directory.is_dir() {
        return Err("Cette instance n'existe plus.".into());
    }
    Ok(f)
}
