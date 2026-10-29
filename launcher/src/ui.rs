//! Original immediate navigation over native Win32 controls and an owner-drawn copper/pixel shell.
//! The window owns App; worker threads receive only cloned model data and a message sender.
use rusted_launcher::{
    auth, catalog, compat, launch,
    model::{Engine, Instance, Settings, Store},
    mods, packs, Event, Result, Task,
};
use std::{
    cell::Cell,
    collections::VecDeque,
    ffi::OsString,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::{Com::*, LibraryLoader::GetModuleHandleW, Memory::*},
    UI::{Controls::Dialogs::*, Controls::*, HiDpi::*, Shell::*, WindowsAndMessaging::*},
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
const BG: u32 = 0x181B1D;
const PANEL: u32 = 0x202529;
const TEXT: u32 = 0xD8E4EC;
struct Control {
    window: HWND,
    rect: [i32; 4],
    footer: bool,
}
struct App {
    store: Store,
    runtime: PathBuf,
    settings: Settings,
    instances: Vec<Instance>,
    accounts: Vec<auth::Account>,
    selected: usize,
    page: u32,
    controls: Vec<Control>,
    font: HFONT,
    bold: HFONT,
    small: HFONT,
    mono: HFONT,
    brush: HBRUSH,
    rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
    job: Option<Arc<AtomicBool>>,
    running: bool,
    status: String,
    progress: (u64, u64),
    logs: VecDeque<String>,
    hits: Vec<mods::Hit>,
    device: Option<(String, String)>,
    window: HWND,
    log_dirty: bool,
    auto_fired: bool,
    version_catalog: catalog::Catalog,
    catalog_busy: bool,
    catalog_started: bool,
    catalog_notice: String,
    draft_name: String,
    draft_engine: Engine,
    draft_version: String,
    rendered_status: String,
    rendered_progress: (u64, u64),
    auth_job: Option<Arc<AtomicBool>>,
    account_config_open: bool,
    account_notice: String,
    mod_tab: u32,
    mod_query: String,
    mod_searching: bool,
    search_generation: u64,
    pending_play: Option<String>,
}
impl App {
    fn selected(&self) -> Option<Instance> {
        self.instances.get(self.selected).cloned()
    }
    fn refresh(&mut self) {
        if let Ok(v) = self.store.list() {
            self.instances = v;
            self.selected = self
                .instances
                .iter()
                .position(|i| i.id == self.settings.selected)
                .unwrap_or(0);
        }
        if let Ok(a) = auth::list(&self.store) {
            self.accounts = a;
        }
    }
    fn message(&self, s: &str) {
        unsafe {
            MessageBoxW(
                self.window,
                wide(s).as_ptr(),
                wide("Rusted Launcher").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
    fn fail(&mut self, s: String) {
        self.status = s.clone();
        self.logs.push_back(s.clone());
        self.message(&s);
    }
    fn start<F>(&mut self, status: &str, f: F)
    where
        F: FnOnce(Task) -> Result<String> + Send + 'static,
    {
        if self.job.is_some() {
            self.message("Une operation est deja en cours.");
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.job = Some(cancel.clone());
        self.status = status.into();
        self.progress = (0, 0);
        let task = Task {
            events: self.tx.clone(),
            cancel,
        };
        std::thread::spawn(move || {
            let result = f(task.clone());
            task.send(match result {
                Ok(s) => Event::Done(s),
                Err(e) => Event::Failed(e),
            });
        });
    }
    fn reload_selection(&mut self, idx: usize) {
        self.pending_play = None;
        self.search_generation += 1;
        self.mod_searching = false;
        self.hits.clear();
        self.selected = idx.min(self.instances.len().saturating_sub(1));
        if let Some(i) = self.selected() {
            self.settings.selected = i.id;
            let _ = self.store.save_settings(&self.settings);
        }
        unsafe {
            build(self);
        }
    }
    fn selected_or_error(&self) -> Result<Instance> {
        self.selected()
            .ok_or("Creer une instance pour commencer.".into())
    }
}
pub fn error_box(s: &str) {
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(s).as_ptr(),
            wide("Rusted Launcher").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
pub fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut home = None;
    while let Some(a) = args.next() {
        if a == "--data-dir" {
            home = args.next().map(PathBuf::from)
        } else {
            return Err(
                "Argument inconnu. Utiliser --data-dir DOSSIER pour un profil portable.".into(),
            );
        }
    }
    let root = home
        .or_else(|| std::env::var_os("RUSTED_LAUNCHER_HOME").map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("APPDATA").unwrap_or_else(|| OsString::from(".")))
                .join("RustedLauncher")
        });
    let store = Store::open(root)?;
    use fs2::FileExt;
    let _single_instance = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.root.join("launcher.lock"))
        .map_err(|_| "Verrou du lanceur inaccessible.")?;
    _single_instance
        .try_lock_exclusive()
        .map_err(|_| "Rusted Launcher est deja ouvert pour ce dossier de donnees.")?;
    let mut settings = store.settings()?;
    if settings.microsoft_client_id.is_empty() {
        if let Some(root) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(PathBuf::from))
        {
            let config = root.join("launcher-config.json");
            if config.metadata().is_ok_and(|m| m.len() <= 65536) {
                if let Ok(value) = rusted_launcher::model::read_json::<serde_json::Value>(&config) {
                    settings.microsoft_client_id = value["microsoft_client_id"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .to_owned();
                }
            }
        }
    }
    if settings.download_threads <= 4 {
        settings.download_threads = 24;
    }
    let mut instances = store.list()?;
    if instances.is_empty() {
        instances.push(store.create("Rusted 26.3", Engine::Native, "26.3")?);
    }
    let selected = instances
        .iter()
        .position(|v| v.id == settings.selected)
        .unwrap_or(0);
    settings.selected = instances[selected].id.clone();
    store.save_settings(&settings)?;
    let accounts = auth::list(&store)?;
    let (tx, rx) = mpsc::channel();
    let runtime = std::env::current_exe()
        .map_err(|_| "Emplacement du lanceur inaccessible.")?
        .parent()
        .unwrap()
        .join("rusted");
    // SAFETY: the UI thread owns all GDI/window handles and the boxed App until WM_DESTROY.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let font = CreateFontW(
            16,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            wide("Segoe UI").as_ptr(),
        );
        let bold = CreateFontW(
            18,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let small = CreateFontW(
            13,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let mono = CreateFontW(
            14,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Consolas").as_ptr(),
        );
        let version_catalog = catalog::cached(&store);
        let app = Box::new(App {
            store,
            runtime,
            settings,
            instances,
            accounts,
            selected,
            page: 0,
            controls: vec![],
            font,
            bold,
            small,
            mono,
            brush: CreateSolidBrush(PANEL),
            rx,
            tx,
            job: None,
            running: false,
            status: "Choisis une instance, puis prepare les fichiers avant de jouer.".into(),
            progress: (0, 0),
            logs: VecDeque::new(),
            hits: vec![],
            device: None,
            window: std::ptr::null_mut(),
            log_dirty: false,
            auto_fired: false,
            version_catalog,
            catalog_busy: false,
            catalog_started: false,
            catalog_notice: "Catalogue inclus hors ligne".into(),
            draft_name: "Mon monde Rusted".into(),
            draft_engine: Engine::Java,
            draft_version: "26.3".into(),
            rendered_status: String::new(),
            rendered_progress: (0, 0),
            auth_job: None,
            account_config_open: false,
            account_notice: "Connecte ton compte avec le navigateur Microsoft".into(),
            mod_tab: 0,
            mod_query: "Sodium".into(),
            mod_searching: false,
            search_generation: 0,
            pending_play: None,
        });
        let h = GetModuleHandleW(std::ptr::null());
        let class = wide("RustedLauncherOriginal");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(proc),
            hInstance: h,
            hIcon: LoadIconW(h, 1usize as _),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("Enregistrement de la fenetre impossible.".into());
        }
        let raw = Box::into_raw(app);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Rusted Launcher").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1200,
            820,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            h,
            raw.cast(),
        );
        if hwnd.is_null() {
            drop(Box::from_raw(raw));
            return Err("Creation de la fenetre impossible.".into());
        }
        let dark = 1i32;
        DwmSetWindowAttribute(hwnd, 20, (&dark as *const i32).cast(), 4);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let a = Box::from_raw(raw);
        if let Some(c) = &a.job {
            c.store(true, Ordering::Relaxed);
        }
        if let Some(c) = &a.auth_job {
            c.store(true, Ordering::Relaxed);
        }
        DeleteObject(a.font);
        DeleteObject(a.bold);
        DeleteObject(a.small);
        DeleteObject(a.mono);
        DeleteObject(a.brush);
        drop(a);
        CoUninitialize();
    }
    Ok(())
}
thread_local! { static DISPATCHING: Cell<bool> = const { Cell::new(false) }; }
struct DispatchGuard;
impl Drop for DispatchGuard {
    fn drop(&mut self) {
        DISPATCHING.with(|v| v.set(false));
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as _);
        let a = &mut *(create.lpCreateParams as *mut App);
        a.window = hwnd;
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_DESTROY {
        KillTimer(hwnd, 1);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        PostQuitMessage(0);
        return 0;
    }
    // Color callbacks may be synchronous while controls are being created/painted.
    // They use only constants and a stock DC brush, without reborrowing App.
    if matches!(
        msg,
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN
    ) {
        let dc = w as HDC;
        let color = if msg == WM_CTLCOLOREDIT || msg == WM_CTLCOLORLISTBOX {
            PANEL
        } else {
            BG
        };
        SetTextColor(dc, TEXT);
        SetBkColor(dc, color);
        SetDCBrushColor(dc, color);
        return GetStockObject(DC_BRUSH) as LRESULT;
    }
    if msg == WM_MEASUREITEM {
        let item = &mut *(l as *mut MEASUREITEMSTRUCT);
        item.itemHeight = if item.CtlType == ODT_COMBOBOX {
            32
        } else {
            match item.CtlID {
                350 => 84,
                200 | 500 => 64,
                _ => 56,
            }
        };
        return 1;
    }
    if DISPATCHING.with(|v| v.replace(true)) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let _guard = DispatchGuard;
    let app = &mut *ptr;
    match msg {
        WM_CREATE => {
            build(app);
            SetTimer(hwnd, 1, 100, None);
            0
        }
        WM_GETMINMAXINFO => {
            let m = &mut *(l as *mut MINMAXINFO);
            m.ptMinTrackSize.x = 1120;
            m.ptMinTrackSize.y = 760;
            0
        }
        WM_SIZE => {
            layout(app);
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(app);
            0
        }
        WM_DRAWITEM => {
            let item = &*(l as *const DRAWITEMSTRUCT);
            if item.CtlType == ODT_BUTTON {
                draw_button(app, item);
            } else {
                draw_selection(app, item);
            }
            1
        }
        WM_COMMAND => {
            let id = (w & 65535) as u32;
            let notification = ((w >> 16) & 65535) as u32;
            action(app, id, notification);
            0
        }
        WM_TIMER => {
            tick(app);
            0
        }
        WM_CLOSE => {
            if app.running
                && MessageBoxW(
                    hwnd,
                    wide(
                        "Fermer Rusted Launcher et arreter le jeu ? Pense a sauvegarder ta partie.",
                    )
                    .as_ptr(),
                    wide("Rusted Launcher").as_ptr(),
                    MB_YESNO | MB_ICONQUESTION,
                ) != IDYES
            {
                return 0;
            }
            if let Some(c) = &app.job {
                c.store(true, Ordering::Relaxed)
            }
            if let Some(c) = &app.auth_job {
                c.store(true, Ordering::Relaxed)
            }
            DestroyWindow(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe fn control(
    app: &mut App,
    id: u32,
    class: &str,
    text: &str,
    style: u32,
    rect: [i32; 4],
    footer: bool,
) -> HWND {
    let h = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | style | if class == "EDIT" { WS_BORDER } else { 0 },
        rect[0],
        rect[1],
        rect[2],
        rect[3],
        app.window,
        id as usize as _,
        GetModuleHandleW(std::ptr::null()),
        std::ptr::null(),
    );
    SendMessageW(h, WM_SETFONT, app.font as _, 1);
    app.controls.push(Control {
        window: h,
        rect,
        footer,
    });
    h
}
unsafe fn label(app: &mut App, text: &str, x: i32, y: i32, w: i32) {
    control(app, 0, "STATIC", text, 0, [x, y, w, 24], false);
}
unsafe fn edit(app: &mut App, id: u32, text: &str, x: i32, y: i32, w: i32) {
    control(
        app,
        id,
        "EDIT",
        text,
        ES_AUTOHSCROLL as u32,
        [x, y, w, 28],
        false,
    );
}
unsafe fn button(app: &mut App, id: u32, text: &str, x: i32, y: i32, w: i32, h: i32) {
    control(
        app,
        id,
        "BUTTON",
        text,
        BS_OWNERDRAW as u32,
        [x, y, w, h],
        false,
    );
}
unsafe fn combo(app: &mut App, id: u32, items: &[&str], selected: usize, x: i32, y: i32, w: i32) {
    let h = control(
        app,
        id,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST as u32 | CBS_OWNERDRAWFIXED as u32 | CBS_HASSTRINGS as u32 | WS_VSCROLL,
        [x, y, w, 230],
        false,
    );
    for s in items {
        SendMessageW(h, CB_ADDSTRING, 0, wide(s).as_ptr() as _);
    }
    SendMessageW(h, CB_SETCURSEL, selected, 0);
    SendMessageW(h, CB_SETITEMHEIGHT, usize::MAX, 32);
    SetWindowTheme(h, wide("").as_ptr(), wide("").as_ptr());
    SetWindowSubclass(h, Some(choice_paint), 1, 0);
}
unsafe fn list(app: &mut App, id: u32, items: &[String], selected: usize, rect: [i32; 4]) {
    let h = control(
        app,
        id,
        "LISTBOX",
        "",
        LBS_NOTIFY as u32
            | LBS_OWNERDRAWFIXED as u32
            | LBS_HASSTRINGS as u32
            | WS_VSCROLL
            | LBS_NOINTEGRALHEIGHT as u32,
        rect,
        false,
    );
    for s in items {
        SendMessageW(h, LB_ADDSTRING, 0, wide(s).as_ptr() as _);
    }
    if !items.is_empty() {
        SendMessageW(h, LB_SETCURSEL, selected.min(items.len() - 1), 0);
    }
}
unsafe fn choice_value(hwnd: HWND) -> String {
    let index = SendMessageW(hwnd, CB_GETCURSEL, 0, 0);
    if index < 0 {
        return String::new();
    }
    let n = SendMessageW(hwnd, CB_GETLBTEXTLEN, index as usize, 0);
    if !(0..=8192).contains(&n) {
        return String::new();
    }
    let mut buffer = vec![0u16; n as usize + 1];
    SendMessageW(hwnd, CB_GETLBTEXT, index as usize, buffer.as_mut_ptr() as _);
    String::from_utf16_lossy(&buffer[..n as usize])
}
unsafe fn text(app: &App, id: u32) -> String {
    let h = GetDlgItem(app.window, id as i32);
    let mut class = [0u16; 32];
    let length = GetClassNameW(h, class.as_mut_ptr(), class.len() as i32);
    if String::from_utf16_lossy(&class[..length.max(0) as usize]).eq_ignore_ascii_case("ComboBox") {
        return choice_value(h);
    }
    let n = GetWindowTextLengthW(h).clamp(0, 16384);
    let mut b = vec![0u16; n as usize + 1];
    GetWindowTextW(h, b.as_mut_ptr(), b.len() as i32);
    String::from_utf16_lossy(&b[..n as usize])
}

unsafe fn chosen(app: &App, id: u32) -> usize {
    SendMessageW(
        GetDlgItem(app.window, id as i32),
        if id == 190 || id == 191 || id == 211 || id == 230 || id == 238 {
            CB_GETCURSEL
        } else {
            LB_GETCURSEL
        },
        0,
        0,
    )
    .max(0) as usize
}
unsafe fn checked(app: &App, id: u32) -> bool {
    SendMessageW(GetDlgItem(app.window, id as i32), BM_GETCHECK, 0, 0) == BST_CHECKED as isize
}
unsafe fn check(app: &mut App, id: u32, title: &str, value: bool, x: i32, y: i32, w: i32) {
    let h = control(
        app,
        id,
        "BUTTON",
        title,
        BS_AUTOCHECKBOX as u32,
        [x, y, w, 28],
        false,
    );
    SendMessageW(
        h,
        BM_SETCHECK,
        if value { BST_CHECKED as usize } else { 0 },
        0,
    );
}
unsafe fn build(app: &mut App) {
    SendMessageW(app.window, WM_SETREDRAW, 0, 0);
    for c in app.controls.drain(..) {
        DestroyWindow(c.window);
    }
    for (i, title) in [
        "Accueil",
        "Instances",
        "Mods",
        "Console",
        "Comptes",
        "Parametres",
        "Importer / Exporter",
    ]
    .iter()
    .enumerate()
    {
        button(app, 100 + i as u32, title, 20, 164 + i as i32 * 48, 208, 40);
    }
    let names = app
        .instances
        .iter()
        .map(|i| i.name.clone())
        .collect::<Vec<_>>();
    combo(
        app,
        190,
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
        app.selected,
        284,
        52,
        335,
    );
    let accounts = app
        .accounts
        .iter()
        .map(|i| i.name.clone())
        .collect::<Vec<_>>();
    let account = app
        .accounts
        .iter()
        .position(|a| a.id == app.settings.selected_account)
        .unwrap_or(0);
    combo(
        app,
        191,
        &accounts.iter().map(String::as_str).collect::<Vec<_>>(),
        account,
        820,
        52,
        320,
    );
    let selected = app.selected();
    match app.page {
        0 => {
            label(app, "TA PROCHAINE PARTIE COMMENCE ICI", 284, 116, 700);
            button(
                app,
                1,
                if app.running {
                    "ARRETER LE JEU"
                } else if app.pending_play.is_some() {
                    "JOUER DES QUE PRET"
                } else {
                    "JOUER"
                },
                306,
                315,
                320,
                48,
            );
            label(app, "CREE TON INSTANCE, PUIS JOUE", 284, 402, 760);
            label(
                app,
                "Les fichiers se preparent automatiquement a la creation ou a l'import de l'instance.",
                284,
                438,
                830,
            );
            label(
                app,
                "Clique sur Jouer : le lancement attend la fin des telechargements si necessaire.",
                284,
                470,
                830,
            );
            label(
                app,
                "Java / Fabric reste disponible pour les mods Java.",
                284,
                502,
                830,
            );
            button(app, 1010, "Mes instances", 284, 558, 350, 40);
            button(app, 1040, "Mon compte Microsoft", 650, 558, 350, 40);
        }
        1 => {
            label(app, "TES INSTANCES", 284, 116, 400);
            let names = app
                .instances
                .iter()
                .map(|i| format!("{}   /   {}", i.name, i.minecraft))
                .collect::<Vec<_>>();
            list(app, 200, &names, app.selected, [284, 155, 390, 340]);
            button(app, 224, "Dupliquer", 284, 512, 185, 38);
            button(app, 225, "Corbeille", 485, 512, 189, 38);
            button(app, 226, "Ouvrir le dossier", 284, 566, 390, 38);
            label(app, "CREER UNE INSTANCE", 712, 152, 400);
            label(app, "Nom", 712, 198, 400);
            edit(app, 210, &app.draft_name.clone(), 712, 226, 400);
            label(app, "Moteur", 712, 269, 400);
            combo(
                app,
                211,
                &["Rusted natif", "Minecraft Java", "Java + Fabric"],
                match app.draft_engine {
                    Engine::Native => 0,
                    Engine::Java => 1,
                    Engine::Fabric => 2,
                },
                712,
                297,
                400,
            );
            label(app, "Version Minecraft sans obfuscation", 712, 343, 400);
            let versions = if app.draft_engine == Engine::Native {
                vec!["26.3".to_owned()]
            } else {
                app.version_catalog.ids(app.settings.show_snapshots)
            };
            let selected_version = versions
                .iter()
                .position(|v| v == &app.draft_version)
                .unwrap_or(0);
            if let Some(v) = versions.get(selected_version) {
                app.draft_version = v.clone();
            }
            combo(
                app,
                212,
                &versions.iter().map(String::as_str).collect::<Vec<_>>(),
                selected_version,
                712,
                372,
                400,
            );
            check(
                app,
                217,
                "Snapshots, preversions et candidates",
                app.settings.show_snapshots,
                712,
                419,
                400,
            );
            button(
                app,
                218,
                if app.catalog_busy {
                    "Actualisation en cours..."
                } else {
                    "Actualiser depuis Mojang"
                },
                712,
                458,
                400,
                36,
            );
            label(
                app,
                "Rusted natif : 26.3. Autres versions : Java / Fabric.",
                712,
                516,
                405,
            );
            button(app, 213, "Creer l'instance", 712, 558, 400, 44);
            label(
                app,
                &format!("{} versions. {}", versions.len(), app.catalog_notice),
                712,
                620,
                410,
            );
        }
        2 => {
            label(app, "MODS & DECOUVERTES", 284, 116, 700);
            button(app, 320, "Mes mods", 284, 154, 180, 36);
            button(app, 321, "Decouvrir sur Modrinth", 480, 154, 260, 36);
            if app.mod_tab == 0 {
                if let Some(i) = &selected {
                    if i.engine == Engine::Native {
                        button(app, 334, "Ressources Fabric", 756, 154, 224, 36);
                        button(app, 335, "Voir les packs", 996, 154, 138, 36);
                    }
                    let files = mods::files(&app.store, i).unwrap_or_default();
                    if files.is_empty() {
                        label(
                            app,
                            "Aucun mod installe. Decouvre les mods compatibles sur Modrinth.",
                            304,
                            250,
                            800,
                        );
                        button(app, 322, "Explorer les mods", 304, 308, 300, 44);
                    } else {
                        list(app, 330, &files, 0, [284, 214, 850, 326]);
                        button(app, 331, "Activer / desactiver", 284, 566, 250, 40);
                        button(app, 332, "Retirer le mod", 550, 566, 250, 40);
                    }
                    button(app, 333, "Ouvrir le dossier", 816, 566, 318, 40);
                    label(
                        app,
                        if i.engine == Engine::Native {
                            "Mods natifs : activation et options dans le menu Mods du jeu."
                        } else {
                            "Les fichiers desactives restent dans l'instance et peuvent etre reactives."
                        },
                        284,
                        624,
                        850,
                    );
                }
            } else {
                edit(app, 340, &app.mod_query.clone(), 284, 214, 650);
                button(
                    app,
                    341,
                    if app.mod_searching {
                        "Recherche..."
                    } else {
                        "Rechercher"
                    },
                    950,
                    212,
                    184,
                    36,
                );
                button(app, 360, "Sodium", 284, 258, 150, 32);
                button(app, 361, "Iris", 450, 258, 150, 32);
                button(app, 362, "Distant Horizons", 616, 258, 230, 32);
                let hits = app.hits.iter().map(|h| h.name.clone()).collect::<Vec<_>>();
                if hits.is_empty() {
                    label(
                        app,
                        if app.mod_searching {
                            "Recherche des mods compatibles avec cette version..."
                        } else {
                            "Choisis une suggestion ou cherche un mod Fabric."
                        },
                        304,
                        354,
                        800,
                    );
                } else {
                    list(app, 350, &hits, 0, [284, 310, 850, 264]);
                }
                button(app, 351, "Installer le mod selectionne", 784, 596, 350, 44);
                label(
                    app,
                    "Fabric / Java : dependances requises installees automatiquement.",
                    284,
                    614,
                    490,
                );
            }
        }
        3 => {
            label(app, "CONSOLE DU LANCEUR ET DU JEU", 284, 116, 780);
            let log = app.logs.iter().cloned().collect::<Vec<_>>().join("\r\n");
            let h = control(
                app,
                400,
                "EDIT",
                &log,
                ES_MULTILINE as u32 | ES_READONLY as u32 | WS_VSCROLL | ES_AUTOVSCROLL as u32,
                [284, 156, 850, 420],
                false,
            );
            SendMessageW(h, WM_SETFONT, app.mono as _, 1);
            button(app, 401, "Copier les journaux", 284, 592, 240, 36);
            button(app, 402, "Effacer l'affichage", 540, 592, 240, 36);
        }
        4 => {
            label(app, "COMPTES MICROSOFT", 284, 116, 700);
            let names = app
                .accounts
                .iter()
                .map(|a| format!("{}  /  Minecraft Java", a.name))
                .collect::<Vec<_>>();
            list(app, 500, &names, account, [284, 160, 360, 225]);
            button(app, 502, "Utiliser ce compte", 284, 402, 360, 38);
            button(app, 503, "Deconnecter de Rusted", 284, 454, 360, 38);
            label(app, "AJOUTER UN COMPTE", 680, 154, 450);
            label(
                app,
                "Une connexion securisee dans ton navigateur.",
                680,
                196,
                450,
            );
            button(
                app,
                501,
                if app.auth_job.is_some() {
                    "Connexion Microsoft en cours..."
                } else {
                    "Se connecter avec Microsoft"
                },
                680,
                242,
                440,
                48,
            );
            control(
                app,
                0,
                "STATIC",
                &app.account_notice.clone(),
                0,
                [680, 310, 450, 44],
                false,
            );
            if let Some((_, code)) = &app.device.clone() {
                label(app, &format!("TON CODE : {code}"), 680, 356, 450);
                button(app, 507, "Rouvrir Microsoft", 680, 402, 210, 40);
                button(app, 508, "Copier le code", 906, 402, 214, 40);
                button(app, 509, "Annuler la connexion", 680, 456, 440, 36);
            } else {
                label(
                    app,
                    "Les sessions sont chiffrees par Windows.",
                    680,
                    356,
                    450,
                );
                button(
                    app,
                    506,
                    "Configuration Microsoft du projet",
                    680,
                    410,
                    440,
                    36,
                );
                if app.account_config_open {
                    edit(
                        app,
                        504,
                        &app.settings.microsoft_client_id.clone(),
                        680,
                        465,
                        440,
                    );
                    label(
                        app,
                        "ID public de l'application Rusted (pas de secret)",
                        680,
                        504,
                        440,
                    );
                    button(app, 505, "Guide de configuration", 680, 548, 440, 36);
                }
            }
        }
        5 => {
            label(app, "REGLAGES DE L'INSTANCE", 284, 116, 700);
            if let Some(i) = &selected {
                label(app, "Nom", 284, 154, 350);
                edit(app, 220, &i.name, 284, 181, 350);
                label(app, "Groupe", 672, 154, 450);
                edit(app, 221, &i.group, 672, 181, 450);
                label(app, "Moteur", 284, 226, 350);
                combo(
                    app,
                    230,
                    &["Rusted natif", "Minecraft Java", "Java + Fabric"],
                    match i.engine {
                        Engine::Native => 0,
                        Engine::Java => 1,
                        Engine::Fabric => 2,
                    },
                    284,
                    253,
                    350,
                );
                label(
                    app,
                    "Memoire Java (Mo, sans effet sur le moteur Rust)",
                    672,
                    226,
                    450,
                );
                edit(app, 231, &i.memory_mb.to_string(), 672, 253, 450);
                label(
                    app,
                    "Java personnalise (vide = version Java automatique)",
                    284,
                    298,
                    760,
                );
                edit(app, 232, &i.java_path, 284, 325, 710);
                button(app, 242, "Parcourir", 1008, 323, 114, 32);
                label(app, "Resolution", 284, 370, 350);
                edit(app, 235, &i.width.to_string(), 284, 398, 160);
                edit(app, 236, &i.height.to_string(), 460, 398, 174);
                label(app, "Distance native / API graphique", 672, 370, 450);
                edit(app, 237, &i.render_distance.to_string(), 672, 398, 150);
                combo(
                    app,
                    238,
                    &["auto", "vulkan", "dx12", "opengl"],
                    ["auto", "vulkan", "dx12", "opengl"]
                        .iter()
                        .position(|v| *v == i.graphics_api)
                        .unwrap_or(0),
                    840,
                    398,
                    282,
                );
                label(
                    app,
                    "Arguments Java avances (un argument par ligne)",
                    284,
                    440,
                    840,
                );
                control(
                    app,
                    233,
                    "EDIT",
                    &i.java_args.join("\r\n"),
                    ES_MULTILINE as u32 | WS_VSCROLL,
                    [284, 468, 838, 72],
                    false,
                );
                check(app, 239, "Plein ecran", i.fullscreen, 284, 558, 230);
                check(
                    app,
                    240,
                    "Verifier les fichiers a l'ouverture",
                    i.auto_prepare,
                    560,
                    558,
                    400,
                );
                button(app, 241, "Enregistrer les reglages", 284, 604, 350, 38);
                check(
                    app,
                    610,
                    "Reduire le lanceur pendant le jeu",
                    app.settings.hide_while_playing,
                    672,
                    604,
                    450,
                );
            }
        }
        6 => {
            label(app, "IMPORTER OU PARTAGER UNE INSTANCE", 284, 116, 850);
            label(
                app,
                "Importer un pack Prism .zip ou Modrinth .mrpack",
                284,
                174,
                850,
            );
            label(app,"L'import cree une instance distincte. Les commandes shell des packs ne sont pas executees.",284,208,850);
            button(app, 601, "Choisir un pack a importer", 284, 260, 410, 44);
            label(
                app,
                "Exporter l'instance selectionnee vers Prism",
                284,
                352,
                850,
            );
            label(app,"L'export contient les reglages, mods et le runtime natif si necessaire. Sans comptes ni mondes.",284,386,850);
            button(app, 602, "Exporter un ZIP Prism", 284, 438, 410, 44);
            button(app, 603, "Ouvrir le dossier du lanceur", 284, 522, 410, 40);
        }
        _ => {}
    }
    control(
        app,
        800,
        "STATIC",
        &app.status.clone(),
        0,
        [284, 0, 850, 26],
        true,
    );
    if let Some(instance) = &selected {
        control(
            app,
            801,
            "STATIC",
            &format!("{}  /  Minecraft {}", instance.name, instance.minecraft),
            windows_sys::Win32::System::SystemServices::SS_ENDELLIPSIS,
            [284, 42, 440, 26],
            true,
        );
    }
    if app.page != 0 {
        control(
            app,
            1,
            "BUTTON",
            if app.running {
                "ARRETER"
            } else if app.pending_play.is_some() {
                "JOUER DES QUE PRET"
            } else {
                "JOUER"
            },
            BS_OWNERDRAW as u32,
            [900, 34, 234, 48],
            true,
        );
    }
    if !app.running && (app.job.is_some() || app.pending_play.is_some()) {
        control(
            app,
            900,
            "BUTTON",
            "Annuler",
            BS_OWNERDRAW as u32,
            if app.page == 0 {
                [940, 34, 194, 48]
            } else {
                [734, 34, 150, 48]
            },
            true,
        );
    }
    layout(app);
    app.rendered_status = app.status.clone();
    SendMessageW(app.window, WM_SETREDRAW, 1, 0);
    RedrawWindow(
        app.window,
        std::ptr::null(),
        std::ptr::null_mut(),
        RDW_INVALIDATE | RDW_ALLCHILDREN,
    );
}
unsafe fn layout(app: &App) {
    let mut r: RECT = std::mem::zeroed();
    GetClientRect(app.window, &mut r);
    for c in &app.controls {
        let mut v = c.rect;
        if c.footer {
            v[1] += r.bottom - 91;
            if v[0] >= 734 {
                v[0] += r.right - 1184;
            }
        }
        MoveWindow(c.window, v[0], v[1], v[2], v[3], 1);
    }
}
unsafe fn remember_draft(app: &mut App) {
    if app.page == 1 && !GetDlgItem(app.window, 210).is_null() {
        app.draft_name = text(app, 210);
        app.draft_version = text(app, 212);
        app.draft_engine = match chosen(app, 211) {
            0 => Engine::Native,
            2 => Engine::Fabric,
            _ => Engine::Java,
        };
    }
}
fn refresh_catalog(app: &mut App) {
    if app.catalog_busy {
        return;
    }
    app.catalog_busy = true;
    let store = app.store.clone();
    let tx = app.tx.clone();
    std::thread::spawn(move || {
        let event = match catalog::refresh(&store) {
            Ok(c) => Event::VersionCatalog(c),
            Err(e) => Event::VersionCatalogFailed(e),
        };
        let _ = tx.send(event);
    });
}
unsafe fn action(app: &mut App, id: u32, notice: u32) {
    remember_draft(app);
    if app.page == 2 && !GetDlgItem(app.window, 340).is_null() {
        app.mod_query = text(app, 340);
    }
    if id == 211 && notice == CBN_SELCHANGE {
        build(app);
        return;
    }
    if id == 212 && notice == CBN_SELCHANGE {
        return;
    }
    if (100..=106).contains(&id) {
        app.page = id - 100;
        build(app);
        return;
    }
    if id == 1010 {
        app.page = 1;
        build(app);
        return;
    }
    if id == 1040 {
        app.page = 4;
        build(app);
        return;
    }
    if id == 190 && notice == CBN_SELCHANGE {
        app.reload_selection(chosen(app, 190));
        return;
    }
    if id == 191 && notice == CBN_SELCHANGE {
        if let Some(a) = app.accounts.get(chosen(app, 191)) {
            app.settings.selected_account = a.id.clone();
            let _ = app.store.save_settings(&app.settings);
        }
        return;
    }
    if id == 200 && notice == LBN_SELCHANGE {
        app.reload_selection(chosen(app, 200));
        return;
    }
    if app.job.is_some() && matches!(id, 213 | 224 | 225 | 241 | 331 | 332 | 351 | 601 | 602) {
        app.message("Terminer ou annuler l'operation en cours avant de modifier cette instance.");
        return;
    }
    let result = (|| -> Result<()> {
        match id {
            1 => {
                if app.running {
                    if let Some(c) = &app.job {
                        c.store(true, Ordering::Relaxed)
                    }
                    return Ok(());
                }
                let i = app.selected_or_error()?;
                if app.job.is_some() {
                    app.pending_play = Some(i.id);
                    app.status = "Le jeu se lancera automatiquement apres la preparation.".into();
                    build(app);
                    return Ok(());
                }
                let account = app.settings.selected_account.clone();
                if account.is_empty() {
                    app.pending_play = Some(i.id);
                    app.page = 4;
                    app.account_notice =
                        "Connecte un compte Microsoft pour lancer ton instance.".into();
                    build(app);
                    if !app.settings.microsoft_client_id.is_empty() && app.auth_job.is_none() {
                        action(app, 501, 0);
                    }
                    return Ok(());
                }
                app.pending_play = None;
                let store = app.store.clone();
                let runtime = app.runtime.clone();
                app.start("Preparation avant le lancement", move |task| {
                    let session = auth::session(&store, &account, &task)?;
                    launch::play(&store, &i, &runtime, &session, &task)?;
                    Ok("Partie terminee".into())
                });
            }
            900 => {
                app.pending_play = None;
                if let Some(c) = &app.job {
                    c.store(true, Ordering::Relaxed);
                    app.status = "Annulation en cours...".into();
                } else {
                    app.status = "Lancement automatique annule.".into();
                }
                build(app);
            }
            217 => {
                app.settings.show_snapshots = checked(app, 217);
                app.store.save_settings(&app.settings)?;
                build(app);
            }
            218 => {
                refresh_catalog(app);
                build(app);
            }
            213 => {
                let engine = match chosen(app, 211) {
                    1 => Engine::Java,
                    2 => Engine::Fabric,
                    _ => Engine::Native,
                };
                let i = app.store.create(&text(app, 210), engine, &text(app, 212))?;
                app.settings.selected = i.id.clone();
                app.store.save_settings(&app.settings)?;
                app.refresh();
                let store = app.store.clone();
                let runtime = app.runtime.clone();
                app.start(
                    "Preparation automatique de la nouvelle instance",
                    move |task| {
                        launch::prepare(&store, &i, &runtime, &task)?;
                        Ok("Instance prete : tous les fichiers sont installes".into())
                    },
                );
                build(app);
            }
            224 => {
                let i = app.selected_or_error()?;
                let store = app.store.clone();
                app.start("Duplication de l'instance", move |task| {
                    task.check()?;
                    let v = store.duplicate(&i)?;
                    Ok(format!("Instance creee : {}", v.name))
                });
            }
            225 => {
                let i = app.selected_or_error()?;
                if MessageBoxW(app.window,wide("Deplacer cette instance vers la corbeille Rusted ? Ses fichiers restent recuperables dans le dossier trash.").as_ptr(),wide("Corbeille Rusted").as_ptr(),MB_YESNO|MB_ICONQUESTION)==IDYES{app.store.trash(&i)?;app.refresh();build(app);}
            }
            226 => {
                open(&app.store.game(&app.selected_or_error()?)?.to_string_lossy());
            }
            241 => {
                let mut i = app.selected_or_error()?;
                i.name = text(app, 220);
                i.group = text(app, 221);
                i.engine = match chosen(app, 230) {
                    1 => Engine::Java,
                    2 => Engine::Fabric,
                    _ => Engine::Native,
                };
                i.memory_mb = text(app, 231)
                    .parse()
                    .map_err(|_| "Memoire Java invalide.")?;
                i.java_path = text(app, 232);
                i.java_args = text(app, 233)
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
                i.width = text(app, 235).parse().map_err(|_| "Largeur invalide.")?;
                i.height = text(app, 236).parse().map_err(|_| "Hauteur invalide.")?;
                i.render_distance = text(app, 237).parse().map_err(|_| "Distance invalide.")?;
                i.graphics_api =
                    ["auto", "vulkan", "dx12", "opengl"][chosen(app, 238).min(3)].into();
                i.fullscreen = checked(app, 239);
                i.auto_prepare = checked(app, 240);
                app.store.save(&i)?;
                app.settings.hide_while_playing = checked(app, 610);
                app.store.save_settings(&app.settings)?;
                app.refresh();
                app.status = "Reglages enregistres".into();
                build(app);
            }
            242 => {
                if let Some(p) = file_dialog(
                    app.window,
                    false,
                    "Choisir java.exe",
                    "Executables\0*.exe\0Tous les fichiers\0*.*\0\0",
                ) {
                    SetWindowTextW(
                        GetDlgItem(app.window, 232),
                        wide(&p.to_string_lossy()).as_ptr(),
                    );
                }
            }
            331 | 332 => {
                let i = app.selected_or_error()?;
                let files = mods::files(&app.store, &i)?;
                let name = files.get(chosen(app, 330)).ok_or("Selectionner un mod.")?;
                if id == 331 {
                    mods::toggle(&app.store, &i, name)?
                } else {
                    mods::trash(&app.store, &i, name)?
                }
                build(app);
            }
            333 => {
                let i = app.selected_or_error()?;
                let p = app.store.game(&i)?.join(if i.engine == Engine::Native {
                    "rusted/native-data/horizon-mods"
                } else {
                    "mods"
                });
                std::fs::create_dir_all(&p).map_err(|_| "Dossier des mods inaccessible.")?;
                open(&p.to_string_lossy());
            }
            334 => {
                let i = app.selected_or_error()?;
                if let Some(path) = file_dialog(
                    app.window,
                    false,
                    "Adapter un mod Fabric sans code (textures / langues)",
                    "Mods Fabric\0*.jar\0\0",
                ) {
                    let store = app.store.clone();
                    app.start("Adaptation des ressources pour Rusted natif", move |task| {
                        compat::import_resources(&store, &i, &path, &task)?;
                        Ok("Ressources adaptees. Active le pack dans Options > Packs de ressources du jeu.".into())
                    });
                }
            }
            335 => {
                let i = app.selected_or_error()?;
                let folder = app.store.game(&i)?.join("rusted/native-data/resourcepacks");
                std::fs::create_dir_all(&folder).map_err(|_| "Dossier des packs inaccessible.")?;
                open(&folder.to_string_lossy());
            }
            320..=322 => {
                if !GetDlgItem(app.window, 340).is_null() {
                    app.mod_query = text(app, 340);
                }
                app.mod_tab = if id == 320 { 0 } else { 1 };
                build(app);
            }
            360..=362 => {
                app.mod_query = ["Sodium", "Iris", "Distant Horizons"][(id - 360) as usize].into();
                SetWindowTextW(GetDlgItem(app.window, 340), wide(&app.mod_query).as_ptr());
                action(app, 341, 0);
            }
            341 => {
                app.mod_query = text(app, 340);
                let query = app.mod_query.clone();
                let version = app.selected_or_error()?.minecraft;
                app.search_generation += 1;
                let request = app.search_generation;
                app.mod_searching = true;
                let tx = app.tx.clone();
                std::thread::spawn(move || {
                    let event = match mods::search(&query, &version) {
                        Ok(hits) => Event::SearchResults { request, hits },
                        Err(error) => Event::SearchFailed { request, error },
                    };
                    let _ = tx.send(event);
                });
                build(app);
            }
            351 => {
                let hit = app
                    .hits
                    .get(chosen(app, 350))
                    .ok_or("Selectionner un resultat Modrinth.")?
                    .clone();
                let mut i = app.selected_or_error()?;
                if i.engine!=Engine::Fabric&&MessageBoxW(app.window,wide("Installer ce mod et ses dependances en passant cette instance sur Java / Fabric ? Les mondes natifs restent separes.").as_ptr(),wide("Installation Fabric").as_ptr(),MB_YESNO|MB_ICONQUESTION)!=IDYES{return Ok(())}
                let store = app.store.clone();
                app.start("Installation des mods Fabric", move |task| {
                    mods::install(&store, &mut i, &hit.id, &task)?;
                    Ok("Mods installes et verifies".into())
                });
            }
            401 => {
                clipboard(
                    app.window,
                    &app.logs.iter().cloned().collect::<Vec<_>>().join("\r\n"),
                )?;
            }
            402 => {
                app.logs.clear();
                SetWindowTextW(GetDlgItem(app.window, 400), wide("").as_ptr());
            }
            501 => {
                if !GetDlgItem(app.window, 504).is_null() {
                    app.settings.microsoft_client_id = text(app, 504).trim().into();
                    app.store.save_settings(&app.settings)?;
                }
                if app.settings.microsoft_client_id.trim().is_empty() {
                    app.account_config_open = true;
                    app.account_notice =
                        "Le mainteneur doit configurer l'ID Microsoft de Rusted une seule fois."
                            .into();
                    build(app);
                    return Ok(());
                }
                if app.auth_job.is_some() {
                    return Ok(());
                }
                let cancel = Arc::new(AtomicBool::new(false));
                app.auth_job = Some(cancel.clone());
                let task = Task {
                    events: app.tx.clone(),
                    cancel,
                };
                let client = app.settings.microsoft_client_id.clone();
                let store = app.store.clone();
                app.account_notice = "Ouverture de Microsoft dans ton navigateur...".into();
                std::thread::spawn(move || {
                    let event = match auth::sign_in(&store, &client, &task) {
                        Ok(a) => Event::AccountConnected(a),
                        Err(e) => Event::AccountFailed(e),
                    };
                    task.send(event);
                });
                build(app);
            }
            506 => {
                app.account_config_open = !app.account_config_open;
                build(app);
            }
            509 => {
                if let Some(c) = &app.auth_job {
                    c.store(true, Ordering::Relaxed);
                }
            }
            502 => {
                let a = app
                    .accounts
                    .get(chosen(app, 500))
                    .ok_or("Selectionner un compte.")?;
                app.settings.selected_account = a.id.clone();
                app.store.save_settings(&app.settings)?;
                app.status = format!("Compte actif : {}", a.name);
                build(app);
            }
            503 => {
                let a = app
                    .accounts
                    .get(chosen(app, 500))
                    .ok_or("Selectionner un compte.")?
                    .clone();
                auth::forget(&app.store, &a.id)?;
                if app.settings.selected_account == a.id {
                    app.settings.selected_account.clear();
                    app.store.save_settings(&app.settings)?;
                }
                app.refresh();
                build(app);
            }
            505 => {
                let guide = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.join("MICROSOFT-SETUP.html")));
                if let Some(p) = guide.filter(|p| p.is_file()) {
                    open(&p.to_string_lossy());
                } else {
                    open("https://learn.microsoft.com/entra/identity-platform/quickstart-register-app");
                }
            }
            507 => {
                if let Some((uri, _)) = &app.device {
                    open(uri);
                }
            }
            508 => {
                if let Some((_, code)) = &app.device {
                    clipboard(app.window, code)?;
                }
            }
            601 => {
                if let Some(file) = file_dialog(
                    app.window,
                    false,
                    "Importer un pack complet",
                    "Packs Minecraft\0*.zip;*.mrpack\0Tous les fichiers\0*.*\0\0",
                ) {
                    let store = app.store.clone();
                    let runtime = app.runtime.clone();
                    app.start("Import du pack", move |task| {
                        let v = packs::import(&store, &file, &runtime, &task)?;
                        let mut s = store.settings()?;
                        s.selected = v.id.clone();
                        store.save_settings(&s)?;
                        launch::prepare(&store, &v, &runtime, &task)?;
                        Ok("Pack importe et prepare dans une nouvelle instance".into())
                    });
                }
            }
            602 => {
                let i = app.selected_or_error()?;
                if let Some(path) = file_dialog(
                    app.window,
                    true,
                    "Exporter pour Prism",
                    "Pack Prism\0*.zip\0\0",
                ) {
                    let store = app.store.clone();
                    let runtime = app.runtime.clone();
                    app.start("Export du pack Prism", move |task| {
                        packs::export_prism(&store, &i, &runtime, &path, &task)?;
                        Ok("Pack Prism exporte, sans comptes ni mondes".into())
                    });
                }
            }
            603 => {
                open(&app.store.root.to_string_lossy());
            }
            _ => {}
        }
        Ok(())
    })();
    if let Err(e) = result {
        app.fail(e)
    }
    SetWindowTextW(GetDlgItem(app.window, 800), wide(&app.status).as_ptr());
    InvalidateRect(app.window, std::ptr::null(), 0);
}
unsafe fn tick(app: &mut App) {
    if !app.catalog_started {
        app.catalog_started = true;
        refresh_catalog(app);
    }
    if !app.auto_fired {
        app.auto_fired = true;
        if let Some(instance) = app.selected().filter(|i| i.auto_prepare) {
            let store = app.store.clone();
            let runtime = app.runtime.clone();
            app.start(
                "Verification automatique des fichiers de l'instance",
                move |task| {
                    launch::prepare(&store, &instance, &runtime, &task)?;
                    Ok("Instance prete : tous les fichiers sont installes".into())
                },
            );
            build(app);
        }
    }
    let mut rebuild = false;
    for _ in 0..1000 {
        let Ok(e) = app.rx.try_recv() else { break };
        match e {
            Event::Status(s) => {
                app.status = s.clone();
                app.logs.push_back(format!("[Rusted] {s}"));
                app.log_dirty = true;
            }
            Event::Progress { done, total, .. } => app.progress = (done, total),
            Event::Log(s) => {
                app.logs.push_back(s);
                app.log_dirty = true;
            }
            Event::AccountConnected(account) => {
                app.auth_job = None;
                app.device = None;
                app.settings.selected_account = account.id;
                let _ = app.store.save_settings(&app.settings);
                app.account_notice = format!("Connecte : {}", account.name);
                app.refresh();
                rebuild = true;
            }
            Event::AccountFailed(error) => {
                app.pending_play = None;
                app.auth_job = None;
                app.device = None;
                app.account_notice = error;
                rebuild = true;
            }
            Event::DeviceCode { uri, code } => {
                // Open the trusted Microsoft verification page as soon as the device flow is ready.
                open(&uri);
                let _ = clipboard(app.window, &code);
                app.account_notice =
                    "Navigateur ouvert. Le code Microsoft est copie : colle-le pour te connecter."
                        .into();
                app.device = Some((uri, code));
                app.page = 4;
                rebuild = true;
            }
            Event::VersionCatalog(catalogue) => {
                app.version_catalog = catalogue;
                app.catalog_busy = false;
                app.catalog_notice = "Catalogue Mojang a jour".into();
                rebuild |= app.page == 1;
            }
            Event::VersionCatalogFailed(error) => {
                app.catalog_busy = false;
                app.catalog_notice = "Catalogue hors ligne conserve".into();
                app.logs.push_back(format!(
                    "[Rusted] {error} Le catalogue existant reste disponible."
                ));
                app.log_dirty = true;
                rebuild |= app.page == 1;
            }
            Event::SearchResults { request, hits } => {
                if request == app.search_generation {
                    app.mod_searching = false;
                    app.hits = hits;
                    rebuild = app.page == 2 || rebuild;
                }
            }
            Event::SearchFailed { request, error } => {
                if request == app.search_generation {
                    app.mod_searching = false;
                    app.logs.push_back(error.clone());
                    app.status = error;
                    rebuild = app.page == 2 || rebuild;
                }
            }
            Event::GameStarted => {
                app.running = true;
                if app.settings.hide_while_playing {
                    ShowWindow(app.window, SW_MINIMIZE);
                }
                rebuild = true;
            }
            Event::GameStopped(code) => {
                app.running = false;
                app.logs.push_back(format!("[Rusted] Fin du jeu : {code}"));
                ShowWindow(app.window, SW_RESTORE);
                rebuild = true;
            }
            Event::Done(s) => {
                app.job = None;
                app.running = false;
                if app.auth_job.is_none() {
                    app.device = None;
                }
                if s.starts_with("Mods installes") {
                    app.mod_tab = 0;
                }
                app.status = s;
                app.settings = app
                    .store
                    .settings()
                    .unwrap_or_else(|_| app.settings.clone());
                app.refresh();
                rebuild = true;
            }
            Event::Failed(s) => {
                app.pending_play = None;
                let cancelled = app.job.as_ref().is_some_and(|c| c.load(Ordering::Relaxed));
                app.job = None;
                app.running = false;
                if app.auth_job.is_none() {
                    app.device = None;
                }
                app.status = s.clone();
                app.logs.push_back(s.clone());
                app.log_dirty = true;
                if !cancelled {
                    app.message(&s);
                }
                ShowWindow(app.window, SW_RESTORE);
                rebuild = true;
            }
        }
    }
    if app.job.is_none() && app.auth_job.is_none() && !app.settings.selected_account.is_empty() {
        if let Some(id) = app.pending_play.clone() {
            if app.selected().is_some_and(|i| i.id == id) {
                action(app, 1, 0);
            } else {
                app.pending_play = None;
            }
        }
    }
    while app.logs.len() > 600 {
        app.logs.pop_front();
    }
    if rebuild {
        remember_draft(app);
        if app.page == 2 && !GetDlgItem(app.window, 340).is_null() {
            app.mod_query = text(app, 340);
        }
        build(app);
    } else {
        let s = if app.job.is_some() && app.progress.1 > 0 {
            format!(
                "{}  -  {} / {} Mo",
                app.status,
                app.progress.0 / 1048576,
                app.progress.1 / 1048576
            )
        } else {
            app.status.clone()
        };
        if app.rendered_status != s {
            SetWindowTextW(GetDlgItem(app.window, 800), wide(&s).as_ptr());
            app.rendered_status = s;
        }
    }
    if app.page == 3 && app.log_dirty {
        let h = GetDlgItem(app.window, 400);
        SetWindowTextW(
            h,
            wide(&app.logs.iter().cloned().collect::<Vec<_>>().join("\r\n")).as_ptr(),
        );
        SendMessageW(h, EM_SETSEL, usize::MAX, -1);
        SendMessageW(h, EM_SCROLLCARET, 0, 0);
        app.log_dirty = false;
    }
    if app.rendered_progress != app.progress {
        app.rendered_progress = app.progress;
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(app.window, &mut rect);
        rect.top = rect.bottom - 12;
        InvalidateRect(app.window, &rect, 0);
    }
}
unsafe fn open(path: &str) {
    ShellExecuteW(
        std::ptr::null_mut(),
        wide("open").as_ptr(),
        wide(path).as_ptr(),
        std::ptr::null(),
        std::ptr::null(),
        SW_SHOWNORMAL,
    );
}
unsafe fn file_dialog(owner: HWND, save: bool, title: &str, filter: &str) -> Option<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let filter = filter.encode_utf16().collect::<Vec<_>>();
    let title = wide(title);
    let mut ofn: OPENFILENAMEW = std::mem::zeroed();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFile = buffer.as_mut_ptr();
    ofn.nMaxFile = buffer.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    ofn.lpstrFilter = filter.as_ptr();
    ofn.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    let extension = wide("zip");
    ofn.lpstrDefExt = extension.as_ptr();
    let ok = if save {
        GetSaveFileNameW(&mut ofn)
    } else {
        GetOpenFileNameW(&mut ofn)
    };
    if ok == 0 {
        return None;
    }
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    use std::os::windows::ffi::OsStringExt;
    Some(PathBuf::from(OsString::from_wide(&buffer[..end])))
}
unsafe fn clipboard(owner: HWND, value: &str) -> Result<()> {
    use windows_sys::Win32::System::DataExchange::*;
    let w = wide(value);
    let data = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2);
    if data.is_null() {
        return Err("Presse-papiers indisponible.".into());
    }
    let ptr = GlobalLock(data).cast::<u16>();
    if ptr.is_null() {
        windows_sys::Win32::Foundation::GlobalFree(data);
        return Err("Presse-papiers indisponible.".into());
    }
    std::ptr::copy_nonoverlapping(w.as_ptr(), ptr, w.len());
    GlobalUnlock(data);
    if OpenClipboard(owner) == 0 {
        windows_sys::Win32::Foundation::GlobalFree(data);
        return Err("Presse-papiers occupe.".into());
    }
    let ok = EmptyClipboard() != 0 && !SetClipboardData(13, data).is_null();
    CloseClipboard();
    if !ok {
        windows_sys::Win32::Foundation::GlobalFree(data);
        return Err("Copie impossible.".into());
    }
    Ok(())
}
unsafe fn fill(dc: HDC, r: RECT, color: u32) {
    let b = CreateSolidBrush(color);
    FillRect(dc, &r, b);
    DeleteObject(b);
}
unsafe fn rectangle(dc: HDC, x: i32, y: i32, w: i32, h: i32, color: u32) {
    fill(
        dc,
        RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        },
        color,
    )
}
#[allow(clippy::too_many_arguments)] // Win32 drawing coordinates and text style.
unsafe fn line_text(
    dc: HDC,
    font: HFONT,
    s: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: u32,
    flags: u32,
) {
    SelectObject(dc, font);
    SetTextColor(dc, color);
    SetBkMode(dc, TRANSPARENT as i32);
    let mut r = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    DrawTextW(dc, wide(s).as_ptr(), -1, &mut r, flags | DT_NOPREFIX);
}
// Retain native keyboard/accessibility behavior while painting the complete choice control.
unsafe extern "system" fn choice_paint(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if msg == WM_ERASEBKGND || msg == WM_NCPAINT {
        return 1;
    }
    if msg == WM_PAINT {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        fill(dc, r, BG);
        let border = CreatePen(PS_SOLID, 1, rgb(94, 75, 61));
        let brush = CreateSolidBrush(rgb(43, 39, 35));
        let old_pen = SelectObject(dc, border);
        let old_brush = SelectObject(dc, brush);
        RoundRect(dc, 0, 0, r.right, r.bottom, 10, 10);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(border);
        DeleteObject(brush);
        let title = choice_value(hwnd);
        let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT;
        line_text(
            dc,
            font,
            if title.is_empty() {
                "Choisir..."
            } else {
                &title
            },
            12,
            0,
            r.right - 46,
            r.bottom,
            TEXT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        let pen = CreatePen(PS_SOLID, 2, rgb(210, 143, 98));
        let old = SelectObject(dc, pen);
        MoveToEx(dc, r.right - 26, r.bottom / 2 - 2, std::ptr::null_mut());
        LineTo(dc, r.right - 21, r.bottom / 2 + 3);
        LineTo(dc, r.right - 16, r.bottom / 2 - 2);
        SelectObject(dc, old);
        DeleteObject(pen);
        EndPaint(hwnd, &ps);
        return 0;
    }
    DefSubclassProc(hwnd, msg, w, l)
}

unsafe fn draw_selection(app: &App, d: &DRAWITEMSTRUCT) {
    let dc = d.hDC;
    let r = d.rcItem;
    fill(dc, r, BG);
    if d.itemID == u32::MAX {
        return;
    }
    let selected = d.itemState & ODS_SELECTED != 0;
    let x = r.left + 2;
    let y = r.top + 3;
    let width = r.right - r.left - 4;
    let height = r.bottom - r.top - 6;
    let brush = CreateSolidBrush(if selected {
        rgb(69, 48, 37)
    } else {
        rgb(39, 37, 34)
    });
    let pen = CreatePen(
        PS_SOLID,
        1,
        if selected {
            rgb(184, 112, 72)
        } else {
            rgb(53, 48, 41)
        },
    );
    let old_b = SelectObject(dc, brush);
    let old_p = SelectObject(dc, pen);
    RoundRect(dc, x, y, x + width, y + height, 10, 10);
    SelectObject(dc, old_b);
    SelectObject(dc, old_p);
    DeleteObject(brush);
    DeleteObject(pen);
    let n = SendMessageW(
        d.hwndItem,
        if d.CtlType == ODT_COMBOBOX {
            CB_GETLBTEXTLEN
        } else {
            LB_GETTEXTLEN
        },
        d.itemID as usize,
        0,
    )
    .clamp(0, 8192);
    let mut value = vec![0u16; n as usize + 1];
    SendMessageW(
        d.hwndItem,
        if d.CtlType == ODT_COMBOBOX {
            CB_GETLBTEXT
        } else {
            LB_GETTEXT
        },
        d.itemID as usize,
        value.as_mut_ptr() as _,
    );
    let title = String::from_utf16_lossy(&value[..n as usize]);
    if d.CtlType == ODT_COMBOBOX {
        line_text(
            dc,
            app.font,
            &title,
            x + 12,
            y,
            width - 24,
            height,
            TEXT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        return;
    }
    let index = d.itemID as usize;
    let (title, detail, tag) = match d.CtlID {
        200 => app
            .instances
            .get(index)
            .map(|i| {
                (
                    i.name.clone(),
                    format!("Minecraft {}  /  {}", i.minecraft, i.group),
                    match i.engine {
                        Engine::Native => "RUSTED",
                        Engine::Java => "JAVA",
                        Engine::Fabric => "FABRIC",
                    },
                )
            })
            .unwrap_or((title.clone(), String::new(), "")),
        350 => app
            .hits
            .get(index)
            .map(|h| (h.name.clone(), h.description.clone(), "FABRIC"))
            .unwrap_or((title.clone(), String::new(), "")),
        330 => (
            title.clone(),
            if title.ends_with(".disabled") {
                "Fichier desactive".into()
            } else if title.ends_with(".hmod") {
                "Mod natif : options dans le jeu".into()
            } else {
                "Mod Fabric installe".into()
            },
            if title.ends_with(".disabled") {
                "INACTIF"
            } else {
                "INSTALLE"
            },
        ),
        500 => app
            .accounts
            .get(index)
            .map(|a| {
                (
                    a.name.clone(),
                    "Compte Minecraft Java Edition".into(),
                    "MICROSOFT",
                )
            })
            .unwrap_or((title.clone(), String::new(), "")),
        _ => (title, String::new(), ""),
    };
    line_text(
        dc,
        app.bold,
        &title,
        x + 16,
        y + 7,
        width - 150,
        24,
        TEXT,
        DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
    line_text(
        dc,
        app.small,
        &detail,
        x + 16,
        y + 32,
        width - 32,
        height - 32,
        rgb(165, 157, 143),
        DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
    if d.CtlID == 350 {
        if let Some(hit) = app.hits.get(index) {
            line_text(
                dc,
                app.small,
                &format!("{} telechargements", hit.downloads),
                x + 16,
                y + 56,
                width - 32,
                18,
                rgb(148, 131, 111),
                DT_LEFT | DT_SINGLELINE,
            );
        }
    }
    line_text(
        dc,
        app.small,
        tag,
        x + width - 134,
        y + 10,
        116,
        20,
        rgb(210, 143, 98),
        DT_RIGHT | DT_SINGLELINE,
    );
}

unsafe fn draw_button(app: &App, d: &DRAWITEMSTRUCT) {
    let id = d.CtlID;
    let active = ((100..=106).contains(&id) && app.page == id - 100)
        || (id == 320 && app.mod_tab == 0)
        || (id == 321 && app.mod_tab == 1);
    let primary = id == 1 || id == 213 || id == 351 || id == 501;
    let pressed = d.itemState & ODS_SELECTED != 0;
    let r = d.rcItem;
    let w = r.right - r.left;
    let h = r.bottom - r.top;
    let base = if primary || active {
        rgb(165, 91, 59)
    } else {
        rgb(54, 61, 64)
    };
    fill(d.hDC, r, if pressed { rgb(121, 67, 44) } else { base });
    rectangle(
        d.hDC,
        0,
        0,
        w,
        2,
        if primary || active {
            rgb(221, 153, 105)
        } else {
            rgb(87, 95, 96)
        },
    );
    rectangle(d.hDC, 0, h - 4, w, 4, rgb(25, 29, 31));
    if active {
        rectangle(d.hDC, 0, 0, 4, h, rgb(239, 184, 124));
    }
    let s = text(app, id);
    line_text(
        d.hDC,
        if primary { app.bold } else { app.font },
        &s,
        10,
        if pressed { 2 } else { 0 },
        w - 20,
        h - 3,
        rgb(248, 236, 214),
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}
unsafe fn pixel(dc: HDC, text: &str, x: i32, y: i32, scale: i32, color: u32) {
    let mut px = x;
    for c in text.chars() {
        let glyph = match c {
            'A' => [14, 17, 17, 31, 17, 17, 17],
            'C' => [15, 16, 16, 16, 16, 16, 15],
            'D' => [30, 17, 17, 17, 17, 17, 30],
            'E' => [31, 16, 16, 30, 16, 16, 31],
            'H' => [17, 17, 17, 31, 17, 17, 17],
            'L' => [16, 16, 16, 16, 16, 16, 31],
            'N' => [17, 25, 25, 21, 19, 19, 17],
            'R' => [30, 17, 17, 30, 20, 18, 17],
            'S' => [15, 16, 16, 14, 1, 1, 30],
            'T' => [31, 4, 4, 4, 4, 4, 4],
            'U' => [17, 17, 17, 17, 17, 17, 14],
            _ => [0; 7],
        };
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    rectangle(
                        dc,
                        px + col * scale,
                        y + row as i32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        px += 6 * scale;
    }
}
unsafe fn paint(app: &App) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let target = BeginPaint(app.window, &mut ps);
    let mut r: RECT = std::mem::zeroed();
    GetClientRect(app.window, &mut r);
    let dc = CreateCompatibleDC(target);
    let bitmap = CreateCompatibleBitmap(target, r.right.max(1), r.bottom.max(1));
    let old = SelectObject(dc, bitmap);
    fill(dc, r, BG);
    rectangle(dc, 0, 0, 248, r.bottom, rgb(20, 23, 25));
    rectangle(dc, 247, 0, 1, r.bottom, rgb(65, 48, 38));
    rectangle(dc, 268, 98, r.right - 296, 1, rgb(66, 54, 44));
    rectangle(dc, 248, r.bottom - 108, r.right - 248, 108, rgb(24, 28, 30));
    DrawIconEx(
        dc,
        26,
        26,
        LoadIconW(GetModuleHandleW(std::ptr::null()), 1usize as _),
        42,
        42,
        0,
        std::ptr::null_mut(),
        DI_NORMAL,
    );
    pixel(dc, "RUSTED", 84, 29, 3, rgb(221, 218, 208));
    pixel(dc, "LAUNCHER", 86, 56, 1, rgb(193, 123, 80));
    line_text(
        dc,
        app.small,
        "MINECRAFT, VERSION CUIVRE",
        27,
        106,
        210,
        24,
        rgb(150, 132, 112),
        DT_LEFT,
    );
    line_text(
        dc,
        app.small,
        "INSTANCE ACTIVE",
        284,
        25,
        350,
        24,
        rgb(163, 148, 127),
        DT_LEFT,
    );
    line_text(
        dc,
        app.small,
        "COMPTE MINECRAFT",
        820,
        25,
        320,
        24,
        rgb(163, 148, 127),
        DT_LEFT,
    );
    line_text(
        dc,
        app.small,
        "Rusted Launcher 0.2.1",
        27,
        r.bottom - 90,
        205,
        24,
        rgb(163, 148, 127),
        DT_LEFT,
    );
    line_text(
        dc,
        app.small,
        "Moteur natif 0.3.5",
        27,
        r.bottom - 65,
        205,
        24,
        rgb(113, 119, 119),
        DT_LEFT,
    );
    if app.page == 0 {
        let x = 284;
        let y = 150;
        let w = r.right - 330;
        rectangle(dc, x, y, w, 227, rgb(58, 46, 39));
        for col in 0..(w / 8) {
            let height = 32 + ((col * 17 + col * col * 3) % 63);
            let xx = x + col * 8;
            rectangle(
                dc,
                xx,
                y + 227 - height,
                8,
                height,
                rgb(74 + (col % 4) as u32 * 4, 55 + (col % 5) as u32 * 3, 42),
            );
            for row in 0..height / 8 {
                if (col + row * 13) % 5 == 0 {
                    rectangle(dc, xx, y + 227 - height + row * 8, 8, 8, rgb(119, 73, 48));
                }
            }
        }
        for i in 0..12 {
            let xx = x + w - 260 + i * 16;
            let top = y + 120 + ((i * 19) % 42);
            rectangle(
                dc,
                xx,
                top,
                16,
                227 - (top - y),
                rgb(122 + (i % 3) as u32 * 13, 68 + (i % 4) as u32 * 8, 43),
            );
            rectangle(dc, xx, top, 16, 4, rgb(197, 132, 82));
        }
        rectangle(dc, x, y, w, 2, rgb(116, 82, 58));
        pixel(dc, "RUSTED", 308, 179, 7, rgb(28, 23, 20));
        pixel(dc, "RUSTED", 306, 174, 7, rgb(229, 224, 208));
        pixel(dc, "LAUNCHER", 310, 238, 3, rgb(216, 143, 93));
    }
    if app.progress.1 > 0 && app.job.is_some() {
        let width = r.right - 248;
        let done = (app.progress.0 as f64 / app.progress.1 as f64).clamp(0.0, 1.0);
        rectangle(
            dc,
            248,
            r.bottom - 6,
            (width as f64 * done) as i32,
            6,
            rgb(206, 126, 76),
        );
    }
    BitBlt(target, 0, 0, r.right, r.bottom, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old);
    DeleteObject(bitmap);
    DeleteDC(dc);
    EndPaint(app.window, &ps);
}
