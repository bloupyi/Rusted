//! Modeless Microsoft code window: its own state never borrows the main App.
use super::{fill, line_text, rgb, wide, BG, DRAWITEMSTRUCT, TEXT};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

struct State {
    owner: HWND,
    font: HFONT,
    bold: HFONT,
    code_font: HFONT,
    code: String,
    owned_by_window: bool,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.code_font) };
    }
}

pub(super) unsafe fn show(
    owner: HWND,
    code: &str,
    font: HFONT,
    bold: HFONT,
) -> Result<HWND, String> {
    let class = wide("RustedMicrosoftCodePopup");
    let instance = GetModuleHandleW(std::ptr::null());
    let wc = WNDCLASSW {
        lpfnWndProc: Some(proc),
        hInstance: instance,
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        hIcon: LoadIconW(instance, 1usize as _),
        lpszClassName: class.as_ptr(),
        ..std::mem::zeroed()
    };
    if RegisterClassW(&wc) == 0 {
        let error = GetLastError();
        if error != ERROR_CLASS_ALREADY_EXISTS {
            return Err(format!(
                "Fenetre Microsoft indisponible (classe Windows : {error})."
            ));
        }
    }
    let code_font = CreateFontW(
        36,
        0,
        0,
        0,
        700,
        0,
        0,
        0,
        DEFAULT_CHARSET as _,
        OUT_DEFAULT_PRECIS as _,
        CLIP_DEFAULT_PRECIS as _,
        CLEARTYPE_QUALITY as _,
        DEFAULT_PITCH as _,
        wide("Consolas").as_ptr(),
    );
    let mut state = Box::new(State {
        owner,
        font,
        bold,
        code_font,
        code: code.into(),
        owned_by_window: false,
    });
    let mut owner_rect: RECT = std::mem::zeroed();
    GetWindowRect(owner, &mut owner_rect);
    let window = CreateWindowExW(
        WS_EX_DLGMODALFRAME,
        class.as_ptr(),
        wide("Connexion Microsoft - Rusted").as_ptr(),
        WS_CAPTION | WS_SYSMENU | WS_POPUP,
        owner_rect.left + (owner_rect.right - owner_rect.left - 520) / 2,
        owner_rect.top + (owner_rect.bottom - owner_rect.top - 360) / 2,
        520,
        360,
        owner,
        std::ptr::null_mut(),
        instance,
        (&mut *state as *mut State).cast(),
    );
    if window.is_null() {
        return Err(format!(
            "Fenetre Microsoft indisponible (creation Windows : {}).",
            GetLastError()
        ));
    }
    // Until creation succeeds, the caller owns State, even if Windows sends
    // WM_NCDESTROY during a failed CreateWindowExW. Transfer ownership only once.
    state.owned_by_window = true;
    let _ = Box::into_raw(state);
    let dark = 1i32;
    DwmSetWindowAttribute(window, 20, (&dark as *const i32).cast(), 4);
    ShowWindow(window, SW_SHOW);
    UpdateWindow(window);
    Ok(window)
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as _);
        return 1;
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_NCDESTROY {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        if (*ptr).owned_by_window {
            drop(Box::from_raw(ptr));
        }
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_ERASEBKGND {
        return 1;
    }
    if msg == WM_CTLCOLORSTATIC {
        let dc = w as HDC;
        SetTextColor(dc, TEXT);
        SetBkColor(dc, BG);
        SetDCBrushColor(dc, BG);
        return GetStockObject(DC_BRUSH) as _;
    }
    let state = &*ptr;
    match msg {
        WM_CREATE => {
            let label = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide(&state.code).as_ptr(),
                WS_CHILD | WS_VISIBLE | windows_sys::Win32::System::SystemServices::SS_CENTER,
                24,
                85,
                456,
                62,
                hwnd,
                6001usize as _,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            SendMessageW(label, WM_SETFONT, state.code_font as _, 0);
            for (id, title, x, y, width) in [
                (508usize, "Copier le code", 24, 166, 218),
                (507, "Ouvrir Microsoft", 258, 166, 222),
                (509, "Annuler la connexion", 24, 220, 456),
            ] {
                let control = CreateWindowExW(
                    0,
                    wide("BUTTON").as_ptr(),
                    wide(title).as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
                    x,
                    y,
                    width,
                    40,
                    hwnd,
                    id as _,
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                SendMessageW(control, WM_SETFONT, state.font as _, 0);
            }
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            fill(dc, rect, BG);
            line_text(
                dc,
                state.bold,
                "Connecte ton compte Microsoft",
                24,
                20,
                456,
                28,
                TEXT,
                DT_LEFT,
            );
            line_text(
                dc,
                state.font,
                "Colle ce code sur la page Microsoft :",
                24,
                53,
                456,
                24,
                TEXT,
                DT_LEFT,
            );
            line_text(dc, state.font, "Le code est copie automatiquement. La fenetre se fermera une fois le compte connecte.", 24, 276, 456, 42, rgb(185, 172, 151), DT_LEFT | DT_WORDBREAK);
            EndPaint(hwnd, &ps);
            0
        }
        WM_DRAWITEM => {
            let item = &*(l as *const DRAWITEMSTRUCT);
            fill(
                item.hDC,
                item.rcItem,
                if item.CtlID == 508 {
                    rgb(165, 91, 59)
                } else {
                    rgb(54, 61, 64)
                },
            );
            let mut text = [0u16; 128];
            let n = GetWindowTextW(item.hwndItem, text.as_mut_ptr(), 128);
            line_text(
                item.hDC,
                state.font,
                &String::from_utf16_lossy(&text[..n.max(0) as usize]),
                0,
                0,
                item.rcItem.right,
                item.rcItem.bottom,
                TEXT,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            1
        }
        WM_COMMAND => {
            let command = match w & 65535 {
                1 => 508,
                2 => 509,
                id @ 507..=509 => id,
                _ => return 0,
            };
            PostMessageW(state.owner, WM_COMMAND, command, 0);
            0
        }
        WM_CLOSE => {
            PostMessageW(state.owner, WM_COMMAND, 509, 0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
