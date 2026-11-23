//! A thread-bound host for Windows Rich Edit. Positions are UTF-16 document offsets.
use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::num::NonZeroIsize;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::rc::Rc;

use windows::UI::Text::{RichEditMathMode, RichEditTextDocument};
use windows::Win32::Foundation::{
    COLORREF, E_FAIL, E_INVALIDARG, FreeLibrary, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT,
    RPC_E_CHANGED_MODE, WPARAM,
};
use windows::Win32::Globalization::{
    CORRECTIVE_ACTION_DELETE, CORRECTIVE_ACTION_GET_SUGGESTIONS, CORRECTIVE_ACTION_REPLACE,
    ISpellChecker, ISpellCheckerFactory, LCIDToLocaleName, SpellCheckerFactory,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_OVER, AlphaBlend, BLENDFUNCTION, COLOR_HIGHLIGHT, ClientToScreen, CombineRgn,
    CreateCompatibleBitmap, CreateCompatibleDC, CreateFontIndirectW, CreatePen, CreateRectRgn,
    CreateRoundRectRgn, CreateSolidBrush, DT_NOPREFIX, DT_WORDBREAK, DeleteDC, DeleteObject,
    DrawTextW, FillRect, GetDC, GetSysColor, HDC, HGDIOBJ, InvalidateRect, LOGFONTW, LineTo,
    MoveToEx, PS_SOLID, RGN_DIFF, RGN_ERROR, ReleaseDC, RestoreDC, SaveDC, ScreenToClient,
    SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{
    CLSID_AccPropServices, IAccPropServices, PROPID_ACC_HELP, PROPID_ACC_NAME,
};
use windows::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, FR_DOWN, GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER,
    OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::Controls::{
    EM_CANUNDO, EM_CHARFROMPOS, EM_EMPTYUNDOBUFFER, EM_GETFIRSTVISIBLELINE, EM_GETLINECOUNT,
    EM_GETRECT, EM_LINEINDEX, EM_POSFROMCHAR, EM_REPLACESEL, EM_SCROLLCARET, EM_SETREADONLY,
    EM_SETRECT, EM_UNDO,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, SetFocus, VK_APPS, VK_ESCAPE, VK_F10, VK_MENU, VK_SHIFT,
    VK_TAB,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CHILDID_SELF, CreatePopupMenu, CreateWindowExW, DestroyMenu, DestroyWindow,
    EN_CHANGE, ES_AUTOVSCROLL, ES_MULTILINE, ES_NOHIDESEL, ES_WANTRETURN, GetClientRect,
    GetWindowRect, GetWindowThreadProcessId, IsWindow, MF_CHECKED, MF_GRAYED, MF_STRING,
    OBJID_CLIENT, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW,
    SetWindowPos, ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CHAR, WM_CLEAR, WM_COMMAND, WM_CONTEXTMENU, WM_COPY, WM_CUT,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_PASTE, WM_RBUTTONDOWN, WM_SETTEXT, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_VSCROLL,
    WS_CHILD, WS_CLIPSIBLINGS, WS_TABSTOP, WS_VSCROLL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HideCaret, ShowCaret, WM_GETTEXTLENGTH, WM_IME_ENDCOMPOSITION, WM_IME_STARTCOMPOSITION,
    WM_KILLFOCUS, WM_PAINT, WM_PRINTCLIENT, WM_SETFOCUS,
};
use windows::core::{BSTR, Error, GUID, IUnknown, Interface, PCWSTR, PWSTR, Result, w};

const UTF16: u32 = 1200;
// Rich Edit's documented GetText2/SetText2 format flags are absent from tom.h.
const TOM_CONVERT_MATH_ML: i32 = 0x0001_0000;
const TOM_GET_UTF16: i32 = 0x0002_0000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    pub start: i32,
    pub end: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Bold,
    Italic,
    Underline,
}

impl Effect {
    fn mask(self) -> CFM_MASK {
        match self {
            Self::Bold => CFM_BOLD,
            Self::Italic => CFM_ITALIC,
            Self::Underline => CFM_UNDERLINE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edit {
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
    Undo,
    Redo,
    Toggle(Effect),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Placement {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub padding_x: i32,
    pub padding_y: i32,
}

pub struct SpellingMenu {
    range: Selection,
    original: Vec<u16>,
    locale: Vec<u16>,
    pub actions: Vec<SpellingAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpellingAction {
    Replace(String),
    DeleteRepeated,
    AddToDictionary,
    IgnoreAll,
    IgnoreOnce,
}

struct IgnoredWord {
    locale: Vec<u16>,
    word: Vec<u16>,
}

struct IgnoredOccurrence {
    range: ITextRange2,
    original: Vec<u16>,
}

#[derive(Clone, Copy, Debug)]
pub enum EditorRequest {
    ContextMenu { x: i32, y: i32 },
    SelectionMenu { x: i32, y: i32 },
    PointerMoved { x: i32, y: i32 },
    DismissMenu { escape: bool },
    Traverse { backward: bool },
}

#[derive(Clone, Copy, PartialEq)]
struct MenuClip {
    editor_size: [i32; 2],
    cutouts: [Option<[i32; 4]>; 2],
    diameter: i32,
}

struct EditorEvents {
    window: HWND,
    parent: HWND,
    changed: Cell<bool>,
    request: Cell<Option<EditorRequest>>,
    selection_menu_enabled: Cell<bool>,
    menu_open: Cell<bool>,
    track_pointer: Cell<bool>,
    placeholder: RefCell<Placeholder>,
    composing: Cell<bool>,
    search_matches: RefCell<Vec<Selection>>,
    spelling_marks: RefCell<Vec<Selection>>,
}

struct Placeholder {
    text: Vec<u16>,
    ink: COLORREF,
    family: [u16; 32],
    logical_size: f32,
    scale: f32,
}

impl Placeholder {
    fn new(scale: f32) -> Self {
        Self {
            text: Vec::new(),
            ink: color_ref([96, 96, 96]),
            family: [0; 32],
            logical_size: 14.0,
            scale,
        }
    }

    fn draw(&mut self, dc: HDC, mut bounds: RECT) {
        if dc.is_invalid() || bounds.left >= bounds.right || bounds.top >= bounds.bottom {
            return;
        }
        let font = LOGFONTW {
            lfHeight: -(self.logical_size * self.scale).round().max(1.0) as i32,
            lfWeight: 400,
            lfFaceName: self.family,
            ..Default::default()
        };
        // Preserve the DC state and release the font after deselecting it.
        unsafe {
            let saved = SaveDC(dc);
            if saved == 0 {
                return;
            }
            let font = CreateFontIndirectW(&font);
            if !font.is_invalid() {
                SelectObject(dc, HGDIOBJ(font.0));
            }
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, self.ink);
            DrawTextW(dc, &mut self.text, &mut bounds, DT_NOPREFIX | DT_WORDBREAK);
            let _ = RestoreDC(dc, saved);
            if !font.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(font.0));
            }
        }
    }
}

impl EditorEvents {
    fn attach(window: HWND, parent: HWND) -> Result<Box<Self>> {
        let events = Box::new(Self {
            window,
            parent,
            changed: Cell::new(true),
            request: Cell::new(None),
            selection_menu_enabled: Cell::new(false),
            menu_open: Cell::new(false),
            track_pointer: Cell::new(false),
            placeholder: RefCell::new(Placeholder::new(
                unsafe { GetDpiForWindow(window) } as f32 / 96.0,
            )),
            composing: Cell::new(false),
            search_matches: RefCell::default(),
            spelling_marks: RefCell::default(),
        });
        let key = (&*events as *const Self) as usize;
        // The boxed state stays put and both subclass hooks are removed before it is freed.
        unsafe {
            SetWindowSubclass(window, Some(editor_event), key, key).ok()?;
            if let Err(error) = SetWindowSubclass(parent, Some(parent_event), key, key).ok() {
                let _ = RemoveWindowSubclass(window, Some(editor_event), key);
                return Err(error);
            }
        }
        Ok(events)
    }

    fn invalidate_content(&self) {
        let _ = unsafe { InvalidateRect(Some(self.window), None, false) };
    }

    fn clear_matches(&self) {
        if let Ok(mut matches) = self.search_matches.try_borrow_mut()
            && !matches.is_empty()
        {
            matches.clear();
            self.invalidate_content();
        }
    }

    fn draws_placeholder(&self) -> bool {
        if self.composing.get() || self.placeholder.borrow().text.is_empty() {
            return false;
        }
        (unsafe { SendMessageW(self.window, WM_GETTEXTLENGTH, None, None) }).0 == 0
    }

    fn paint_decorations(&self, dc: HDC, placeholder: bool, highlight: bool) {
        if dc.is_invalid() {
            return;
        }
        let mut bounds = RECT::default();
        // Fetch the current native formatting rect, including changes to margins and scrolling.
        unsafe {
            SendMessageW(
                self.window,
                EM_GETRECT,
                None,
                Some(LPARAM(&mut bounds as *mut RECT as isize)),
            );
        }
        if placeholder && let Ok(mut cue) = self.placeholder.try_borrow_mut() {
            cue.draw(dc, bounds);
        }
        if highlight && let Ok(matches) = self.search_matches.try_borrow() {
            tint_matches(dc, &visible_range_bounds(self.window, bounds, &matches));
        }
        if let Ok(marks) = self.spelling_marks.try_borrow()
            && !marks.is_empty()
        {
            underline_spelling(dc, &visible_range_bounds(self.window, bounds, &marks));
        }
    }
}

impl Drop for EditorEvents {
    fn drop(&mut self) {
        let key = self as *mut Self as usize;
        unsafe {
            let _ = RemoveWindowSubclass(self.parent, Some(parent_event), key);
            let _ = RemoveWindowSubclass(self.window, Some(editor_event), key);
        }
    }
}

unsafe extern "system" fn parent_event(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    let events = unsafe { &*(data as *const EditorEvents) };
    if msg == WM_COMMAND
        && lparam.0 == events.window.0 as isize
        && (wparam.0 >> 16) as u16 == EN_CHANGE as u16
    {
        events.changed.set(true);
        events.clear_matches();
    }
    if msg == WM_LBUTTONDOWN && unsafe { GetFocus() } == events.window {
        let _ = unsafe { SetFocus(Some(hwnd)) };
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn editor_event(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    let events = unsafe { &*(data as *const EditorEvents) };
    if msg == WM_SETFOCUS {
        events.clear_matches();
    }
    if msg == WM_IME_STARTCOMPOSITION || msg == WM_IME_ENDCOMPOSITION || msg == WM_KILLFOCUS {
        let composing = msg == WM_IME_STARTCOMPOSITION;
        if events.composing.replace(composing) != composing {
            events.invalidate_content();
        }
    }
    if msg == WM_PAINT || msg == WM_PRINTCLIENT {
        let placeholder = events.draws_placeholder();
        let highlight = !events.search_matches.borrow().is_empty();
        if !placeholder && !highlight && events.spelling_marks.borrow().is_empty() {
            return unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        }
        if msg == WM_PRINTCLIENT {
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            events.paint_decorations(HDC(wparam.0 as *mut _), placeholder, highlight);
            return result;
        }
        // Draw over fresh native pixels to avoid tint accumulating after partial repaints.
        events.invalidate_content();
        // The native caret is drawn above the hint. Balance HideCaret only when it succeeds.
        let hidden = unsafe { HideCaret(Some(hwnd)) }.is_ok();
        let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        let dc = unsafe { GetDC(Some(hwnd)) };
        if !dc.is_invalid() {
            events.paint_decorations(dc, placeholder, highlight);
            unsafe { ReleaseDC(Some(hwnd), dc) };
        }
        if hidden {
            let _ = unsafe { ShowCaret(Some(hwnd)) };
        }
        return result;
    }
    if msg == WM_CONTEXTMENU {
        let x = lparam.0 as i16 as i32;
        let y = (lparam.0 >> 16) as i16 as i32;
        events
            .request
            .set(Some(EditorRequest::ContextMenu { x, y }));
        return LRESULT(0);
    }
    if matches!(msg, WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP)
        && (wparam.0 == usize::from(VK_APPS.0)
            || (wparam.0 == usize::from(VK_F10.0)
                && unsafe { GetKeyState(i32::from(VK_SHIFT.0)) < 0 }
                && unsafe { GetKeyState(i32::from(VK_MENU.0)) >= 0 }))
    {
        if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN) {
            events
                .request
                .set(Some(EditorRequest::ContextMenu { x: -1, y: -1 }));
        }
        return LRESULT(0);
    }
    if events.menu_open.get() {
        if wparam.0 == VK_ESCAPE.0 as usize && matches!(msg, WM_KEYDOWN | WM_CHAR) {
            if msg == WM_KEYDOWN {
                events
                    .request
                    .set(Some(EditorRequest::DismissMenu { escape: true }));
            }
            return LRESULT(0);
        }
        if matches!(
            msg,
            WM_LBUTTONDOWN
                | WM_LBUTTONDBLCLK
                | WM_RBUTTONDOWN
                | WM_MOUSEWHEEL
                | WM_VSCROLL
                | WM_KEYDOWN
        ) {
            events
                .request
                .set(Some(EditorRequest::DismissMenu { escape: false }));
        }
        if msg == WM_MOUSEMOVE
            && events.track_pointer.get()
            && matches!(
                events.request.get(),
                None | Some(EditorRequest::PointerMoved { .. })
            )
        {
            events.request.set(Some(EditorRequest::PointerMoved {
                x: lparam.0 as i16 as i32,
                y: (lparam.0 >> 16) as i16 as i32,
            }));
        }
    }
    if wparam.0 == VK_TAB.0 as usize {
        if msg == WM_KEYDOWN {
            let backward = unsafe { GetKeyState(i32::from(VK_SHIFT.0)) } < 0;
            events
                .request
                .set(Some(EditorRequest::Traverse { backward }));
            return LRESULT(0);
        }
        if msg == WM_CHAR {
            return LRESULT(0);
        }
    }
    let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
    if msg == WM_LBUTTONUP && events.selection_menu_enabled.get() {
        let mut range = CHARRANGE::default();
        unsafe {
            SendMessageW(
                hwnd,
                EM_EXGETSEL,
                None,
                Some(LPARAM(&mut range as *mut _ as isize)),
            );
        }
        if range.cpMin != range.cpMax {
            events.request.set(Some(EditorRequest::SelectionMenu {
                x: lparam.0 as i16 as i32,
                y: (lparam.0 >> 16) as i16 as i32,
            }));
        }
    }
    result
}

struct RichEditLibrary(HMODULE);

impl Drop for RichEditLibrary {
    fn drop(&mut self) {
        // Each successful load belongs to this instance.
        let _ = unsafe { FreeLibrary(self.0) };
    }
}

pub struct RichEdit {
    window: HWND,
    read_only: bool,
    spelling_enabled: bool,
    spelling_locale: Option<Vec<u16>>,
    ignored_words: Vec<IgnoredWord>,
    ignored_occurrences: RefCell<Vec<IgnoredOccurrence>>,
    math_mode: Cell<bool>,
    events: Box<EditorEvents>,
    menu_clip: Cell<Option<MenuClip>>,
    _library: RichEditLibrary,
    _thread: PhantomData<Rc<()>>,
}

impl RichEdit {
    pub fn new(parent: NonZeroIsize) -> Result<Self> {
        let parent = HWND(parent.get() as *mut _);
        // Every message and HWND lifetime operation must stay on the parent's UI thread.
        if unsafe { GetWindowThreadProcessId(parent, None) } != unsafe { GetCurrentThreadId() } {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        let library = RichEditLibrary(unsafe {
            LoadLibraryExW(w!("msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)?
        });
        let styles = WS_CHILD
            | WS_CLIPSIBLINGS
            | WS_TABSTOP
            | WS_VSCROLL
            | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | ES_NOHIDESEL) as u32);
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                MSFTEDIT_CLASS,
                PCWSTR::null(),
                styles,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                None,
                None,
            )?
        };
        let events = match EditorEvents::attach(window, parent) {
            Ok(events) => events,
            Err(error) => {
                let _ = unsafe { DestroyWindow(window) };
                return Err(error);
            }
        };
        let editor = Self {
            window,
            read_only: false,
            spelling_enabled: false,
            spelling_locale: None,
            ignored_words: Vec::new(),
            ignored_occurrences: RefCell::default(),
            math_mode: Cell::new(false),
            events,
            menu_clip: Cell::new(None),
            _library: library,
            _thread: PhantomData,
        };
        editor.message(
            EM_SETEDITSTYLE,
            (SES_EXTENDBACKCOLOR | SES_USECTF) as usize,
            (SES_EXTENDBACKCOLOR | SES_USECTF) as isize,
        );
        editor.message(EM_EXLIMITTEXT, 0, i32::MAX as isize);
        editor.message(EM_SETEVENTMASK, 0, ENM_CHANGE as isize);
        Ok(editor)
    }

    fn message(&self, code: u32, wparam: usize, lparam: isize) -> isize {
        // This call is synchronous, including when the parameters borrow stack data.
        unsafe {
            SendMessageW(
                self.window,
                code,
                Some(WPARAM(wparam)),
                Some(LPARAM(lparam)),
            )
            .0
        }
    }

    pub fn place(&self, placement: Placement, visible: bool) -> Result<()> {
        if !visible || placement.width <= 0 || placement.height <= 0 {
            let _ = unsafe { ShowWindow(self.window, SW_HIDE) };
            return Ok(());
        }
        let mut content = RECT::default();
        unsafe {
            SetWindowPos(
                self.window,
                None,
                placement.left,
                placement.top,
                placement.width,
                placement.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )?;
            GetClientRect(self.window, &mut content)?;
        }
        content.left += placement.padding_x.min((content.right - 1).max(0) / 2);
        content.right -= content.left;
        content.top += placement.padding_y.min((content.bottom - 1).max(0) / 2);
        content.bottom -= content.top;
        self.message(EM_SETRECT, 0, &content as *const RECT as isize);
        let _ = unsafe { ShowWindow(self.window, SW_SHOWNOACTIVATE) };
        Ok(())
    }

    pub fn focus(&self) {
        // GetFocus returns NULL when another thread owns the active window.
        if !self.has_focus() {
            let _ = unsafe { SetFocus(Some(self.window)) };
        }
    }

    pub fn has_focus(&self) -> bool {
        unsafe { GetFocus() == self.window }
    }

    pub fn yield_focus(&self) {
        let _ = unsafe { SetFocus(Some(self.events.parent)) };
    }

    pub fn take_request(&self) -> Option<EditorRequest> {
        self.events.request.take()
    }

    pub fn enable_selection_menu(&self) {
        self.events.selection_menu_enabled.set(true);
    }

    pub fn pointer_in_parent(&self, x: i32, y: i32) -> Result<[i32; 2]> {
        let mut point = POINT { x, y };
        unsafe { ClientToScreen(self.window, &mut point).ok()? };
        self.screen_to_parent(point)
    }

    pub fn context_menu_anchor(&self, x: i32, y: i32) -> Result<[i32; 2]> {
        self.screen_to_parent(self.context_screen_point(x, y)?)
    }

    pub fn set_spelling_enabled(&mut self, enabled: bool) {
        self.spelling_enabled = enabled;
        self.sync_native_spelling();
        self.events.changed.set(true);
    }

    fn sync_native_spelling(&self) {
        let enabled = self.spelling_enabled && !self.draws_spelling_marks();
        self.message(
            EM_SETEDITSTYLE,
            if enabled {
                SES_CTFALLOWPROOFING as usize
            } else {
                0
            },
            SES_CTFALLOWPROOFING as isize,
        );
        let flags = self.message(EM_GETLANGOPTIONS, 0, 0) & !(IMF_SPELLCHECKING as isize);
        let spell = if enabled {
            IMF_SPELLCHECKING as isize
        } else {
            0
        };
        self.message(EM_SETLANGOPTIONS, 0, flags | spell);
    }

    fn draws_spelling_marks(&self) -> bool {
        self.spelling_locale.is_some()
            || !self.ignored_words.is_empty()
            || !self.ignored_occurrences.borrow().is_empty()
    }

    pub fn set_default_language(&self, lcid: u32) {
        let mut format = char_format();
        format.Base.dwMask = CFM_LCID;
        format.lcid = lcid;
        self.apply_format(SCF_DEFAULT, &format);
    }

    pub fn set_spelling_locale(&mut self, language: &str) -> Result<bool> {
        let locale = terminated(language)?;
        if with_spell_checker(PCWSTR(locale.as_ptr()), |_| Ok(()))?.is_none() {
            return Ok(false);
        }
        self.spelling_locale = Some(locale);
        self.sync_native_spelling();
        self.events.changed.set(true);
        Ok(true)
    }

    /// Refresh the marks when this editor owns the dictionary or has session ignores.
    pub fn refresh_spelling_marks(&self) -> Result<()> {
        self.retain_ignored_occurrences(|ignored| unsafe {
            ignored
                .range
                .GetText2(0)
                .is_ok_and(|text| &*text == ignored.original.as_slice())
        });
        let next = if self.spelling_enabled
            && !self.read_only
            && !self.math_mode.get()
            && self.draws_spelling_marks()
        {
            self.document_spelling_marks()?
        } else {
            Vec::new()
        };
        let mut current = self.events.spelling_marks.borrow_mut();
        if *current != next {
            *current = next;
            drop(current);
            self.events.invalidate_content();
        }
        Ok(())
    }

    fn document_spelling_marks(&self) -> Result<Vec<Selection>> {
        let document = text_document(self.window).ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let end = self.text_length() as i32;
        let mut marks = Vec::new();
        if let Some(locale) = &self.spelling_locale {
            self.append_spelling_marks(&document, Selection { start: 0, end }, locale, &mut marks)?;
        } else {
            for (span, language) in spelling_language_runs(&document, end)? {
                if let Some(locale) = proofing_locale(language) {
                    self.append_spelling_marks(&document, span, &locale, &mut marks)?;
                }
            }
        }
        marks.retain(|span| !self.ignores_occurrence(*span));
        Ok(marks)
    }

    fn append_spelling_marks(
        &self,
        document: &ITextDocument2,
        span: Selection,
        locale: &[u16],
        marks: &mut Vec<Selection>,
    ) -> Result<()> {
        if span.start == span.end {
            return Ok(());
        }
        let range = unsafe { document.Range2(span.end, span.start)? };
        let text = unsafe { range.GetText2(0)? };
        let ranges =
            self.with_spelling_dictionary(locale, |checker| spelling_ranges(checker, &text))?;
        marks.extend(ranges.into_iter().flatten().map(|issue| Selection {
            start: issue.start + span.start,
            end: issue.end + span.start,
        }));
        Ok(())
    }

    fn with_spelling_dictionary<T>(
        &self,
        locale: &[u16],
        run: impl FnOnce(&ISpellChecker) -> Result<T>,
    ) -> Result<Option<T>> {
        with_spell_checker(PCWSTR(locale.as_ptr()), |checker| {
            for ignored in self
                .ignored_words
                .iter()
                .filter(|ignored| ignored.locale == locale)
            {
                unsafe { checker.Ignore(PCWSTR(ignored.word.as_ptr()))? };
            }
            run(checker)
        })
    }

    fn ignores_occurrence(&self, span: Selection) -> bool {
        self.ignored_occurrences
            .borrow()
            .iter()
            .any(|ignored| unsafe {
                ignored.range.GetStart().ok() == Some(span.start)
                    && ignored.range.GetEnd().ok() == Some(span.end)
                    && ignored
                        .range
                        .GetText2(0)
                        .is_ok_and(|text| &*text == ignored.original.as_slice())
            })
    }

    fn retain_ignored_occurrences(&self, keep: impl FnMut(&IgnoredOccurrence) -> bool) {
        let mut occurrences = self.ignored_occurrences.borrow_mut();
        let previously_ignored = !occurrences.is_empty();
        occurrences.retain(keep);
        let enable_native = previously_ignored && occurrences.is_empty();
        drop(occurrences);
        if enable_native {
            self.sync_native_spelling();
        }
    }

    pub fn spelling_at(&self, x: i32, y: i32) -> Result<Option<SpellingMenu>> {
        if !self.spelling_enabled || self.read_only || self.math_mode.get() || !self.has_text() {
            return Ok(None);
        }
        let point = if (x, y) == (-1, -1) {
            None
        } else {
            let mut point = POINT { x, y };
            unsafe { ScreenToClient(self.window, &mut point).ok()? };
            Some(point)
        };
        let position = point.map_or_else(
            || self.selection().start,
            |point| self.message(EM_CHARFROMPOS, 0, &point as *const POINT as isize) as i32,
        );
        let document = text_document(self.window).ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let hit = unsafe { document.Range2(position, position)? };
        let locale = if let Some(locale) = self.spelling_locale.as_deref() {
            locale.to_vec()
        } else {
            let language = unsafe { hit.GetFont2()?.GetLanguageID()? };
            let Some(locale) = proofing_locale(language) else {
                return Ok(None);
            };
            locale
        };
        unsafe { hit.Expand(tomParagraph.0)? };
        let start = unsafe { hit.GetStart()? };
        let paragraph = unsafe { hit.GetText2(0)? };
        let menu = self
            .with_spelling_dictionary(&locale, |checker| {
                spelling_menu(checker, &paragraph, start, position, &locale)
            })?
            .flatten();
        let menu = menu.filter(|menu| !self.ignores_occurrence(menu.range));
        if let (Some(point), Some(menu)) = (point, &menu) {
            let word = unsafe { document.Range2(menu.range.end, menu.range.start)? };
            let mut bounds = RECT::default();
            unsafe {
                word.GetRect(
                    tomClientCoord.0,
                    &mut bounds.left,
                    &mut bounds.top,
                    &mut bounds.right,
                    &mut bounds.bottom,
                    &mut 0,
                )?;
            }
            if !(bounds.left..bounds.right).contains(&point.x)
                || !(bounds.top..bounds.bottom).contains(&point.y)
            {
                return Ok(None);
            }
        }
        Ok(menu)
    }

    pub fn apply_spelling(&mut self, menu: &SpellingMenu, index: usize) -> Result<bool> {
        self.apply_spelling_using(menu, index, |locale, word| {
            with_spell_checker(PCWSTR(locale.as_ptr()), |checker| unsafe {
                checker.Add(PCWSTR(word.as_ptr()))
            })
            .map(|result| result.is_some())
        })
    }

    fn apply_spelling_using(
        &mut self,
        menu: &SpellingMenu,
        index: usize,
        add: impl FnOnce(&[u16], &[u16]) -> Result<bool>,
    ) -> Result<bool> {
        let Some(action) = menu.actions.get(index) else {
            return Ok(false);
        };
        if !self.spelling_enabled || self.read_only || self.math_mode.get() {
            return Ok(false);
        }
        let document = text_document(self.window).ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let word = unsafe { document.Range2(menu.range.end, menu.range.start)? };
        if &*unsafe { word.GetText2(0)? } != menu.original.as_slice() {
            return Ok(false);
        }
        match action {
            SpellingAction::Replace(replacement) => {
                self.set_selection(menu.range);
                self.replace_selection(replacement)?;
            }
            SpellingAction::DeleteRepeated => {
                self.set_selection(menu.range);
                self.replace_selection("")?;
            }
            SpellingAction::AddToDictionary => {
                let word: Vec<u16> = menu.original.iter().copied().chain([0]).collect();
                if !add(&menu.locale, &word)? {
                    return Ok(false);
                }
                if !self.draws_spelling_marks() {
                    let flags = self.message(EM_GETLANGOPTIONS, 0, 0);
                    self.message(EM_SETLANGOPTIONS, 0, flags & !(IMF_SPELLCHECKING as isize));
                    self.message(EM_SETLANGOPTIONS, 0, flags);
                }
                self.events.changed.set(true);
            }
            SpellingAction::IgnoreAll => {
                let word: Vec<u16> = menu.original.iter().copied().chain([0]).collect();
                let accepted =
                    with_spell_checker(PCWSTR(menu.locale.as_ptr()), |checker| unsafe {
                        checker.Ignore(PCWSTR(word.as_ptr()))
                    })?;
                if accepted.is_none() {
                    return Ok(false);
                }
                self.ignored_words.push(IgnoredWord {
                    locale: menu.locale.clone(),
                    word,
                });
                self.sync_native_spelling();
                self.events.changed.set(true);
            }
            SpellingAction::IgnoreOnce => {
                self.ignored_occurrences.get_mut().push(IgnoredOccurrence {
                    range: word,
                    original: menu.original.clone(),
                });
                self.sync_native_spelling();
                self.events.changed.set(true);
            }
        }
        Ok(true)
    }

    fn screen_to_parent(&self, mut point: POINT) -> Result<[i32; 2]> {
        unsafe { ScreenToClient(self.events.parent, &mut point).ok()? };
        Ok([point.x, point.y])
    }

    pub fn selection_menu_anchor(&self, x: i32, y: i32) -> Result<[i32; 3]> {
        let Selection { start, end } = self.selection();
        let mut bounds = RECT::default();
        let mut hit = 0;
        if let Some(document) = text_document(self.window)
            && let Ok(range) = unsafe { document.Range2(end, start) }
            && unsafe {
                range.GetRect(
                    tomClientCoord.0 | tomAllowOffClient.0,
                    &mut bounds.left,
                    &mut bounds.top,
                    &mut bounds.right,
                    &mut bounds.bottom,
                    &mut hit,
                )
            }
            .is_ok()
        {
            let mut viewport = RECT::default();
            unsafe { GetClientRect(self.window, &mut viewport)? };
            let top = bounds.top.max(viewport.top);
            let bottom = bounds.bottom.min(viewport.bottom);
            if top < bottom {
                let height = bottom - top;
                let [left, top] = self.pointer_in_parent(x, top)?;
                return Ok([left, top, height]);
            }
        }
        let [left, top] = self.pointer_in_parent(x, y)?;
        Ok([left, top, 0])
    }

    /// Keep the native editor live outside the area drawn by the parent's command bar.
    pub fn set_menu_overlay(
        &self,
        bounds: [Option<[i32; 4]>; 2],
        diameter: i32,
        track_pointer: bool,
    ) -> Result<()> {
        let open = bounds.iter().any(Option::is_some);
        self.events.menu_open.set(open);
        self.events.track_pointer.set(track_pointer);
        let mut clip = None;
        if open {
            let mut window = RECT::default();
            unsafe { GetWindowRect(self.window, &mut window)? };
            let [x, y] = self.screen_to_parent(POINT {
                x: window.left,
                y: window.top,
            })?;
            let width = window.right - window.left;
            let height = window.bottom - window.top;
            let cutouts = bounds.map(|panel| {
                let [left, top, right, bottom] = panel?;
                (left < right
                    && top < bottom
                    && left < x + width
                    && right > x
                    && top < y + height
                    && bottom > y)
                    .then_some([left - x, top - y, right - x, bottom - y])
            });
            if cutouts.iter().any(Option::is_some) {
                clip = Some(MenuClip {
                    editor_size: [width, height],
                    cutouts,
                    diameter,
                });
            }
        }
        if clip == self.menu_clip.get() {
            return Ok(());
        }
        unsafe {
            if let Some(clip) = clip {
                let [width, height] = clip.editor_size;
                let region = CreateRectRgn(0, 0, width, height);
                if region.is_invalid() {
                    return Err(Error::from_hresult(E_FAIL));
                }
                for [left, top, right, bottom] in clip.cutouts.into_iter().flatten() {
                    let cutout =
                        CreateRoundRectRgn(left, top, right + 1, bottom + 1, diameter, diameter);
                    let combined = !cutout.is_invalid()
                        && CombineRgn(Some(region), Some(region), Some(cutout), RGN_DIFF)
                            != RGN_ERROR;
                    if !cutout.is_invalid() {
                        let _ = DeleteObject(HGDIOBJ(cutout.0));
                    }
                    if !combined {
                        let _ = DeleteObject(HGDIOBJ(region.0));
                        return Err(Error::from_hresult(E_FAIL));
                    }
                }
                // A successful SetWindowRgn transfers ownership to the system.
                if SetWindowRgn(self.window, Some(region), true) == 0 {
                    let _ = DeleteObject(HGDIOBJ(region.0));
                    return Err(Error::from_hresult(E_FAIL));
                }
            } else if SetWindowRgn(self.window, None, true) == 0 {
                return Err(Error::from_hresult(E_FAIL));
            }
        }
        self.menu_clip.set(clip);
        Ok(())
    }

    pub fn take_text_changed(&self) -> bool {
        self.events.changed.replace(false)
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = unsafe { EnableWindow(self.window, enabled) };
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.message(EM_SETREADONLY, usize::from(read_only), 0);
        self.read_only = read_only;
        if self.draws_spelling_marks() {
            self.events.changed.set(true);
        }
    }

    /// Like RichEditBox, changing math mode clears the document and its undo history.
    pub fn set_math_mode(&self, enabled: bool) -> Result<()> {
        if self.math_mode.get() == enabled {
            return Ok(());
        }
        let document = text_document(self.window).ok_or_else(|| Error::from_hresult(E_FAIL))?;
        document
            .cast::<RichEditTextDocument>()?
            .SetMathMode(if enabled {
                RichEditMathMode::MathOnly
            } else {
                RichEditMathMode::NoMath
            })?;
        self.math_mode.set(enabled);
        self.set_text("")
    }

    /// Export a single displayed line of math as MathML.
    pub fn math_ml(&self) -> Result<String> {
        if !self.math_fits_one_line() {
            return Ok(String::new());
        }
        self.math_ml_for_story()
    }

    pub fn math_fits_one_line(&self) -> bool {
        self.message(EM_GETLINECOUNT, 0, 0) <= 1
    }

    fn math_ml_for_story(&self) -> Result<String> {
        let document = text_document(self.window).ok_or_else(|| Error::from_hresult(E_FAIL))?;
        let content = unsafe { document.Range2(self.text_length() as i32, 0)? };
        let xml = unsafe { content.GetText2(TOM_CONVERT_MATH_ML | TOM_GET_UTF16)? };
        Ok(String::from_utf16_lossy(&xml))
    }

    /// Replace a math document after Rich Edit successfully parses the supplied MathML.
    pub fn set_math_ml(&self, math_ml: &str) -> io::Result<()> {
        let staged = self.new_hidden_sibling()?;
        staged.set_math_mode(true).map_err(io::Error::other)?;

        let mut format = char_format();
        self.message(
            EM_GETCHARFORMAT,
            SCF_DEFAULT as usize,
            &mut format as *mut CHARFORMAT2W as isize,
        );
        staged.apply_format(SCF_DEFAULT, &format);
        let document = text_document(staged.window).ok_or(io::ErrorKind::Unsupported)?;
        let range = unsafe { document.Range2(0, 0) }.map_err(io::Error::other)?;
        unsafe { range.SetText2(TOM_CONVERT_MATH_ML, &BSTR::from(math_ml)) }
            .map_err(io::Error::other)?;
        if staged
            .math_ml_for_story()
            .map_err(io::Error::other)?
            .is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected a MathML equation",
            ));
        }
        self.replace_document_rtf(&staged.export_rtf()?)
    }

    pub fn set_background(&self, rgb: [u8; 3]) {
        self.message(EM_SETBKGNDCOLOR, 0, color_ref(rgb).0 as isize);
    }

    pub fn set_scale(&self, scale: f32) -> Result<()> {
        let dpi = unsafe { GetDpiForWindow(self.window) };
        if !scale.is_finite() || scale <= 0.0 || dpi == 0 {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        // Rich Edit already scales by monitor DPI, while a host may override its logical scale.
        let zoom = (scale * 96.0 * 1024.0 / dpi as f32)
            .round()
            .clamp(17.0, 65535.0) as usize;
        self.message(EM_SETZOOM, zoom, 1024);
        self.events.placeholder.borrow_mut().scale = scale;
        self.events.invalidate_content();
        Ok(())
    }

    pub fn set_accessible_name(&self, name: &str) -> Result<()> {
        self.set_accessible_string(PROPID_ACC_NAME, &terminated(name)?)
    }

    pub fn set_placeholder(&self, text: &str) -> Result<()> {
        let mut wide = terminated(text)?;
        let result = self.set_accessible_string(PROPID_ACC_HELP, &wide);
        wide.pop();
        self.events.placeholder.borrow_mut().text = wide;
        self.events.invalidate_content();
        result
    }

    pub fn set_placeholder_color(&self, rgb: [u8; 3]) {
        self.events.placeholder.borrow_mut().ink = color_ref(rgb);
        self.events.invalidate_content();
    }

    fn set_accessible_string(&self, property: GUID, value: &[u16]) -> Result<()> {
        let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if init != RPC_E_CHANGED_MODE {
            init.ok()?;
        }
        let result = (|| unsafe {
            let service: IAccPropServices =
                CoCreateInstance(&CLSID_AccPropServices, None, CLSCTX_INPROC_SERVER)?;
            service.SetHwndPropStr(
                self.window,
                OBJID_CLIENT.0 as u32,
                CHILDID_SELF,
                property,
                PCWSTR(value.as_ptr()),
            )
        })();
        if init.is_ok() {
            unsafe { CoUninitialize() };
        }
        result
    }

    pub fn set_default_font(&self, family: &str, logical_size: f32) {
        let mut format = char_format();
        format.Base.dwMask = CFM_FACE | CFM_SIZE;
        format.Base.yHeight = (logical_size * 15.0).round().max(15.0) as i32;
        let limit = format.Base.szFaceName.len() - 1;
        let mut offset = 0;
        for ch in family.chars() {
            if offset + ch.len_utf16() > limit || ch == '\0' {
                break;
            }
            ch.encode_utf16(&mut format.Base.szFaceName[offset..]);
            offset += ch.len_utf16();
        }
        self.apply_format(SCF_DEFAULT, &format);
        let mut placeholder = self.events.placeholder.borrow_mut();
        placeholder.family = format.Base.szFaceName;
        placeholder.logical_size = format.Base.yHeight as f32 / 15.0;
        drop(placeholder);
        self.events.invalidate_content();
    }

    pub fn text(&self) -> String {
        let length = self.text_length();
        let mut text = vec![0u16; length + 1];
        let request = GETTEXTEX {
            cb: (text.len() * 2) as u32,
            codepage: UTF16,
            ..Default::default()
        };
        let count = self.message(
            EM_GETTEXTEX,
            &request as *const GETTEXTEX as usize,
            text.as_mut_ptr() as isize,
        );
        decode_text(&text, count)
    }

    fn text_length(&self) -> usize {
        let request = GETTEXTLENGTHEX {
            flags: GTL_PRECISE | GTL_NUMCHARS,
            codepage: UTF16,
        };
        self.message(
            EM_GETTEXTLENGTHEX,
            &request as *const GETTEXTLENGTHEX as usize,
            0,
        )
        .max(0) as usize
    }

    pub fn has_text(&self) -> bool {
        self.text_length() != 0
    }

    pub fn set_text(&self, text: &str) -> Result<()> {
        let wide = terminated(text)?;
        if self.message(WM_SETTEXT, 0, wide.as_ptr() as isize) == 0 {
            return Err(Error::from_hresult(E_FAIL));
        }
        self.message(EM_EMPTYUNDOBUFFER, 0, 0);
        self.retain_ignored_occurrences(|_| false);
        self.events.changed.set(true);
        self.events.clear_matches();
        Ok(())
    }

    pub fn replace_selection(&self, text: &str) -> Result<()> {
        if self.read_only {
            return Ok(());
        }
        let wide = terminated(text)?;
        self.message(EM_STOPGROUPTYPING, 0, 0);
        self.message(EM_REPLACESEL, 1, wide.as_ptr() as isize);
        Ok(())
    }

    pub fn selection(&self) -> Selection {
        let mut range = CHARRANGE::default();
        self.message(EM_EXGETSEL, 0, &mut range as *mut CHARRANGE as isize);
        Selection {
            start: range.cpMin,
            end: range.cpMax,
        }
    }

    pub fn set_selection(&self, range: Selection) {
        let range = CHARRANGE {
            cpMin: range.start,
            cpMax: range.end,
        };
        self.message(EM_EXSETSEL, 0, &range as *const CHARRANGE as isize);
    }

    pub fn selected_text(&self) -> String {
        let range = self.selection();
        let mut text = vec![0u16; (range.end - range.start).max(0) as usize + 1];
        let count = self.message(EM_GETSELTEXT, 0, text.as_mut_ptr() as isize);
        decode_text(&text, count)
    }

    pub fn can_undo(&self) -> bool {
        !self.read_only && self.message(EM_CANUNDO, 0, 0) != 0
    }

    pub fn can_redo(&self) -> bool {
        !self.read_only && self.message(EM_CANREDO, 0, 0) != 0
    }

    pub fn can_paste(&self) -> bool {
        !self.read_only && self.message(EM_CANPASTE, 0, 0) != 0
    }

    fn context_screen_point(&self, x: i32, y: i32) -> Result<POINT> {
        let mut point = POINT { x, y };
        if x == -1 && y == -1 {
            point = POINT::default();
            self.message(
                EM_POSFROMCHAR,
                &mut point as *mut POINT as usize,
                self.selection().end as isize,
            );
            point.y += 20;
            unsafe { ClientToScreen(self.window, &mut point).ok()? };
        }
        Ok(point)
    }

    pub fn show_context_menu(&self, x: i32, y: i32) -> Result<()> {
        let point = self.context_screen_point(x, y)?;
        let selection = self.selection();
        let has_selection = selection.start != selection.end;
        let actions = [
            (
                Edit::Cut,
                w!("Cut\tCtrl+X"),
                has_selection && !self.read_only,
                false,
            ),
            (Edit::Copy, w!("Copy\tCtrl+C"), has_selection, false),
            (Edit::Paste, w!("Paste\tCtrl+V"), self.can_paste(), false),
            (Edit::Undo, w!("Undo\tCtrl+Z"), self.can_undo(), false),
            (Edit::Redo, w!("Redo\tCtrl+Y"), self.can_redo(), false),
            (
                Edit::SelectAll,
                w!("Select All\tCtrl+A"),
                self.text_length() > 0,
                false,
            ),
            (
                Edit::Toggle(Effect::Bold),
                w!("Bold\tCtrl+B"),
                !self.read_only,
                self.effect_active(Effect::Bold),
            ),
            (
                Edit::Toggle(Effect::Italic),
                w!("Italic\tCtrl+I"),
                !self.read_only,
                self.effect_active(Effect::Italic),
            ),
            (
                Edit::Toggle(Effect::Underline),
                w!("Underline\tCtrl+U"),
                !self.read_only,
                self.effect_active(Effect::Underline),
            ),
        ];
        let menu = unsafe { CreatePopupMenu()? };
        let outcome = (|| unsafe {
            for (index, (_, label, enabled, checked)) in actions.iter().enumerate() {
                let mut flags = MF_STRING;
                if !enabled {
                    flags |= MF_GRAYED;
                }
                if *checked {
                    flags |= MF_CHECKED;
                }
                AppendMenuW(menu, flags, index + 1, *label)?;
            }
            let index = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
                point.x,
                point.y,
                None,
                self.window,
                None,
            )
            .0 as usize;
            if let Some((command, _, true, _)) = index.checked_sub(1).and_then(|i| actions.get(i)) {
                self.execute(*command);
            }
            Ok(())
        })();
        let _ = unsafe { DestroyMenu(menu) };
        outcome
    }

    pub fn pick_rtf_file(&self, save: bool) -> io::Result<Option<PathBuf>> {
        let mut path = vec![0u16; 32768];
        if save {
            for (slot, unit) in path.iter_mut().zip("New Document.rtf".encode_utf16()) {
                *slot = unit;
            }
        }
        let mut dialog = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: self.events.parent,
            lpstrFilter: w!("Rich Text Format (*.rtf)\0*.rtf\0"),
            lpstrFile: PWSTR(path.as_mut_ptr()),
            nMaxFile: path.len() as u32,
            lpstrDefExt: w!("rtf"),
            Flags: OFN_EXPLORER
                | OFN_NOCHANGEDIR
                | OFN_PATHMUSTEXIST
                | if save {
                    OFN_OVERWRITEPROMPT
                } else {
                    OFN_FILEMUSTEXIST
                },
            ..Default::default()
        };
        let accepted = unsafe {
            if save {
                GetSaveFileNameW(&mut dialog)
            } else {
                GetOpenFileNameW(&mut dialog)
            }
        };
        if accepted.as_bool() {
            let length = path
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(path.len());
            return Ok(Some(PathBuf::from(OsString::from_wide(&path[..length]))));
        }
        let error = unsafe { CommDlgExtendedError() }.0;
        if error == 0 {
            Ok(None)
        } else {
            Err(io::Error::other(format!(
                "Windows file dialog failed ({error:#x})"
            )))
        }
    }

    pub fn execute(&self, edit: Edit) {
        let message = match edit {
            Edit::Copy => WM_COPY,
            Edit::SelectAll => {
                self.set_selection(Selection { start: 0, end: -1 });
                return;
            }
            _ if self.read_only => return,
            Edit::Toggle(effect) => {
                let mut format = char_format();
                format.Base.dwMask = effect.mask();
                if !self.effect_active(effect) {
                    format.Base.dwEffects = CFE_EFFECTS(effect.mask().0);
                }
                self.apply_format(SCF_SELECTION, &format);
                return;
            }
            Edit::Cut => WM_CUT,
            Edit::Paste => WM_PASTE,
            Edit::Delete => WM_CLEAR,
            Edit::Undo => EM_UNDO,
            Edit::Redo => EM_REDO,
        };
        self.message(message, 0, 0);
    }

    pub fn effect_active(&self, effect: Effect) -> bool {
        let format = self.selection_format();
        format.Base.dwMask.contains(effect.mask()) && format.Base.dwEffects.0 & effect.mask().0 != 0
    }

    pub fn selection_color(&self) -> Option<[u8; 3]> {
        let format = self.selection_format();
        if !format.Base.dwMask.contains(CFM_COLOR) || format.Base.dwEffects.contains(CFE_AUTOCOLOR)
        {
            return None;
        }
        let [red, green, blue, _] = format.Base.crTextColor.0.to_le_bytes();
        Some([red, green, blue])
    }

    pub fn set_selection_color(&self, rgb: Option<[u8; 3]>) {
        if !self.read_only {
            let mut format = char_format();
            format.Base.dwMask = CFM_COLOR;
            if let Some(rgb) = rgb {
                format.Base.crTextColor = color_ref(rgb);
            } else {
                format.Base.dwEffects = CFE_AUTOCOLOR;
            }
            self.apply_format(SCF_SELECTION, &format);
        }
    }

    pub fn find_from(&self, needle: &str, start: i32) -> Option<Selection> {
        if needle.is_empty() {
            return None;
        }
        let needle = terminated(needle).ok()?;
        self.find_wide(&needle, start)
    }

    fn find_wide(&self, needle: &[u16], start: i32) -> Option<Selection> {
        let mut query = FINDTEXTEXW {
            chrg: CHARRANGE {
                cpMin: start.max(0),
                cpMax: -1,
            },
            lpstrText: PCWSTR(needle.as_ptr()),
            ..Default::default()
        };
        let result = self.message(
            EM_FINDTEXTEXW,
            FR_DOWN.0 as usize,
            &mut query as *mut FINDTEXTEXW as isize,
        );
        (result >= 0).then_some(Selection {
            start: query.chrgText.cpMin,
            end: query.chrgText.cpMax,
        })
    }

    pub fn reveal_selection(&self) {
        self.message(EM_SCROLLCARET, 0, 0);
    }

    /// Highlight non-overlapping search matches without editing the document or its undo stack.
    pub fn highlight_matches(&self, needle: &str) -> Result<usize> {
        let needle = terminated(needle)?;
        let mut matches = Vec::new();
        if needle.len() > 1 {
            let mut next = 0;
            while let Some(selection) = self.find_wide(&needle, next) {
                if selection.end <= next {
                    break;
                }
                matches.push(selection);
                next = selection.end;
            }
        }
        let count = matches.len();
        if *self.events.search_matches.borrow() != matches {
            *self.events.search_matches.borrow_mut() = matches;
            self.events.invalidate_content();
        }
        Ok(count)
    }

    fn selection_format(&self) -> CHARFORMAT2W {
        let mut format = char_format();
        self.message(
            EM_GETCHARFORMAT,
            SCF_SELECTION as usize,
            &mut format as *mut CHARFORMAT2W as isize,
        );
        format
    }

    fn apply_format(&self, scope: u32, format: &CHARFORMAT2W) {
        self.message(
            EM_SETCHARFORMAT,
            scope as usize,
            format as *const CHARFORMAT2W as isize,
        );
    }

    /// Load a local RTF document using the operating system's reader.
    pub fn import_rtf(&self, bytes: &[u8]) -> io::Result<()> {
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        if !bytes.starts_with(b"{\\rtf") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected an RTF document",
            ));
        }
        // Native stream-in can report an error after partially replacing a document.
        // Parse with an invisible sibling before changing the active document and its history.
        self.new_hidden_sibling()?.stream_in(bytes)?;
        self.replace_document_rtf(bytes)
    }

    fn new_hidden_sibling(&self) -> io::Result<Self> {
        let parent =
            NonZeroIsize::new(self.events.parent.0 as isize).ok_or(io::ErrorKind::InvalidInput)?;
        Self::new(parent).map_err(io::Error::other)
    }

    fn replace_document_rtf(&self, bytes: &[u8]) -> io::Result<()> {
        let backup = self.export_rtf()?;
        let selection = self.selection();
        if let Err(error) = self.stream_in(bytes) {
            self.stream_in(&backup)?;
            self.set_selection(selection);
            return Err(error);
        }
        self.message(EM_EMPTYUNDOBUFFER, 0, 0);
        self.retain_ignored_occurrences(|_| false);
        self.events.changed.set(true);
        self.events.clear_matches();
        Ok(())
    }

    fn stream_in(&self, mut input: &[u8]) -> io::Result<()> {
        let mut stream = EDITSTREAM {
            dwCookie: &mut input as *mut &[u8] as usize,
            pfnCallback: Some(read_rtf),
            ..Default::default()
        };
        self.message(
            EM_STREAMIN,
            SF_RTF as usize,
            &mut stream as *mut EDITSTREAM as isize,
        );
        let error = stream.dwError;
        if error != 0 {
            return Err(io::Error::other(format!(
                "Rich Edit could not read RTF ({error:#x})"
            )));
        }
        Ok(())
    }

    pub fn export_rtf(&self) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut stream = EDITSTREAM {
            dwCookie: &mut bytes as *mut Vec<u8> as usize,
            pfnCallback: Some(write_rtf),
            ..Default::default()
        };
        self.message(
            EM_STREAMOUT,
            SF_RTF as usize,
            &mut stream as *mut EDITSTREAM as isize,
        );
        let error = stream.dwError;
        if error != 0 {
            return Err(io::Error::other(format!(
                "Rich Edit could not write RTF ({error:#x})"
            )));
        }
        Ok(bytes)
    }
}

impl Drop for RichEdit {
    fn drop(&mut self) {
        self.ignored_occurrences.get_mut().clear();
        // Closing the parent can destroy this child before its Rust handle is dropped.
        unsafe {
            if IsWindow(Some(self.window)).as_bool() {
                let _ = DestroyWindow(self.window);
            }
        }
    }
}

fn char_format() -> CHARFORMAT2W {
    let mut format = CHARFORMAT2W::default();
    format.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
    format
}

fn with_spell_checker<T>(
    locale: PCWSTR,
    run: impl FnOnce(&ISpellChecker) -> Result<T>,
) -> Result<Option<T>> {
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if init != RPC_E_CHANGED_MODE {
        init.ok()?;
    }
    let result = (|| unsafe {
        let factory: ISpellCheckerFactory =
            CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER)?;
        if !factory.IsSupported(locale)?.as_bool() {
            return Ok(None);
        }
        run(&factory.CreateSpellChecker(locale)?).map(Some)
    })();
    if init.is_ok() {
        unsafe { CoUninitialize() };
    }
    result
}

fn spelling_menu(
    checker: &ISpellChecker,
    paragraph: &BSTR,
    start: i32,
    position: i32,
    locale: &[u16],
) -> Result<Option<SpellingMenu>> {
    let errors = unsafe { checker.ComprehensiveCheck(PCWSTR(paragraph.as_ptr()))? };
    while let Some(error) = {
        let mut error = None;
        unsafe { errors.Next(&mut error).ok()? };
        error
    } {
        let offset = unsafe { error.StartIndex()? } as usize;
        let length = unsafe { error.Length()? } as usize;
        let Some(original) = paragraph.get(offset..offset.saturating_add(length)) else {
            continue;
        };
        let range = Selection {
            start: start + offset as i32,
            end: start + (offset + length) as i32,
        };
        if range.start == range.end || !(range.start..=range.end).contains(&position) {
            continue;
        }
        let mut actions = match unsafe { error.CorrectiveAction()? } {
            CORRECTIVE_ACTION_REPLACE => {
                vec![SpellingAction::Replace(read_spelling_string(unsafe {
                    error.Replacement()?
                })?)]
            }
            CORRECTIVE_ACTION_DELETE => {
                vec![SpellingAction::DeleteRepeated, SpellingAction::IgnoreOnce]
            }
            CORRECTIVE_ACTION_GET_SUGGESTIONS => {
                let word: Vec<u16> = original.iter().copied().chain([0]).collect();
                let suggestions = unsafe { checker.Suggest(PCWSTR(word.as_ptr()))? };
                let mut results = Vec::new();
                for _ in 0..3 {
                    let mut value = [PWSTR::null()];
                    let mut fetched = 0;
                    unsafe { suggestions.Next(&mut value, Some(&mut fetched)).ok()? };
                    if fetched == 0 {
                        break;
                    }
                    results.push(SpellingAction::Replace(read_spelling_string(value[0])?));
                }
                results
            }
            _ => continue,
        };
        if !actions.contains(&SpellingAction::DeleteRepeated) {
            actions.extend([SpellingAction::AddToDictionary, SpellingAction::IgnoreAll]);
        }
        return Ok(Some(SpellingMenu {
            range,
            original: original.to_vec(),
            locale: locale.to_vec(),
            actions,
        }));
    }
    Ok(None)
}

fn proofing_locale(language: i32) -> Option<Vec<u16>> {
    let mut name = [0u16; 85];
    let length = unsafe { LCIDToLocaleName(language as u32, Some(&mut name), 0) } as usize;
    (length > 0).then(|| name[..length].to_vec())
}

fn spelling_language_runs(document: &ITextDocument2, end: i32) -> Result<Vec<(Selection, i32)>> {
    let mut runs: Vec<(Selection, i32)> = Vec::new();
    let mut start = 0;
    while start < end {
        // Start on a character rather than an insertion point at a format boundary.
        let run = unsafe { document.Range2(start + 1, start)? };
        let language = unsafe { run.GetFont2()?.GetLanguageID()? };
        unsafe { run.EndOf(tomCharFormat.0, tomExtend.0)? };
        let stop = unsafe { run.GetEnd()? }.clamp(start + 1, end);
        if let Some((tail, previous_language)) = runs.last_mut()
            && *previous_language == language
        {
            tail.end = stop;
        } else {
            runs.push((Selection { start, end: stop }, language));
        }
        start = stop;
    }
    Ok(runs)
}

fn spelling_ranges(checker: &ISpellChecker, text: &BSTR) -> Result<Vec<Selection>> {
    let mut ranges = Vec::new();
    let errors = unsafe { checker.ComprehensiveCheck(PCWSTR(text.as_ptr()))? };
    while let Some(error) = {
        let mut error = None;
        unsafe { errors.Next(&mut error).ok()? };
        error
    } {
        let start = unsafe { error.StartIndex()? } as usize;
        let end = start.saturating_add(unsafe { error.Length()? } as usize);
        if start < end && end <= text.len() {
            ranges.push(Selection {
                start: start as i32,
                end: end as i32,
            });
        }
    }
    ranges.sort_unstable_by_key(|span| (span.start, span.end));
    Ok(ranges)
}

fn read_spelling_string(value: PWSTR) -> Result<String> {
    let string = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.as_ptr().cast())) };
    Ok(string?)
}

fn text_document(window: HWND) -> Option<ITextDocument2> {
    let mut pointer = std::ptr::null_mut::<std::ffi::c_void>();
    if unsafe {
        SendMessageW(
            window,
            EM_GETOLEINTERFACE,
            None,
            Some(LPARAM(&mut pointer as *mut _ as isize)),
        )
    }
    .0 == 0
        || pointer.is_null()
    {
        return None;
    }
    // EM_GETOLEINTERFACE returns an owned COM reference.
    unsafe { IUnknown::from_raw(pointer) }.cast().ok()
}

fn visible_range_bounds(window: HWND, viewport: RECT, matches: &[Selection]) -> Vec<RECT> {
    let Some(document) = text_document(window) else {
        return Vec::new();
    };
    let line_of = |offset| unsafe {
        SendMessageW(
            window,
            EM_EXLINEFROMCHAR,
            None,
            Some(LPARAM(offset as isize)),
        )
        .0
    };
    let line_start = |line| unsafe {
        SendMessageW(window, EM_LINEINDEX, Some(WPARAM(line as usize)), None).0 as i32
    };
    let probe = POINT {
        x: viewport.left,
        y: viewport.bottom - 1,
    };
    let last_visible = unsafe {
        SendMessageW(
            window,
            EM_CHARFROMPOS,
            None,
            Some(LPARAM(&probe as *const _ as isize)),
        )
        .0
    };
    let first = unsafe { SendMessageW(window, EM_GETFIRSTVISIBLELINE, None, None) }.0;
    let last = line_of(last_visible as i32).max(first);
    let mut rectangles = Vec::new();
    let first_offset = line_start(first);
    let first_match = matches.partition_point(|range| range.end <= first_offset);
    for range in &matches[first_match..] {
        let past_start = line_of(range.start);
        if past_start > last {
            break;
        }
        let through_end = line_of(range.end - 1);
        for line in past_start.max(first)..=through_end.min(last) {
            let start = if line == past_start {
                range.start
            } else {
                line_start(line)
            };
            let end = if line == through_end {
                range.end
            } else {
                line_start(line + 1)
            };
            if start < 0 || end <= start {
                continue;
            }
            let Ok(slice) = (unsafe { document.Range2(end, start) }) else {
                continue;
            };
            let mut bounds = RECT::default();
            let mut hit = 0;
            if unsafe {
                slice.GetRect(
                    tomClientCoord.0 | tomAllowOffClient.0,
                    &mut bounds.left,
                    &mut bounds.top,
                    &mut bounds.right,
                    &mut bounds.bottom,
                    &mut hit,
                )
            }
            .is_err()
            {
                continue;
            }
            bounds.left = bounds.left.max(viewport.left);
            bounds.right = bounds.right.min(viewport.right);
            bounds.top = bounds.top.max(viewport.top);
            bounds.bottom = bounds.bottom.min(viewport.bottom);
            if bounds.right > bounds.left && bounds.bottom > bounds.top {
                rectangles.push(bounds);
            }
        }
    }
    rectangles
}

fn underline_spelling(dc: HDC, rectangles: &[RECT]) {
    if rectangles.is_empty() {
        return;
    }
    unsafe {
        let pen = CreatePen(PS_SOLID, 1, color_ref([0xc4, 0x2b, 0x1c]));
        if pen.is_invalid() {
            return;
        }
        let old = SelectObject(dc, HGDIOBJ(pen.0));
        for rect in rectangles {
            let peak = (rect.bottom - 3).max(rect.top);
            let trough = rect.bottom - 1;
            let _ = MoveToEx(dc, rect.left, peak, None);
            for (index, x) in (rect.left + 2..rect.right).step_by(2).enumerate() {
                let _ = LineTo(dc, x, if index % 2 == 0 { trough } else { peak });
            }
        }
        SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

fn tint_matches(dc: HDC, rectangles: &[RECT]) {
    if rectangles.is_empty() {
        return;
    }
    unsafe {
        let source = CreateCompatibleDC(Some(dc));
        if source.is_invalid() {
            return;
        }
        let bitmap = CreateCompatibleBitmap(dc, 1, 1);
        let brush = CreateSolidBrush(COLORREF(GetSysColor(COLOR_HIGHLIGHT)));
        if !bitmap.is_invalid() && !brush.is_invalid() {
            let old = SelectObject(source, HGDIOBJ(bitmap.0));
            FillRect(
                source,
                &RECT {
                    left: 0,
                    top: 0,
                    right: 1,
                    bottom: 1,
                },
                brush,
            );
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                SourceConstantAlpha: 96,
                ..Default::default()
            };
            for rect in rectangles {
                let _ = AlphaBlend(
                    dc,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    source,
                    0,
                    0,
                    1,
                    1,
                    blend,
                );
            }
            SelectObject(source, old);
        }
        let _ = DeleteObject(HGDIOBJ(brush.0));
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(source);
    }
}

fn color_ref([r, g, b]: [u8; 3]) -> COLORREF {
    COLORREF(u32::from_le_bytes([r, g, b, 0]))
}

fn terminated(text: &str) -> Result<Vec<u16>> {
    if text.contains('\0') {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    Ok(text.encode_utf16().chain([0]).collect())
}

fn decode_text(text: &[u16], count: isize) -> String {
    let count = (count.max(0) as usize).min(text.len());
    String::from_utf16_lossy(&text[..count]).replace('\r', "\n")
}

unsafe extern "system" fn read_rtf(
    cookie: usize,
    buffer: *mut u8,
    length: i32,
    count: *mut i32,
) -> u32 {
    if length <= 0 {
        unsafe { *count = 0 };
        return u32::from(length < 0);
    }
    // EDITSTREAM callbacks run synchronously. The cookie is the caller's input slice
    // and Rich Edit supplies a writable buffer of length bytes.
    let input = unsafe { &mut *(cookie as *mut &[u8]) };
    let buffer = unsafe { std::slice::from_raw_parts_mut(buffer, length as usize) };
    match input.read(buffer) {
        Ok(bytes) => {
            unsafe { *count = bytes as i32 };
            0
        }
        Err(_) => 1,
    }
}

unsafe extern "system" fn write_rtf(
    cookie: usize,
    buffer: *mut u8,
    length: i32,
    count: *mut i32,
) -> u32 {
    if length <= 0 {
        unsafe { *count = 0 };
        return u32::from(length < 0);
    }
    // The caller owns the output vector until the synchronous stream finishes.
    let output = unsafe { &mut *(cookie as *mut Vec<u8>) };
    let buffer = unsafe { std::slice::from_raw_parts(buffer, length as usize) };
    match output.write(buffer) {
        Ok(bytes) => {
            unsafe { *count = bytes as i32 };
            0
        }
        Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{GetWindowRgn, PtInRegion};
    use windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPED;

    struct TestWindow(HWND);

    impl TestWindow {
        fn new() -> Self {
            Self(unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    PCWSTR::null(),
                    WS_OVERLAPPED,
                    0,
                    0,
                    300,
                    120,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap()
            })
        }

        fn editor(&self) -> RichEdit {
            RichEdit::new(NonZeroIsize::new(self.0.0 as isize).unwrap()).unwrap()
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }

    #[test]
    fn floating_menu_clips_only_its_overlap_and_preserves_document_selection() {
        let parent = TestWindow::new();
        let editor = parent.editor();
        editor
            .place(
                Placement {
                    left: 12,
                    top: 18,
                    width: 180,
                    height: 80,
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        editor.set_text("First second").unwrap();
        let selection = Selection { start: 0, end: 5 };
        editor.set_selection(selection);

        editor
            .set_menu_overlay([Some([32, 10, 72, 44]), Some([100, 25, 124, 52])], 16, true)
            .unwrap();
        let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
        assert!(!region.is_invalid());
        let (clipped, covered_text, covered_by_submenu, uncovered_text) = unsafe {
            let kind = GetWindowRgn(editor.window, region);
            let covered = PtInRegion(region, 30, 12).as_bool();
            let uncovered = PtInRegion(region, 10, 12).as_bool();
            (
                kind,
                covered,
                PtInRegion(region, 96, 20).as_bool(),
                uncovered,
            )
        };
        editor.set_menu_overlay([None, None], 16, false).unwrap();
        let unclipped = unsafe { GetWindowRgn(editor.window, region) };
        let _ = unsafe { DeleteObject(HGDIOBJ(region.0)) };

        assert_ne!(clipped, RGN_ERROR);
        assert!(!covered_text);
        assert!(!covered_by_submenu);
        assert!(uncovered_text);
        assert_eq!(unclipped, RGN_ERROR);
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.text(), "First second");
    }

    #[test]
    fn spelling_replacement_uses_the_native_range_and_rejects_a_stale_word() {
        let parent = TestWindow::new();
        let mut editor = parent.editor();
        let (wrong, expected) = if editor.set_spelling_locale("en-US").unwrap() {
            ("quik", "quick")
        } else if editor.set_spelling_locale("zh-Latn-CN-x-ext").unwrap() {
            ("beijng", "Beijing")
        } else {
            return;
        };
        editor.set_spelling_enabled(true);
        let initial = format!("😀 {wrong}.");
        editor.set_text(&initial).unwrap();
        editor.set_selection(Selection { start: 4, end: 4 });
        let menu = editor
            .spelling_at(-1, -1)
            .unwrap()
            .expect("native proofing");
        let index = menu
            .actions
            .iter()
            .position(|action| matches!(action, SpellingAction::Replace(word) if word == expected))
            .unwrap();
        assert!(editor.apply_spelling(&menu, index).unwrap());
        assert_eq!(editor.text(), format!("😀 {expected}."));
        assert!(!editor.apply_spelling(&menu, index).unwrap());
        editor.execute(Edit::Undo);
        assert_eq!(editor.text(), initial);
    }

    #[test]
    fn proofing_dictionary_and_ignore_actions_preserve_document_and_undo() {
        let parent = TestWindow::new();
        let mut editor = parent.editor();
        let wrong = if editor.set_spelling_locale("en-US").unwrap() {
            "quik"
        } else if editor.set_spelling_locale("zh-Latn-CN-x-ext").unwrap() {
            "beijng"
        } else {
            return;
        };
        editor.set_spelling_enabled(true);
        let initial = format!("{wrong}. {wrong}.");
        editor.set_text(&initial).unwrap();
        editor.set_selection(Selection { start: 2, end: 2 });
        let menu = editor.spelling_at(-1, -1).unwrap().unwrap();
        let add = menu
            .actions
            .iter()
            .position(|item| *item == SpellingAction::AddToDictionary)
            .unwrap();
        let ignore = menu
            .actions
            .iter()
            .position(|item| *item == SpellingAction::IgnoreAll)
            .unwrap();

        let editor_locale = editor.spelling_locale.clone();
        let check_add = |locale: &[u16], word: &[u16]| {
            assert_eq!(Some(locale), editor_locale.as_deref());
            assert_eq!(String::from_utf16_lossy(word), format!("{wrong}\0"));
            Ok(true)
        };
        assert!(editor.apply_spelling_using(&menu, add, check_add).unwrap());
        assert_eq!(editor.text(), initial);
        assert!(!editor.can_undo());
        assert!(editor.apply_spelling(&menu, ignore).unwrap());
        assert!(editor.spelling_at(-1, -1).unwrap().is_none());
        editor.set_selection(Selection {
            start: wrong.len() as i32 + 3,
            end: wrong.len() as i32 + 3,
        });
        assert!(editor.spelling_at(-1, -1).unwrap().is_none());
        editor.refresh_spelling_marks().unwrap();
        assert!(editor.events.spelling_marks.borrow().is_empty());
        assert_eq!(editor.text(), initial);
        assert!(!editor.can_undo());
    }

    #[test]
    fn ignoring_a_repetition_follows_the_occurrence_through_edits() {
        let parent = TestWindow::new();
        let mut editor = parent.editor();
        let word = if editor.set_spelling_locale("en-US").unwrap() {
            "fox"
        } else if editor.set_spelling_locale("zh-Latn-CN-x-ext").unwrap() {
            "Beijing"
        } else {
            return;
        };
        editor.set_spelling_enabled(true);
        editor
            .set_text(&format!("{word} {word}. {word} {word}."))
            .unwrap();
        let position = word.len() as i32 + 2;
        editor.set_selection(Selection {
            start: position,
            end: position,
        });
        let menu = editor.spelling_at(-1, -1).unwrap().unwrap();
        let ignore = menu
            .actions
            .iter()
            .position(|item| *item == SpellingAction::IgnoreOnce)
            .unwrap();
        let span = menu.range;
        assert!(editor.apply_spelling(&menu, ignore).unwrap());
        assert!(editor.spelling_at(-1, -1).unwrap().is_none());
        editor.set_selection(Selection { start: 0, end: 0 });
        editor.replace_selection("😀 ").unwrap();
        editor.refresh_spelling_marks().unwrap();
        assert!(editor.ignores_occurrence(Selection {
            start: span.start + 3,
            end: span.end + 3
        }));
        assert_eq!(editor.events.spelling_marks.borrow().len(), 1);
        editor.set_text("New document").unwrap();
        assert!(editor.ignored_occurrences.borrow().is_empty());
    }

    #[test]
    fn proofing_preserves_the_language_of_mixed_format_runs() {
        let parent = TestWindow::new();
        let editor = parent.editor();
        editor
            .import_rtf(br"{\rtf1\ansi\deflang1033 \lang1033 Quick \b fox\b0 . \lang2052 other}")
            .unwrap();
        let document = text_document(editor.window).unwrap();
        let end = editor.text_length() as i32;
        let first_chinese = editor.find_from("other", 0).unwrap().start;
        assert_eq!(
            spelling_language_runs(&document, end).unwrap(),
            vec![
                (
                    Selection {
                        start: 0,
                        end: first_chinese
                    },
                    0x0409
                ),
                (
                    Selection {
                        start: first_chinese,
                        end
                    },
                    0x0804
                )
            ]
        );
    }

    #[test]
    fn math_mode_builds_unicode_math_and_replaces_equations_from_math_ml() {
        let parent = TestWindow::new();
        let editor = parent.editor();
        editor.set_math_mode(true).unwrap();

        for key in "4^2 ".chars() {
            editor.message(WM_CHAR, key as usize, 0);
        }
        let power = editor.math_ml().unwrap();
        assert!(power.contains("<mml:msup>"), "{power}");

        editor
            .set_math_ml(
                r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mfrac><mi>x</mi><mi>y</mi></mfrac></math>"#,
            )
            .unwrap();
        let fraction = editor.math_ml().unwrap();
        assert!(fraction.contains("<mml:mfrac>"), "{fraction}");
        assert!(!fraction.contains("<mml:msup>"));
        let selection = Selection { start: 1, end: 3 };
        editor.set_selection(selection);
        assert!(editor.set_math_ml("Not MathML").is_err());
        assert_eq!(editor.math_ml().unwrap(), fraction);
        assert_eq!(editor.selection(), selection);

        editor.set_math_mode(false).unwrap();
        assert!(!editor.has_text());
        assert!(!editor.can_undo());
        assert!(editor.math_ml().unwrap().is_empty());
        for key in "4^2 ".chars() {
            editor.message(WM_CHAR, key as usize, 0);
        }
        assert_eq!(editor.text(), "4^2 ");
    }

    #[test]
    fn placeholder_follows_document_emptiness_without_becoming_document_content() {
        let parent = TestWindow::new();
        let editor = parent.editor();
        let before = editor.export_rtf().unwrap();
        editor.set_placeholder("Enter text & 输入").unwrap();
        assert!(editor.events.draws_placeholder());
        assert_eq!(editor.text(), "");
        assert_eq!(editor.export_rtf().unwrap(), before);
        assert!(!editor.can_undo());

        editor.replace_selection("a").unwrap();
        assert!(!editor.events.draws_placeholder());
        editor.execute(Edit::Undo);
        assert!(editor.events.draws_placeholder());
        editor.set_placeholder("").unwrap();
        assert!(!editor.events.draws_placeholder());
    }

    #[test]
    fn search_highlights_leave_rtf_selection_style_and_history_intact() {
        let parent = TestWindow::new();
        let editor = parent.editor();
        editor.set_text("one two 😀 one").unwrap();
        let selection = Selection { start: 4, end: 7 };
        editor.set_selection(selection);
        editor.set_selection_color(Some([0x48, 0x2b, 0x78]));
        let before = editor.export_rtf().unwrap();
        let undo = editor.can_undo();
        assert_eq!(editor.highlight_matches("ONE").unwrap(), 2);
        assert_eq!(before, editor.export_rtf().unwrap());
        assert_eq!(editor.highlight_matches("two").unwrap(), 1);
        assert_eq!(editor.highlight_matches("").unwrap(), 0);
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.selection_color(), Some([0x48, 0x2b, 0x78]));
        assert_eq!(editor.can_undo(), undo);
        assert_eq!(before, editor.export_rtf().unwrap());
    }

    #[test]
    fn formats_and_round_trips_unicode_and_line_breaks() {
        let parent = TestWindow::new();
        let editor = parent.editor();
        let text = "Hi 😀\n第二行";
        editor.set_text(text).unwrap();
        assert_eq!(editor.text(), text);
        editor.set_selection(Selection { start: 3, end: 5 });
        assert_eq!(editor.selected_text(), "😀");
        editor.execute(Edit::Toggle(Effect::Bold));
        editor.execute(Edit::Toggle(Effect::Italic));
        editor.set_selection_color(Some([0x31, 0x91, 0x74]));
        assert!(editor.effect_active(Effect::Bold));
        let rtf = editor.export_rtf().unwrap();

        let loaded = parent.editor();
        loaded.import_rtf(&rtf).unwrap();
        assert_eq!(loaded.text(), text);
        loaded.set_selection(Selection { start: 3, end: 5 });
        assert!(loaded.effect_active(Effect::Bold));
        assert!(loaded.effect_active(Effect::Italic));
        assert_eq!(loaded.selection_color(), Some([0x31, 0x91, 0x74]));
        assert!(!loaded.can_undo());
        loaded.set_selection_color(None);
        assert_eq!(loaded.selection_color(), None);
        assert!(loaded.import_rtf(b"not RTF").is_err());
        assert_eq!(loaded.text(), text);
    }

    #[test]
    fn undo_redo_search_and_read_only_share_the_document() {
        let parent = TestWindow::new();
        let mut editor = parent.editor();
        editor.set_text("one TWO one two").unwrap();
        let range = editor.find_from("two", 0).unwrap();
        assert_eq!(range, Selection { start: 4, end: 7 });
        editor.set_selection(range);
        editor.replace_selection("three").unwrap();
        assert_eq!(editor.text(), "one three one two");
        assert!(editor.can_undo());
        editor.execute(Edit::Undo);
        assert_eq!(editor.text(), "one TWO one two");
        assert!(editor.can_redo());
        editor.execute(Edit::Redo);
        assert_eq!(editor.text(), "one three one two");
        editor.set_read_only(true);
        editor.execute(Edit::SelectAll);
        editor.replace_selection("x").unwrap();
        editor.execute(Edit::Delete);
        editor.execute(Edit::Undo);
        assert_eq!(editor.selected_text(), "one three one two");
    }
}
