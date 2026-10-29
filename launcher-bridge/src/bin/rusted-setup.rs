#![cfg_attr(windows, windows_subsystem = "windows")]
use rusted_launch::{installer, setup, Result};
use std::path::PathBuf;
include!(concat!(env!("OUT_DIR"), "/installer_payload.rs"));

fn run() -> Result<Option<String>> {
    let executable = std::env::current_exe().map_err(|_| "Emplacement de l'outil inaccessible.")?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty()
        && setup::instance_config(&executable.with_file_name("rusted-launch.exe"))?.is_some()
    {
        // No console and no polling hidden from the user: ask them to close Prism first.
        setup::require_prism_closed()?;
        let result = setup::install(&executable.with_file_name("rusted-launch.exe"))?;
        installer::enable_auto(executable.parent().unwrap())?;
        return Ok(Some(format!("Instance configuree, mode automatique active.\n\nRouvrez Prism puis cliquez sur Jouer.\nLes anciens reglages ont ete sauvegardes.\n\n{}", result.config.display())));
    }
    let destination = match args.as_slice() {
        [flag, path] if flag == "--destination" => PathBuf::from(path),
        [] => match choose_folder()? {
            Some(p) => p,
            None => return Ok(None),
        },
        _ => return Err("Argument invalide pour l'installateur.".into()),
    };
    if INSTALL_PAYLOAD.is_empty() {
        return Err("Ce configurateur doit etre place dans minecraft/rusted. Pour une nouvelle instance, utiliser Installer-Rusted-Windows.exe.".into());
    }
    setup::require_prism_closed()?;
    let installed = installer::install(INSTALL_PAYLOAD, &destination)?;
    Ok(Some(format!("Rusted est installe et configure.\n\nRouvrez Prism puis lancez Rusted 0.3.4.\nAucune commande a saisir.\n\n{}", installed.display())))
}

#[cfg(windows)]
fn choose_folder() -> Result<Option<PathBuf>> {
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::*};
    // SAFETY: documented shell structs, owned PIDL freed using CoTaskMemFree.
    unsafe {
        let title: Vec<u16> =
            "Choisissez le dossier instances de Prism Launcher (fermez Prism avant l'installation)"
                .encode_utf16()
                .chain(Some(0))
                .collect();
        let info = BROWSEINFOW {
            lpszTitle: title.as_ptr(),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
            ..std::mem::zeroed()
        };
        let pidl = SHBrowseForFolderW(&info);
        if pidl.is_null() {
            return Ok(None);
        }
        let mut path = [0u16; 32768];
        let ok = SHGetPathFromIDListEx(pidl, path.as_mut_ptr(), path.len() as u32, 0);
        CoTaskMemFree(pidl.cast());
        if ok == 0 {
            return Err("Ce dossier n'est pas accessible sur le disque.".into());
        }
        use std::os::windows::ffi::OsStringExt;
        let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
            &path[..end],
        ))))
    }
}
#[cfg(not(windows))]
fn choose_folder() -> Result<Option<PathBuf>> {
    Err("Interface disponible sous Windows.".into())
}

fn main() {
    #[cfg(windows)]
    // SAFETY: initialize COM on this UI thread before the shell folder picker.
    unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(
            std::ptr::null(),
            windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32,
        );
    }
    let result = run();
    // CLI destination is an automation hook; it must never open a blocking dialog.
    let quiet = std::env::args().any(|a| a == "--destination");
    let failed = result.is_err();
    let message = match result {
        Ok(message) => message,
        Err(error) => Some(error),
    };
    if let Some(message) = message {
        if quiet {
            eprintln!("{message}");
        } else {
            show_message(&message, failed);
        }
    }
    if failed {
        std::process::exit(1)
    }
}
#[cfg(windows)]
fn show_message(message: &str, failed: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = "Rusted".encode_utf16().chain(Some(0)).collect();
    // SAFETY: terminated UTF-16 buffers live throughout this modal call.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK
                | if failed {
                    MB_ICONERROR
                } else {
                    MB_ICONINFORMATION
                },
        );
    }
}
#[cfg(not(windows))]
fn show_message(message: &str, _: bool) {
    eprintln!("{message}");
}
