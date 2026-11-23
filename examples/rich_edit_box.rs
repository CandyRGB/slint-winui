#[path = "../src/behavior/command_bar.rs"]
mod command_bar;
#[path = "../src/behavior/menu_flyout.rs"]
mod menu_flyout;

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::num::NonZeroIsize;
use std::rc::Rc;
use std::time::Duration;

use quick_xml::{
    Reader, Writer,
    events::{BytesText, Event},
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{Color, ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use slint_winui::platform::{
    accent::light_accent_text_colors,
    rich_edit::{Edit, EditorRequest, Effect, Placement, RichEdit, SpellingAction, SpellingMenu},
};

slint::slint! {
    import { RichEditBoxExample, RichEditCommand, RichEditGeometry, RichEditState, FlyoutRect, TextCommand,
        MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel,
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind,
        CommandBarLayout, CommandBarOverflowButtonVisibility }
        from "ui/rich_edit_box.slint";
    export { RichEditBoxExample, RichEditCommand, RichEditGeometry, RichEditState, FlyoutRect, TextCommand,
        MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel,
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind,
        CommandBarLayout, CommandBarOverflowButtonVisibility }
}

const SAMPLE_FORMULA: &str = r#"<math xmlns="http://www.w3.org/1998/Math/MathML" display="block">
  <mi>x</mi><mo>∈</mo><mi>P</mi><mfenced><mi>A</mi></mfenced>
  <mo>↔</mo><mi>x</mi><mo>⊆</mo><mi>A</mi>
</math>"#;

struct MountedEditor {
    editor: RichEdit,
    geometry: Option<RichEditGeometry>,
    scale: f32,
    has_text: bool,
    math_fits_one_line: bool,
    spelling_menu: Option<SpellingMenu>,
}

impl MountedEditor {
    fn new(editor: RichEdit) -> Self {
        editor.set_default_font("Segoe UI Variable Text", 14.0);
        editor.enable_selection_menu();
        Self {
            editor,
            geometry: None,
            scale: 0.0,
            has_text: false,
            math_fits_one_line: true,
            spelling_menu: None,
        }
    }

    fn update(
        &mut self,
        geometry: RichEditGeometry,
        scale: f32,
    ) -> Result<(RichEditState, bool), Box<dyn Error>> {
        let previous = self.geometry.as_ref();
        let geometry_changed = previous != Some(&geometry) || self.scale != scale;
        if geometry_changed {
            let px = |logical: f32| (logical * scale).round() as i32;
            if self.scale != scale {
                self.editor.set_scale(scale)?;
            }
            let placement_changed = previous.is_none_or(|old| {
                (old.x, old.y, old.width, old.height, old.visible)
                    != (
                        geometry.x,
                        geometry.y,
                        geometry.width,
                        geometry.height,
                        geometry.visible,
                    )
            });
            if placement_changed || self.scale != scale {
                self.editor.place(
                    Placement {
                        left: px(geometry.x),
                        top: px(geometry.y),
                        width: px(geometry.x + geometry.width) - px(geometry.x),
                        height: px(geometry.y + geometry.height) - px(geometry.y),
                        padding_x: px(8.0),
                        padding_y: px(4.0),
                    },
                    geometry.visible,
                )?;
            }
            if previous.is_none_or(|old| old.fill != geometry.fill) {
                self.editor.set_background(rgb(geometry.fill));
            }
            if previous.is_none_or(|old| old.placeholder != geometry.placeholder) {
                self.editor.set_placeholder(&geometry.placeholder)?;
            }
            if previous.is_none_or(|old| old.placeholder_ink != geometry.placeholder_ink) {
                self.editor
                    .set_placeholder_color(rgb(geometry.placeholder_ink));
            }
            if previous.is_none_or(|old| old.enabled != geometry.enabled) {
                self.editor.set_enabled(geometry.enabled);
            }
            if previous.is_none_or(|old| old.read_only != geometry.read_only) {
                self.editor.set_read_only(geometry.read_only);
            }
            self.geometry = Some(geometry);
            self.scale = scale;
        }
        let document_changed = self.editor.take_text_changed();
        if document_changed {
            self.has_text = self.editor.has_text();
            self.editor.refresh_spelling_marks()?;
        }
        let selection = self.editor.selection();
        let state = RichEditState {
            has_focus: self.editor.has_focus(),
            has_text: self.has_text,
            has_selection: selection.start != selection.end,
            can_undo: self.editor.can_undo(),
            can_redo: self.editor.can_redo(),
            bold: self.editor.effect_active(Effect::Bold),
            italic: self.editor.effect_active(Effect::Italic),
            underlined: self.editor.effect_active(Effect::Underline),
        };
        Ok((state, document_changed || geometry_changed))
    }
}

fn format_math_ml(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    loop {
        match reader.read_event().ok()? {
            Event::Eof => return String::from_utf8(writer.into_inner()).ok(),
            Event::GeneralRef(reference) => {
                // Keep references inline with the surrounding MathML token text.
                let text = match reference.resolve_char_ref().ok()? {
                    Some(character) => {
                        BytesText::new(character.encode_utf8(&mut [0; 4])).into_owned()
                    }
                    None => BytesText::from_escaped(format!("&{};", reference.decode().ok()?)),
                };
                writer.write_event(Event::Text(text)).ok()?;
            }
            event => writer.write_event(event).ok()?,
        }
    }
}

fn rgb(color: Color) -> [u8; 3] {
    [color.red(), color.green(), color.blue()]
}

fn command(editor: &RichEdit, command: RichEditCommand) {
    let action = match command {
        RichEditCommand::Focus => {
            editor.focus();
            return;
        }
        RichEditCommand::Cut => Edit::Cut,
        RichEditCommand::Copy => Edit::Copy,
        RichEditCommand::Paste => Edit::Paste,
        RichEditCommand::SelectAll => Edit::SelectAll,
        RichEditCommand::Undo => Edit::Undo,
        RichEditCommand::Redo => Edit::Redo,
        RichEditCommand::Bold => Edit::Toggle(Effect::Bold),
        RichEditCommand::Italic => Edit::Toggle(Effect::Italic),
        RichEditCommand::Underline => Edit::Toggle(Effect::Underline),
    };
    editor.execute(action);
    editor.focus();
}

fn open_file(editor: &RichEdit) -> io::Result<Option<String>> {
    let Some(path) = editor.pick_rtf_file(false)? else {
        return Ok(None);
    };
    let bytes = fs::read(&path)?;
    editor.import_rtf(&bytes)?;
    editor.focus();
    Ok(Some(format!("Opened {}", path.display())))
}

fn save_file(editor: &RichEdit) -> io::Result<Option<String>> {
    let Some(path) = editor.pick_rtf_file(true)? else {
        return Ok(None);
    };
    let output = editor.export_rtf()?;
    let directory = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    // Swap the completed file into place so a partial write preserves the existing document.
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(&output)?;
    temporary.as_file().sync_all()?;
    temporary.persist(&path).map_err(|error| error.error)?;
    editor.focus();
    Ok(Some(format!("Saved {}", path.display())))
}

fn show_status(ui: &RichEditBoxExample, result: io::Result<Option<String>>) {
    match result {
        Ok(Some(message)) => ui.set_status(message.into()),
        Err(error) => ui.set_status(error.to_string().into()),
        Ok(None) => {}
    }
}

fn tick(ui: &RichEditBoxExample, slot: &RefCell<MountedEditor>, simple: bool) {
    let Ok(mut host) = slot.try_borrow_mut() else {
        return;
    };
    let geometry = if simple {
        ui.get_simple_geometry()
    } else {
        ui.get_custom_geometry()
    };
    match host.update(geometry, ui.window().scale_factor()) {
        Ok((state, _)) if simple => {
            if ui.get_simple_state() != state {
                ui.set_simple_state(state);
            }
        }
        Ok((state, refresh_math)) => {
            if ui.get_custom_state() != state {
                ui.set_custom_state(state);
            }
            if ui.get_math_samples() {
                // Native line layout may finish after the text change notification.
                let fits_one_line = host.editor.math_fits_one_line();
                if refresh_math || host.math_fits_one_line != fits_one_line {
                    host.math_fits_one_line = fits_one_line;
                    match host.editor.math_ml() {
                        Ok(xml) => ui.set_math_ml(format_math_ml(&xml).unwrap_or(xml).into()),
                        Err(error) => ui.set_status(error.to_string().into()),
                    }
                }
            }
        }
        Err(error) => ui.set_status(error.to_string().into()),
    }
    let request = host.editor.take_request();
    drop(host);
    if let Some(request) = request
        && let Err(error) = handle_editor_request(ui, slot, simple, request)
    {
        ui.set_status(error.to_string().into());
    }
}

fn handle_editor_request(
    ui: &RichEditBoxExample,
    slot: &RefCell<MountedEditor>,
    simple: bool,
    request: EditorRequest,
) -> Result<(), Box<dyn Error>> {
    let logical = |physical: i32| physical as f32 / ui.window().scale_factor();
    match request {
        EditorRequest::ContextMenu { x, y } => {
            let (anchor, can_paste, proofing) = {
                let mut host = slot.borrow_mut();
                let [anchor_x, anchor_y] = host.editor.context_menu_anchor(x, y)?;
                host.spelling_menu = match host.editor.spelling_at(x, y) {
                    Ok(menu) => menu,
                    Err(error) => {
                        ui.set_status(error.to_string().into());
                        None
                    }
                };
                let proofing = host
                    .spelling_menu
                    .as_ref()
                    .map_or_else(Vec::new, proofing_items);
                (
                    FlyoutRect {
                        x: logical(anchor_x),
                        y: logical(anchor_y),
                        ..Default::default()
                    },
                    host.editor.can_paste(),
                    proofing,
                )
            };
            ui.set_proofing_items(ModelRc::new(VecModel::from(proofing)));
            ui.invoke_open_editor_menu(simple, false, anchor, can_paste);
            if ui.get_menu_open() {
                slot.borrow().editor.yield_focus();
            }
        }
        EditorRequest::SelectionMenu { x, y } => {
            let (anchor, can_paste) = {
                let host = slot.borrow();
                let [x, y, height] = host.editor.selection_menu_anchor(x, y)?;
                (
                    FlyoutRect {
                        x: logical(x),
                        y: logical(y),
                        height: logical(height),
                        ..Default::default()
                    },
                    host.editor.can_paste(),
                )
            };
            ui.invoke_open_editor_menu(simple, true, anchor, can_paste);
        }
        EditorRequest::PointerMoved { x, y } => {
            let [x, y] = slot.borrow().editor.pointer_in_parent(x, y)?;
            ui.invoke_track_editor_pointer(logical(x), logical(y));
        }
        EditorRequest::DismissMenu { escape } => ui.invoke_dismiss_editor_menu(escape),
        EditorRequest::Traverse { backward } => {
            ui.invoke_dismiss_editor_menu(false);
            slot.borrow().editor.yield_focus();
            ui.invoke_traverse_from_editor(simple, backward);
        }
    }
    Ok(())
}

fn proofing_items(menu: &SpellingMenu) -> Vec<MenuFlyoutItem> {
    let mut items = Vec::new();
    for (index, action) in menu.actions.iter().enumerate() {
        let text = match action {
            SpellingAction::Replace(word) => word,
            SpellingAction::DeleteRepeated => "Delete repeated word",
            SpellingAction::AddToDictionary => {
                if index > 0 {
                    items.push(MenuFlyoutItem {
                        kind: MenuFlyoutItemKind::Separator,
                        ..Default::default()
                    });
                }
                "Add to dictionary"
            }
            SpellingAction::IgnoreAll | SpellingAction::IgnoreOnce => "Ignore",
        };
        items.push(MenuFlyoutItem {
            id: index.to_string().into(),
            text: text.into(),
            ..Default::default()
        });
    }
    items
}

struct MountedPair {
    simple: Rc<RefCell<MountedEditor>>,
    custom: Rc<RefCell<MountedEditor>>,
}

impl MountedPair {
    fn sync_menu(&self, ui: &RichEditBoxExample) {
        let scale = ui.window().scale_factor();
        let px = |logical: f32| (logical * scale).round() as i32;
        let bounds = |panel: FlyoutRect| {
            [
                px(panel.x),
                px(panel.y),
                px(panel.x + panel.width),
                px(panel.y + panel.height),
            ]
        };
        let panels = [
            ui.get_menu_open().then(|| bounds(ui.get_menu_bounds())),
            ui.get_proofing_open()
                .then(|| bounds(ui.get_proofing_bounds())),
        ];
        for slot in [&self.simple, &self.custom] {
            if let Err(error) = slot.borrow().editor.set_menu_overlay(
                panels,
                px(16.0),
                ui.get_menu_tracks_pointer(),
            ) {
                ui.invoke_dismiss_editor_menu(false);
                ui.set_status(error.to_string().into());
                break;
            }
        }
    }
}

fn mount(ui: &RichEditBoxExample, parent: NonZeroIsize) -> Result<MountedPair, Box<dyn Error>> {
    let simple = Rc::new(RefCell::new(MountedEditor::new(RichEdit::new(parent)?)));
    let custom = Rc::new(RefCell::new(MountedEditor::new(RichEdit::new(parent)?)));
    if ui.get_math_samples() {
        for (host, name) in [(&simple, "UnicodeMath editor"), (&custom, "MathML editor")] {
            let host = host.borrow();
            host.editor.set_math_mode(true)?;
            host.editor.set_default_font("Cambria Math", 16.0);
            host.editor.set_accessible_name(name)?;
        }
        let host = custom.clone();
        let weak = ui.as_weak();
        ui.on_set_sample_formula(move || {
            if let (Some(ui), Ok(host)) = (weak.upgrade(), host.try_borrow()) {
                match host.editor.set_math_ml(SAMPLE_FORMULA) {
                    Ok(()) => ui.set_status("".into()),
                    Err(error) => ui.set_status(error.to_string().into()),
                }
            }
        });
        ui.set_math_ready(true);
    } else {
        simple.borrow_mut().editor.set_spelling_enabled(true);
        custom.borrow_mut().editor.set_spelling_enabled(true);
        simple
            .borrow()
            .editor
            .set_accessible_name("Simple text editor")?;
        custom
            .borrow()
            .editor
            .set_accessible_name("Custom rich text editor")?;
        if std::env::args_os().any(|arg| arg == "--spelling") {
            let mut host = custom.borrow_mut();
            if host.editor.set_spelling_locale("en-US")? {
                host.editor.set_default_language(0x0409);
                host.editor
                    .set_text("The quik brown fox jumped over the fence.")?;
            } else if host.editor.set_spelling_locale("zh-Latn-CN-x-ext")? {
                host.editor.set_default_language(0x0804);
                host.editor.set_text("beijng")?;
            } else {
                ui.set_status("No English or Pinyin spell checker is installed.".into());
            }
        } else if std::env::args_os().any(|arg| arg == "--sample-text") {
            custom.borrow().editor.set_text("Apply a format to a selection, then save the document as RTF.\nThe editor preserves Unicode, including 中文 and 😀.")?;
        }
    }

    let host = simple.clone();
    ui.on_simple_command(move |action| {
        if let Ok(host) = host.try_borrow() {
            command(&host.editor, action);
        }
    });
    let host = custom.clone();
    ui.on_custom_command(move |action| {
        if let Ok(host) = host.try_borrow() {
            command(&host.editor, action);
        }
    });
    let simple_menu = simple.clone();
    let custom_menu = custom.clone();
    let weak = ui.as_weak();
    ui.on_menu_command(move |simple, action| {
        let slot = if simple { &simple_menu } else { &custom_menu };
        let action = match action {
            TextCommand::Cut => Edit::Cut,
            TextCommand::Copy => Edit::Copy,
            TextCommand::Paste => Edit::Paste,
            TextCommand::Undo => Edit::Undo,
            TextCommand::Redo => Edit::Redo,
            TextCommand::SelectAll => Edit::SelectAll,
            TextCommand::Bold => Edit::Toggle(Effect::Bold),
            TextCommand::Italic => Edit::Toggle(Effect::Italic),
            TextCommand::Underline => Edit::Toggle(Effect::Underline),
        };
        if let Ok(host) = slot.try_borrow() {
            host.editor.execute(action);
            // Transient commands leave keyboard input at the native editor.
            if weak
                .upgrade()
                .is_some_and(|ui| ui.get_menu_tracks_pointer())
            {
                host.editor.focus();
            }
        }
    });
    let simple_proofing = simple.clone();
    let custom_proofing = custom.clone();
    let weak = ui.as_weak();
    ui.on_proofing_command(move |simple, id| {
        let slot = if simple {
            &simple_proofing
        } else {
            &custom_proofing
        };
        if let (Some(ui), Ok(mut host), Ok(index)) =
            (weak.upgrade(), slot.try_borrow_mut(), id.parse())
        {
            if let Some(menu) = host.spelling_menu.take() {
                if let Err(error) = host.editor.apply_spelling(&menu, index) {
                    ui.set_status(error.to_string().into());
                }
            }
        }
    });
    let simple_menu = simple.clone();
    let custom_menu = custom.clone();
    let weak = ui.as_weak();
    ui.on_restore_menu_focus(move |simple| {
        let slot = if simple { &simple_menu } else { &custom_menu }.clone();
        let weak = weak.clone();
        // Slint moves focus as it removes the active command from the scene.
        Timer::single_shot(Duration::ZERO, move || {
            if weak.upgrade().is_some_and(|ui| !ui.get_menu_open()) {
                slot.borrow().editor.focus();
            }
        });
    });
    let host = custom.clone();
    ui.on_apply_color(move |color, automatic| {
        if let Ok(host) = host.try_borrow() {
            host.editor
                .set_selection_color((!automatic).then(|| rgb(color)));
            host.editor.focus();
        }
    });
    let host = custom.clone();
    let weak = ui.as_weak();
    ui.on_open_document(move || {
        if let (Some(ui), Ok(host)) = (weak.upgrade(), host.try_borrow_mut()) {
            show_status(&ui, open_file(&host.editor));
        }
    });
    let host = custom.clone();
    let weak = ui.as_weak();
    ui.on_save_document(move || {
        if let (Some(ui), Ok(host)) = (weak.upgrade(), host.try_borrow_mut()) {
            show_status(&ui, save_file(&host.editor));
        }
    });
    let host = custom.clone();
    let weak = ui.as_weak();
    ui.on_highlight_matches(move |needle| {
        if let (Some(ui), Ok(host)) = (weak.upgrade(), host.try_borrow()) {
            match host.editor.highlight_matches(&needle) {
                Ok(_) if needle.is_empty() => ui.set_status("".into()),
                Ok(1) => ui.set_status("1 match".into()),
                Ok(count) => ui.set_status(format!("{count} matches").into()),
                Err(error) => ui.set_status(error.to_string().into()),
            }
        }
    });
    let host = custom.clone();
    let weak = ui.as_weak();
    ui.on_find_next(move |needle| {
        if let (Some(ui), Ok(host)) = (weak.upgrade(), host.try_borrow()) {
            let editor = &host.editor;
            let selected = editor.selection();
            if let Some(range) = editor
                .find_from(&needle, selected.end)
                .or_else(|| editor.find_from(&needle, 0))
            {
                editor.set_selection(range);
                editor.reveal_selection();
                editor.focus();
                ui.set_status("".into());
            } else {
                ui.set_status("No matches found".into());
            }
        }
    });

    Ok(MountedPair { simple, custom })
}

fn main() -> Result<(), Box<dyn Error>> {
    let ui = RichEditBoxExample::new()?;
    ui.set_math_samples(std::env::args_os().any(|arg| arg == "--math"));
    command_bar::install(ui.global::<CommandBarBehavior>());
    menu_flyout::install(ui.global::<MenuFlyoutBehavior>());
    if let Ok(accent) = light_accent_text_colors() {
        let [red, green, blue] = accent.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.show()?;
    let weak = ui.as_weak();
    let mounted: RefCell<Option<MountedPair>> = RefCell::new(None);
    let remaining_attempts = Cell::new(20u32);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(25), move || {
        let (Some(ui), Ok(mut mounted)) = (weak.upgrade(), mounted.try_borrow_mut()) else {
            return;
        };
        if mounted.is_none() {
            let Some(remaining) = remaining_attempts.get().checked_sub(1) else {
                return;
            };
            remaining_attempts.set(remaining);
            // A winit window exposes its HWND after processing the first event-loop turn.
            let window = ui.window().window_handle();
            let Ok(handle) = window.window_handle() else {
                if remaining == 0 {
                    ui.set_status(
                        "The window backend did not provide a native Windows handle.".into(),
                    );
                }
                return;
            };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                ui.set_status("RichEditBox requires a Windows host window.".into());
                remaining_attempts.set(0);
                return;
            };
            match mount(&ui, handle.hwnd) {
                Ok(pair) => *mounted = Some(pair),
                Err(error) => {
                    ui.set_status(error.to_string().into());
                    remaining_attempts.set(0);
                    return;
                }
            }
        }
        if let Some(pair) = &*mounted {
            tick(&ui, &pair.simple, true);
            tick(&ui, &pair.custom, false);
            pair.sync_menu(&ui);
        }
    });
    slint::run_event_loop()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::format_math_ml;

    #[test]
    fn math_ml_preview_keeps_operators_inline() {
        let text = format_math_ml("<math><mo>&#x2208;</mo><mo>&lt;</mo></math>").unwrap();
        assert_eq!(text, "<math>\n  <mo>∈</mo>\n  <mo>&lt;</mo>\n</math>");
    }
}
