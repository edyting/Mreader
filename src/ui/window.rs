//! Main application window.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;

use gtk::prelude::*;
use gtk::{
    gdk, gio, glib, pango, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider,
    DrawingArea, DropDown, FileDialog, HeaderBar, Label, Orientation, Overlay, Paned,
    ScrolledWindow, SearchEntry, Stack, TextBuffer, TextTag, TextTagTable, TextView,
};

use crate::config::{AppConfig, ReadingTheme};
use crate::database::{now_rfc3339, LibraryEntry, Storage};
use crate::document::{open_document, Document, DocumentFormat};
use crate::pacer::engine::{PaceState, PacerEngine};
use crate::pacer::modes::PacingMode;
use crate::reader::bookmarks::create_bookmark;
use crate::reader::{ReadingViewState, ViewMode};
use crate::statistics::{format_duration, SessionTracker};
use crate::ui::settings::SettingsPanel;
use crate::ui::sidebar::Sidebar;
use crate::ui::toolbar::BottomToolbar;

struct AppState {
    config: AppConfig,
    storage: Storage,
    document: Option<Document>,
    engine: PacerEngine,
    reading: ReadingViewState,
    session: Option<SessionTracker>,
    status: String,
}

pub fn build_ui(app: &Application) {
    let config = AppConfig::load();
    let storage = Storage::open_default().unwrap_or_else(|e| {
        eprintln!("DB open failed, using memory: {e}");
        Storage::open_in_memory().expect("in-memory db")
    });

    let state = Rc::new(RefCell::new(AppState {
        engine: {
            let mut e = PacerEngine::new();
            e.mode = config.pacing_mode;
            e.wpm = config.default_wpm;
            e.phrase_size = config.phrase_group_size;
            e.paragraph_pause_ms = config.paragraph_pause_ms;
            e
        },
        config: config.clone(),
        storage,
        document: None,
        reading: ReadingViewState::default(),
        session: None,
        status: "Open a document to begin.".into(),
    }));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Mreader")
        .default_width(config.window_width)
        .default_height(config.window_height)
        .build();
    window.add_css_class("mreader-window");

    load_css();
    apply_theme(&window, config.theme);

    // Header
    let header = HeaderBar::new();
    let brand = Label::new(Some("Mreader"));
    brand.add_css_class("brand-label");
    header.set_title_widget(Some(&brand));

    let open_btn = Button::from_icon_name("document-open-symbolic");
    open_btn.set_tooltip_text(Some("Open document (Ctrl+O)"));
    header.pack_start(&open_btn);

    let bookmark_btn = Button::from_icon_name("bookmark-new-symbolic");
    bookmark_btn.set_tooltip_text(Some("Bookmark position (Ctrl+B)"));
    header.pack_start(&bookmark_btn);

    let font_dec = Button::from_icon_name("zoom-out-symbolic");
    let font_inc = Button::from_icon_name("zoom-in-symbolic");
    header.pack_end(&font_inc);
    header.pack_end(&font_dec);
    let search_entry = SearchEntry::new();
    search_entry.set_placeholder_text(Some("Find in document"));
    header.pack_end(&search_entry);

    window.set_titlebar(Some(&header));

    // Body
    let outer = GtkBox::new(Orientation::Vertical, 0);
    let paned = Paned::new(Orientation::Horizontal);
    paned.set_vexpand(true);
    paned.set_hexpand(true);

    let sidebar = Sidebar::new();
    paned.set_start_child(Some(&sidebar.root));
    paned.set_resize_start_child(false);
    paned.set_shrink_start_child(false);

    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    // Reading page
    let reading_page = GtkBox::new(Orientation::Vertical, 0);
    let title_label = Label::new(Some("Welcome to Mreader"));
    title_label.add_css_class("reading-title");
    title_label.set_halign(gtk::Align::Center);
    title_label.set_margin_top(16);

    let meta_label = Label::new(Some(
        "Import a PDF, DOCX, EPUB, Markdown, HTML, RTF or text file to start paced reading.",
    ));
    meta_label.set_wrap(true);
    meta_label.set_halign(gtk::Align::Center);
    meta_label.set_margin_bottom(8);

    let focus_scroll = ScrolledWindow::new();
    focus_scroll.set_vexpand(true);
    focus_scroll.set_hexpand(true);
    focus_scroll.add_css_class("reading-area");

    let tag_table = TextTagTable::new();
    let highlight_tag = TextTag::new(Some("highlight"));
    highlight_tag.set_property(
        "background",
        color_with_opacity(
            &state.borrow().config.appearance.highlight_color,
            state.borrow().config.appearance.highlight_opacity,
        )
        .as_str(),
    );
    let underline_tag = TextTag::new(Some("underline"));
    underline_tag.set_property("underline", pango::Underline::Single);
    underline_tag.set_property(
        "foreground",
        state.borrow().config.appearance.underline_color.as_str(),
    );
    let pointer_tag = TextTag::new(Some("pointer"));
    pointer_tag.set_property("weight", 700i32);
    pointer_tag.set_property(
        "foreground",
        state.borrow().config.appearance.pointer_color.as_str(),
    );
    tag_table.add(&highlight_tag);
    tag_table.add(&underline_tag);
    tag_table.add(&pointer_tag);

    let buffer = TextBuffer::new(Some(&tag_table));
    buffer.set_text(welcome_text());
    let text_view = TextView::with_buffer(&buffer);
    text_view.set_wrap_mode(gtk::WrapMode::WordChar);
    text_view.set_editable(false);
    text_view.set_cursor_visible(false);
    text_view.set_left_margin(48);
    text_view.set_right_margin(48);
    text_view.set_top_margin(24);
    text_view.set_bottom_margin(48);
    text_view.add_css_class("reading-content");
    apply_text_style(&text_view, &state.borrow().config);
    focus_scroll.set_child(Some(&text_view));

    // RSVP overlay
    let rsvp_label = Label::new(Some(""));
    rsvp_label.add_css_class("rsvp-word");
    rsvp_label.set_halign(gtk::Align::Center);
    rsvp_label.set_valign(gtk::Align::Center);
    rsvp_label.set_visible(false);

    let overlay = Overlay::new();
    overlay.set_child(Some(&focus_scroll));
    overlay.add_overlay(&rsvp_label);

    // Original PDF view
    let original_scroll = ScrolledWindow::new();
    original_scroll.set_vexpand(true);
    let pdf_area = DrawingArea::new();
    pdf_area.set_content_width(800);
    pdf_area.set_content_height(1000);
    original_scroll.set_child(Some(&pdf_area));
    original_scroll.set_visible(false);

    reading_page.append(&title_label);
    reading_page.append(&meta_label);
    reading_page.append(&overlay);
    reading_page.append(&original_scroll);

    // Stats page
    let stats_page = build_stats_page();
    let settings_panel = SettingsPanel::new(&config);

    stack.add_named(&reading_page, Some("reading"));
    stack.add_named(&stats_page.root, Some("stats"));
    stack.add_named(&settings_panel.root, Some("settings"));
    stack.set_visible_child_name("reading");

    paned.set_end_child(Some(&stack));
    outer.append(&paned);

    let toolbar = BottomToolbar::new(config.default_wpm);
    let speed_syncing = Rc::new(Cell::new(false));
    let position_syncing = Rc::new(Cell::new(false));
    // Speed presets
    let presets = GtkBox::new(Orientation::Horizontal, 4);
    presets.set_margin_start(8);
    presets.set_margin_end(8);
    for wpm in [100u32, 150, 200, 250, 300, 400, 500, 600, 800, 1000] {
        let b = Button::with_label(&wpm.to_string());
        b.add_css_class("flat");
        b.set_tooltip_text(Some(&format!("{wpm} WPM")));
        let toolbar_c = toolbar.speed_scale.clone();
        let state_c = Rc::clone(&state);
        let wpm_label = toolbar.wpm_label.clone();
        b.connect_clicked(move |_| {
            state_c.borrow_mut().engine.set_wpm(wpm);
            state_c.borrow_mut().config.default_wpm = wpm;
            toolbar_c.set_value(wpm as f64);
            wpm_label.set_text(&format!("{wpm} WPM"));
            if let Some(session) = state_c.borrow_mut().session.as_mut() {
                session.note_wpm(wpm);
            }
        });
        presets.append(&b);
    }
    outer.append(&presets);
    outer.append(&toolbar.root);

    let status = Label::new(Some(&state.borrow().status));
    status.add_css_class("status-bar");
    status.set_halign(gtk::Align::Start);
    outer.append(&status);

    window.set_child(Some(&outer));

    // Shared refresh closure pieces
    let refresh_ui: Rc<dyn Fn()> = {
        let state = Rc::clone(&state);
        let buffer = buffer.clone();
        let title_label = title_label.clone();
        let meta_label = meta_label.clone();
        let paragraph_selector = toolbar.paragraph_selector.clone();
        let toolbar = toolbar_handles(&toolbar);
        let rsvp_label = rsvp_label.clone();
        let text_view = text_view.clone();
        let status = status.clone();
        let highlight_tag = highlight_tag.clone();
        let underline_tag = underline_tag.clone();
        let pointer_tag = pointer_tag.clone();
        let focus_scroll = focus_scroll.clone();
        let original_scroll = original_scroll.clone();
        let pdf_area = pdf_area.clone();
        let speed_syncing = Rc::clone(&speed_syncing);
        let position_syncing = Rc::clone(&position_syncing);

        Rc::new(move || {
            let st = state.borrow();
            let snap = st.engine.snapshot();
            highlight_tag.set_property(
                "background",
                color_with_opacity(
                    &st.config.appearance.highlight_color,
                    st.config.appearance.highlight_opacity,
                )
                .as_str(),
            );
            underline_tag.set_property("foreground", st.config.appearance.underline_color.as_str());
            pointer_tag.set_property("foreground", st.config.appearance.pointer_color.as_str());
            toolbar.0.set_playing(snap.state == PaceState::Running);
            speed_syncing.set(true);
            toolbar.1.set_wpm(snap.wpm);
            speed_syncing.set(false);
            toolbar
                .1
                .set_progress(snap.progress, snap.word_index + 1, st.engine.total_words());

            if let Some(doc) = st.document.as_ref() {
                position_syncing.set(true);
                paragraph_selector.set_range(1.0, doc.paragraphs.len().max(1) as f64);
                paragraph_selector.set_value((snap.paragraph_index + 1) as f64);
                paragraph_selector.set_sensitive(!doc.paragraphs.is_empty());
                position_syncing.set(false);
                title_label.set_text(&doc.title);
                let author = doc
                    .meta
                    .author
                    .clone()
                    .unwrap_or_else(|| doc.format.label().to_string());
                meta_label.set_text(&format!(
                    "{author} · {} words · {}",
                    doc.total_words(),
                    doc.format.label()
                ));

                let is_rsvp = snap.mode == PacingMode::Rsvp;
                rsvp_label.set_visible(is_rsvp);
                text_view.set_visible(!is_rsvp);

                if st.reading.view_mode == ViewMode::Original && doc.supports_original_view {
                    focus_scroll.set_visible(false);
                    original_scroll.set_visible(true);
                    if doc.format == DocumentFormat::Pdf {
                        render_pdf_page(&pdf_area, &doc.path, st.reading.original_page);
                    }
                } else {
                    focus_scroll.set_visible(true);
                    original_scroll.set_visible(false);
                }

                if is_rsvp {
                    let word = snap.current_text;
                    if st.config.rsvp_focal_letter {
                        let idx = PacerEngine::focal_letter_index(&word);
                        let chars: Vec<char> = word.chars().collect();
                        if !chars.is_empty() {
                            let before: String = chars[..idx].iter().collect();
                            let focal = chars[idx];
                            let after: String = chars[idx + 1..].iter().collect();
                            let before = glib::markup_escape_text(&before);
                            let focal = glib::markup_escape_text(&focal.to_string());
                            let after = glib::markup_escape_text(&after);
                            rsvp_label.set_markup(&format!(
                                "{before}<span color=\"#c0392b\"><b>{focal}</b></span>{after}"
                            ));
                        } else {
                            rsvp_label.set_text("");
                        }
                    } else {
                        rsvp_label.set_text(&word);
                    }
                } else if let Some(display) = st.reading.display.clone() {
                    // Update buffer text once; apply tags for active range
                    let current = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                    if current.as_str() != display.plain.as_str() {
                        buffer.set_text(&display.plain);
                    }
                    // Clear tags
                    let start = buffer.start_iter();
                    let end = buffer.end_iter();
                    buffer.remove_tag(&highlight_tag, &start, &end);
                    buffer.remove_tag(&underline_tag, &start, &end);
                    buffer.remove_tag(&pointer_tag, &start, &end);

                    let end_word = snap.phrase_end;
                    if let Some((a, b)) = display.span_for_range(snap.word_index, end_word) {
                        let mut i1 = buffer.iter_at_offset(display.character_offset(a) as i32);
                        let i2 = buffer.iter_at_offset(display.character_offset(b) as i32);
                        match snap.mode {
                            PacingMode::WordHighlight | PacingMode::PhrasePacing => {
                                buffer.apply_tag(&highlight_tag, &i1, &i2);
                            }
                            PacingMode::MovingUnderline => {
                                buffer.apply_tag(&underline_tag, &i1, &i2);
                            }
                            PacingMode::ReadingPointer => {
                                buffer.apply_tag(&pointer_tag, &i1, &i2);
                            }
                            PacingMode::Rsvp => {}
                        }
                        text_view.scroll_to_iter(&mut i1, 0.2, true, 0.0, 0.35);
                    }
                }
            } else {
                position_syncing.set(true);
                paragraph_selector.set_range(1.0, 1.0);
                paragraph_selector.set_value(1.0);
                paragraph_selector.set_sensitive(false);
                position_syncing.set(false);
            }
            status.set_text(&st.status);
            let _ = &st;
        })
    };

    // Parse documents off the GTK thread so large files do not freeze the window.
    let (load_tx, load_rx) = mpsc::channel::<(
        u64,
        PathBuf,
        Result<Document, crate::document::DocumentError>,
    )>();
    let latest_open = Rc::new(Cell::new(0_u64));
    {
        let state = Rc::clone(&state);
        let buffer = buffer.clone();
        let text_view = text_view.clone();
        let refresh = Rc::clone(&refresh_ui);
        let latest_open = Rc::clone(&latest_open);
        glib::timeout_add_local(std::time::Duration::from_millis(25), move || {
            loop {
                match load_rx.try_recv() {
                    Ok((request_id, path, result)) => {
                        if request_id != latest_open.get() {
                            continue;
                        }
                        complete_document_load(
                            result, &path, &state, &buffer, &text_view, &refresh,
                        );
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return glib::ControlFlow::Break;
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    let open_path = {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        let latest_open = Rc::clone(&latest_open);
        move |path: PathBuf| {
            let request_id = latest_open.get().wrapping_add(1);
            latest_open.set(request_id);
            state.borrow_mut().status = format!(
                "Loading {}…",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("document")
            );
            refresh();

            let sender = load_tx.clone();
            std::thread::spawn(move || {
                let result = open_document(&path);
                if sender.send((request_id, path, result)).is_err() {
                    log::debug!("Document load result discarded because the window closed");
                }
            });
        }
    };

    // File open
    {
        let window = window.clone();
        let open_path = open_path.clone();
        open_btn.connect_clicked(move |_| {
            let dialog = FileDialog::builder().title("Open document").build();
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Documents"));
            for pattern in [
                "*.pdf",
                "*.docx",
                "*.epub",
                "*.txt",
                "*.md",
                "*.markdown",
                "*.html",
                "*.htm",
                "*.rtf",
            ] {
                filter.add_pattern(pattern);
            }
            filters.append(&filter);
            dialog.set_filters(Some(&filters));
            let open_path = open_path.clone();
            dialog.open(Some(&window), None::<&gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        open_path(path);
                    }
                }
            });
        });
    }

    // Drag and drop
    {
        let drop_target = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
        let open_path = open_path.clone();
        drop_target.connect_drop(move |_, value, _, _| {
            if let Ok(file) = value.get::<gio::File>() {
                if let Some(path) = file.path() {
                    open_path(path);
                    return true;
                }
            }
            false
        });
        window.add_controller(drop_target);
    }

    // Toolbar controls
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        let position_syncing = Rc::clone(&position_syncing);
        toolbar
            .paragraph_selector
            .connect_value_changed(move |selector| {
                if position_syncing.get() {
                    return;
                }
                let paragraph = (selector.value() as usize).saturating_sub(1);
                state.borrow_mut().engine.jump_to_paragraph(paragraph);
                refresh();
            });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.play_btn.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            let previous_state = st.engine.state;
            let was_idle = matches!(st.engine.state, PaceState::Idle | PaceState::Finished);
            st.engine.toggle_play_pause();
            if st.engine.state == PaceState::Running {
                if was_idle || st.session.is_none() {
                    if let Some(doc) = st.document.as_ref() {
                        st.session = Some(SessionTracker::start(
                            &doc.id,
                            st.engine.word_index,
                            st.engine.wpm,
                        ));
                    }
                } else if previous_state == PaceState::Paused {
                    if let Some(session) = st.session.as_mut() {
                        session.resume();
                    }
                }
                st.status = "Reading…".into();
            } else if st.engine.state == PaceState::Paused {
                if let Some(session) = st.session.as_mut() {
                    session.pause();
                }
                st.status = "Paused".into();
            }
            drop(st);
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.stop_btn.connect_clicked(move |_| {
            finish_session(&state);
            state.borrow_mut().engine.stop();
            state.borrow_mut().status = "Stopped".into();
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.prev_btn.connect_clicked(move |_| {
            navigate_word_or_pdf_page(&state, -1, &refresh);
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.next_btn.connect_clicked(move |_| {
            navigate_word_or_pdf_page(&state, 1, &refresh);
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.prev_paragraph_btn.connect_clicked(move |_| {
            state.borrow_mut().engine.prev_paragraph();
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.next_paragraph_btn.connect_clicked(move |_| {
            state.borrow_mut().engine.next_paragraph();
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.restart_btn.connect_clicked(move |_| {
            finish_session(&state);
            let mut st = state.borrow_mut();
            if let Some(doc) = st.document.as_ref() {
                let doc_id = doc.id.clone();
                let wpm = st.engine.wpm;
                st.engine.restart();
                st.session = Some(SessionTracker::start(&doc_id, 0, wpm));
                st.status = "Reading…".into();
            } else {
                st.status = "Open a document before starting a session.".into();
            }
            drop(st);
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        let wpm_label = toolbar.wpm_label.clone();
        let speed_syncing = Rc::clone(&speed_syncing);
        toolbar.speed_scale.connect_value_changed(move |scale| {
            if speed_syncing.get() {
                return;
            }
            let wpm = AppConfig::clamp_wpm(scale.value() as u32);
            let mut st = state.borrow_mut();
            st.engine.set_wpm(wpm);
            st.config.default_wpm = wpm;
            if let Some(session) = st.session.as_mut() {
                session.note_wpm(wpm);
            }
            wpm_label.set_text(&format!("{wpm} WPM"));
            drop(st);
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        toolbar.mode_dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            let mode = PacingMode::all().get(idx).copied().unwrap_or_default();
            let mut st = state.borrow_mut();
            st.engine.mode = mode;
            st.config.pacing_mode = mode;
            drop(st);
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        let view_toggle = toolbar.view_toggle.clone();
        toolbar.view_toggle.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            if st
                .document
                .as_ref()
                .map(|d| d.supports_original_view)
                .unwrap_or(false)
            {
                st.reading.view_mode = match st.reading.view_mode {
                    ViewMode::Focus => {
                        view_toggle.set_label("Original View");
                        ViewMode::Original
                    }
                    ViewMode::Original => {
                        view_toggle.set_label("Focus View");
                        ViewMode::Focus
                    }
                };
            } else {
                st.status = "Original view is currently available for PDF documents.".into();
            }
            drop(st);
            refresh();
        });
    }

    // Sidebar navigation
    connect_sidebar(
        &sidebar,
        &stack,
        &window,
        &text_view,
        &state,
        &stats_page,
        &settings_panel,
        open_path.clone(),
        Rc::clone(&refresh_ui),
    );

    // Bookmark
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        bookmark_btn.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            if let Some(doc) = st.document.as_ref() {
                let para = st
                    .engine
                    .paragraph_index_of(st.engine.word_index)
                    .unwrap_or(0);
                match create_bookmark(
                    &st.storage,
                    &doc.id,
                    para,
                    st.engine.word_index,
                    "Reading position",
                ) {
                    Ok(_) => st.status = "Bookmark saved.".into(),
                    Err(e) => st.status = format!("Bookmark failed: {e}"),
                }
            }
            drop(st);
            refresh();
        });
    }

    // Search within the loaded document and move the pacer to the next match.
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        search_entry.connect_activate(move |entry| {
            let mut st = state.borrow_mut();
            st.reading.search_query = entry.text().to_string();
            let current = st.engine.word_index;
            match st.reading.find_next(current) {
                Some(index) => {
                    st.engine.word_index = index;
                    st.status = format!("Found match at word {}.", index + 1);
                }
                None => st.status = "No matching text found.".into(),
            }
            drop(st);
            refresh();
        });
    }

    // Font size
    {
        let state = Rc::clone(&state);
        let text_view = text_view.clone();
        let refresh = Rc::clone(&refresh_ui);
        font_inc.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            st.config.font_size = (st.config.font_size + 1.0).min(48.0);
            apply_text_style(&text_view, &st.config);
            drop(st);
            refresh();
        });
    }
    {
        let state = Rc::clone(&state);
        let text_view = text_view.clone();
        let refresh = Rc::clone(&refresh_ui);
        font_dec.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            st.config.font_size = (st.config.font_size - 1.0).max(10.0);
            apply_text_style(&text_view, &st.config);
            drop(st);
            refresh();
        });
    }

    // Keyboard shortcuts
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        let open_btn = open_btn.clone();
        let text_view = text_view.clone();
        let controller = gtk::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _code, mods| {
            use gdk::Key;
            let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
            match key {
                Key::space => {
                    {
                        let mut st = state.borrow_mut();
                        let previous_state = st.engine.state;
                        st.engine.toggle_play_pause();
                        if st.engine.state == PaceState::Running {
                            if st.session.is_none() {
                                if let Some(doc) = st.document.as_ref() {
                                    let idx = st.engine.word_index;
                                    let wpm = st.engine.wpm;
                                    st.session = Some(SessionTracker::start(&doc.id, idx, wpm));
                                }
                            } else if previous_state == PaceState::Paused {
                                if let Some(session) = st.session.as_mut() {
                                    session.resume();
                                }
                            }
                            st.status = "Reading…".into();
                        } else if st.engine.state == PaceState::Paused {
                            if let Some(session) = st.session.as_mut() {
                                session.pause();
                            }
                            st.status = "Paused".into();
                        }
                    }
                    refresh();
                    glib::Propagation::Stop
                }
                Key::Left => {
                    navigate_word_or_pdf_page(&state, -1, &refresh);
                    glib::Propagation::Stop
                }
                Key::Right => {
                    navigate_word_or_pdf_page(&state, 1, &refresh);
                    glib::Propagation::Stop
                }
                Key::Up => {
                    {
                        let mut st = state.borrow_mut();
                        let wpm = AppConfig::clamp_wpm(st.engine.wpm + 10);
                        st.engine.set_wpm(wpm);
                        st.config.default_wpm = wpm;
                    }
                    refresh();
                    glib::Propagation::Stop
                }
                Key::Down => {
                    {
                        let mut st = state.borrow_mut();
                        let wpm = AppConfig::clamp_wpm(st.engine.wpm.saturating_sub(10));
                        st.engine.set_wpm(wpm);
                        st.config.default_wpm = wpm;
                    }
                    refresh();
                    glib::Propagation::Stop
                }
                Key::Escape => {
                    finish_session(&state);
                    state.borrow_mut().engine.stop();
                    state.borrow_mut().status = "Stopped".into();
                    refresh();
                    glib::Propagation::Stop
                }
                Key::o if ctrl => {
                    open_btn.emit_clicked();
                    glib::Propagation::Stop
                }
                Key::b if ctrl => {
                    // bookmark via same logic
                    let mut st = state.borrow_mut();
                    if let Some(doc) = st.document.as_ref() {
                        let para = st
                            .engine
                            .paragraph_index_of(st.engine.word_index)
                            .unwrap_or(0);
                        let _ = create_bookmark(
                            &st.storage,
                            &doc.id,
                            para,
                            st.engine.word_index,
                            "Reading position",
                        );
                        st.status = "Bookmark saved.".into();
                    }
                    drop(st);
                    refresh();
                    glib::Propagation::Stop
                }
                Key::plus | Key::equal if ctrl => {
                    let mut st = state.borrow_mut();
                    st.config.font_size = (st.config.font_size + 1.0).min(48.0);
                    apply_text_style(&text_view, &st.config);
                    drop(st);
                    refresh();
                    glib::Propagation::Stop
                }
                Key::minus if ctrl => {
                    let mut st = state.borrow_mut();
                    st.config.font_size = (st.config.font_size - 1.0).max(10.0);
                    apply_text_style(&text_view, &st.config);
                    drop(st);
                    refresh();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        window.add_controller(controller);
    }

    // Pacer tick — 16ms poll with monotonic engine timing
    {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh_ui);
        let mut completion_recorded = false;
        glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let (changed, completed_now) = {
                let mut st = state.borrow_mut();
                let changed = st.engine.tick();
                let finished = st.engine.state == PaceState::Finished;
                let completed_now = finished && !completion_recorded;
                completion_recorded = finished;
                if completed_now {
                    st.status = "Document complete.".into();
                    if let Some(doc) = st.document.as_ref() {
                        if let Err(error) =
                            st.storage
                                .update_progress(&doc.id, st.engine.word_index as i64, true)
                        {
                            log::warn!("Could not save completed reading progress: {error}");
                        }
                    }
                }
                (changed, completed_now)
            };
            if completed_now {
                finish_session(&state);
            }
            if changed {
                refresh();
            }
            glib::ControlFlow::Continue
        });
    }

    // Persist on close
    {
        let state = Rc::clone(&state);
        let apply_settings = settings_apply_handles(&settings_panel);
        window.connect_close_request(move |win| {
            finish_session(&state);
            let mut st = state.borrow_mut();
            apply_settings(&mut st.config);
            if let Some(doc) = st.document.as_ref() {
                if let Err(error) = st.storage.update_progress(
                    &doc.id,
                    st.engine.word_index as i64,
                    st.engine.state == PaceState::Finished,
                ) {
                    log::warn!("Could not save reading progress: {error}");
                }
            }
            st.config.window_width = win.width();
            st.config.window_height = win.height();
            if let Err(error) = st.config.save() {
                log::warn!("Could not save application settings: {error}");
            }
            glib::Propagation::Proceed
        });
    }

    refresh_ui();
    if let Ok(path) = std::env::var("MREADER_OPEN") {
        unsafe {
            std::env::remove_var("MREADER_OPEN");
        }
        open_path(PathBuf::from(path));
    }
    window.present();
}

struct StatsPage {
    root: ScrolledWindow,
    summary: Label,
    chart: DrawingArea,
}

fn complete_document_load(
    result: Result<Document, crate::document::DocumentError>,
    path: &Path,
    state: &Rc<RefCell<AppState>>,
    buffer: &TextBuffer,
    text_view: &TextView,
    refresh: &Rc<dyn Fn()>,
) {
    let mut doc = match result {
        Ok(doc) => doc,
        Err(error) => {
            state.borrow_mut().status = format!("Could not open document: {error}");
            refresh();
            return;
        }
    };

    match state.borrow().storage.find_by_path(&path.to_string_lossy()) {
        Ok(Some(entry)) => {
            doc.id = entry.id;
            doc.progress_word_index = entry.progress_word.max(0) as usize;
        }
        Ok(None) => {}
        Err(error) => log::warn!("Could not restore document progress: {error}"),
    }

    finish_session(state);

    let entry = LibraryEntry {
        id: doc.id.clone(),
        title: doc.title.clone(),
        path: path.to_string_lossy().to_string(),
        format: doc.format.label().to_string(),
        word_count: doc.total_words() as i64,
        progress_word: doc.progress_word_index as i64,
        last_opened: now_rfc3339(),
        completed: false,
    };
    if let Err(error) = state.borrow().storage.upsert_library_entry(&entry) {
        log::warn!("Could not save document to library: {error}");
    }

    {
        let mut st = state.borrow_mut();
        st.engine.load_document(&doc);
        st.engine.mode = st.config.pacing_mode;
        st.engine.phrase_size = st.config.phrase_group_size;
        st.engine.paragraph_pause_ms = st.config.paragraph_pause_ms;
        let default_wpm = st.config.default_wpm;
        st.engine.set_wpm(default_wpm);
        st.reading.load(&doc);
        if let Some(display) = st.reading.display.as_ref() {
            buffer.set_text(&display.plain);
        }
        st.status = format!("Loaded «{}» ({} words)", doc.title, doc.total_words());
        st.document = Some(doc);
    }
    apply_text_style(text_view, &state.borrow().config);
    refresh();
}

fn build_stats_page() -> StatsPage {
    let box_ = GtkBox::new(Orientation::Vertical, 12);
    box_.set_margin_top(16);
    box_.set_margin_start(24);
    box_.set_margin_end(24);
    let title = Label::new(Some("Reading Statistics"));
    title.add_css_class("reading-title");
    title.set_halign(gtk::Align::Start);
    let summary = Label::new(Some("No sessions yet."));
    summary.set_halign(gtk::Align::Start);
    summary.set_wrap(true);
    summary.add_css_class("stats-card");
    let chart = DrawingArea::new();
    chart.add_css_class("chart-area");
    chart.set_content_height(200);
    chart.set_hexpand(true);
    box_.append(&title);
    box_.append(&summary);
    box_.append(&Label::new(Some("Speed over time")));
    box_.append(&chart);
    let root = ScrolledWindow::new();
    root.set_child(Some(&box_));
    StatsPage {
        root,
        summary,
        chart,
    }
}

fn refresh_stats(page: &StatsPage, state: &Rc<RefCell<AppState>>) {
    let st = state.borrow();
    if let Ok(agg) = st.storage.aggregate_stats() {
        let today = chrono::Utc::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let today_stats = match st.storage.daily_stats(1) {
            Ok(stats) => stats.into_iter().find(|daily| daily.day == today),
            Err(error) => {
                log::warn!("Could not load daily reading statistics: {error}");
                None
            }
        };
        let today_reading_secs = today_stats.map(|daily| daily.reading_secs).unwrap_or(0);
        page.summary.set_text(&format!(
            "Total time: {}\nToday's reading: {} / {} min\nWords processed: {}\nAverage speed: {:.0} WPM\nHighest speed: {} WPM\nSessions: {}\nDocuments completed: {}",
            format_duration(agg.total_reading_secs),
            format_duration(today_reading_secs),
            st.config.daily_goal_minutes,
            agg.total_words,
            agg.average_wpm,
            agg.highest_wpm,
            agg.session_count,
            agg.documents_completed
        ));
    }
    let history = st.storage.speed_history(40).unwrap_or_default();
    page.chart.set_draw_func(move |_area, cr, width, height| {
        cr.set_source_rgb(0.95, 0.95, 0.94);
        cr.paint().ok();
        if history.is_empty() {
            return;
        }
        let max_wpm = history.iter().map(|(_, w)| *w).max().unwrap_or(1).max(1) as f64;
        let n = history.len().max(1) as f64;
        cr.set_source_rgb(0.9, 0.49, 0.13);
        cr.set_line_width(2.0);
        for (i, (_, wpm)) in history.iter().enumerate() {
            let x = (i as f64 / (n - 1.0).max(1.0)) * (width as f64 - 20.0) + 10.0;
            let y = height as f64 - 10.0 - ((*wpm as f64 / max_wpm) * (height as f64 - 20.0));
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
        cr.stroke().ok();
    });
    page.chart.queue_draw();
}

fn connect_sidebar(
    sidebar: &Sidebar,
    stack: &Stack,
    window: &ApplicationWindow,
    text_view: &TextView,
    state: &Rc<RefCell<AppState>>,
    stats: &StatsPage,
    settings: &SettingsPanel,
    open_path: impl Fn(PathBuf) + Clone + 'static,
    refresh: Rc<dyn Fn()>,
) {
    let show_library = {
        let sidebar_btns = sidebar_list_clone(sidebar);
        let state = Rc::clone(state);
        let open_path = open_path.clone();
        let stack = stack.clone();
        move || {
            stack.set_visible_child_name("reading");
            populate_library(&sidebar_btns, &state, open_path.clone());
            // clicking items already wired in populate
            let _ = &open_path;
        }
    };

    {
        let show_library = show_library.clone();
        sidebar.library_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                show_library();
            }
        });
    }
    {
        let stack = stack.clone();
        let state = Rc::clone(state);
        let list = sidebar_list_clone(sidebar);
        let open_path = open_path.clone();
        sidebar.recent_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                stack.set_visible_child_name("reading");
                list.clear_list();
                if let Ok(recent) = state.borrow().storage.recent_documents(20) {
                    for entry in recent {
                        let label = format!("{} ({})", entry.title, entry.format);
                        let btn = list.add_list_button(&label);
                        let path = PathBuf::from(entry.path);
                        let open_path = open_path.clone();
                        btn.connect_clicked(move |_| open_path(path.clone()));
                    }
                }
            }
        });
    }
    {
        let stack = stack.clone();
        let state = Rc::clone(state);
        let list = sidebar_list_clone(sidebar);
        let refresh = Rc::clone(&refresh);
        sidebar.bookmarks_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                stack.set_visible_child_name("reading");
                list.clear_list();
                let st = state.borrow();
                if let Some(doc) = st.document.as_ref() {
                    if let Ok(bms) = st.storage.bookmarks_for(&doc.id) {
                        for bm in bms {
                            let label =
                                format!("¶{} · word {}", bm.paragraph_index + 1, bm.word_index + 1);
                            let b = list.add_list_button(&label);
                            let state = Rc::clone(&state);
                            let refresh = Rc::clone(&refresh);
                            let idx = bm.word_index as usize;
                            b.connect_clicked(move |_| {
                                state.borrow_mut().engine.word_index = idx;
                                refresh();
                            });
                        }
                    }
                } else {
                    list.add_list_button("Open a document to see bookmarks");
                }
            }
        });
    }
    {
        let stack = stack.clone();
        let state = Rc::clone(state);
        let list = sidebar_list_clone(sidebar);
        let open_path = open_path.clone();
        sidebar.history_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                stack.set_visible_child_name("reading");
                list.clear_list();
                if let Ok(recent) = state.borrow().storage.recent_documents(50) {
                    for entry in recent {
                        let pct = if entry.word_count > 0 {
                            (entry.progress_word as f64 / entry.word_count as f64) * 100.0
                        } else {
                            0.0
                        };
                        let label = format!("{:.0}% · {}", pct, entry.title);
                        let b = list.add_list_button(&label);
                        let path = PathBuf::from(entry.path);
                        let open_path = open_path.clone();
                        b.connect_clicked(move |_| open_path(path.clone()));
                    }
                }
            }
        });
    }
    {
        let stack = stack.clone();
        let state = Rc::clone(state);
        let stats_summary = stats.summary.clone();
        let stats_chart = stats.chart.clone();
        sidebar.stats_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                stack.set_visible_child_name("stats");
                let page = StatsPage {
                    root: ScrolledWindow::new(),
                    summary: stats_summary.clone(),
                    chart: stats_chart.clone(),
                };
                refresh_stats(&page, &state);
            }
        });
    }
    {
        let stack = stack.clone();
        let window = window.clone();
        let text_view = text_view.clone();
        let state = Rc::clone(state);
        let settings_root_apply = settings_apply_handles(settings);
        let refresh = Rc::clone(&refresh);
        sidebar.settings_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                stack.set_visible_child_name("settings");
            } else {
                let mut st = state.borrow_mut();
                settings_root_apply(&mut st.config);
                st.engine.mode = st.config.pacing_mode;
                st.engine.phrase_size = st.config.phrase_group_size;
                st.engine.paragraph_pause_ms = st.config.paragraph_pause_ms;
                let default_wpm = st.config.default_wpm;
                st.engine.set_wpm(default_wpm);
                if let Err(error) = st.config.save() {
                    log::warn!("Could not save application settings: {error}");
                }
                let config = st.config.clone();
                drop(st);
                apply_theme(&window, config.theme);
                apply_text_style(&text_view, &config);
                refresh();
            }
        });
    }

    show_library();
}

// --- helpers ---

fn toolbar_handles(tb: &BottomToolbar) -> (BottomToolbarPlay, BottomToolbarProgress) {
    (
        BottomToolbarPlay {
            play_btn: tb.play_btn.clone(),
        },
        BottomToolbarProgress {
            speed_scale: tb.speed_scale.clone(),
            wpm_label: tb.wpm_label.clone(),
            progress_label: tb.progress_label.clone(),
        },
    )
}

struct BottomToolbarPlay {
    play_btn: Button,
}
impl BottomToolbarPlay {
    fn set_playing(&self, playing: bool) {
        let icon = if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.play_btn.set_icon_name(icon);
    }
}
struct BottomToolbarProgress {
    speed_scale: gtk::Scale,
    wpm_label: Label,
    progress_label: Label,
}
impl BottomToolbarProgress {
    fn set_wpm(&self, wpm: u32) {
        self.wpm_label.set_text(&format!("{wpm} WPM"));
        if (self.speed_scale.value() as u32) != wpm {
            self.speed_scale.set_value(wpm as f64);
        }
    }
    fn set_progress(&self, progress: f64, word: usize, total: usize) {
        self.progress_label
            .set_text(&format!("{:.0}%  ({word}/{total})", progress * 100.0));
    }
}

#[derive(Clone)]
struct SidebarList {
    list_box: GtkBox,
}
impl SidebarList {
    fn clear_list(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }
    }
    fn add_list_button(&self, label: &str) -> Button {
        let btn = Button::with_label(label);
        btn.add_css_class("library-row");
        btn.add_css_class("flat");
        btn.set_halign(gtk::Align::Fill);
        btn.set_hexpand(true);
        self.list_box.append(&btn);
        btn
    }
}
fn sidebar_list_clone(sidebar: &Sidebar) -> SidebarList {
    SidebarList {
        list_box: sidebar.list_box.clone(),
    }
}
fn settings_apply_handles(settings: &SettingsPanel) -> impl Fn(&mut AppConfig) + Clone {
    let font_family = settings.font_family.clone();
    let font_size = settings.font_size.clone();
    let line_spacing = settings.line_spacing.clone();
    let paragraph_spacing = settings.paragraph_spacing.clone();
    let max_width = settings.max_width.clone();
    let alignment = settings.alignment.clone();
    let theme = settings.theme.clone();
    let pacing_mode = settings.pacing_mode.clone();
    let default_wpm = settings.default_wpm.clone();
    let phrase_size = settings.phrase_size.clone();
    let auto_para = settings.auto_para.clone();
    let paragraph_pause = settings.paragraph_pause.clone();
    let rsvp_transition = settings.rsvp_transition.clone();
    let rsvp_focal = settings.rsvp_focal.clone();
    let daily_goal = settings.daily_goal.clone();
    let highlight_opacity = settings.highlight_opacity.clone();
    move |cfg: &mut AppConfig| {
        let panel = SettingsPanelView {
            font_family: font_family.clone(),
            font_size: font_size.clone(),
            line_spacing: line_spacing.clone(),
            paragraph_spacing: paragraph_spacing.clone(),
            max_width: max_width.clone(),
            alignment: alignment.clone(),
            theme: theme.clone(),
            pacing_mode: pacing_mode.clone(),
            default_wpm: default_wpm.clone(),
            phrase_size: phrase_size.clone(),
            auto_para: auto_para.clone(),
            paragraph_pause: paragraph_pause.clone(),
            rsvp_transition: rsvp_transition.clone(),
            rsvp_focal: rsvp_focal.clone(),
            daily_goal: daily_goal.clone(),
            highlight_opacity: highlight_opacity.clone(),
        };
        panel.apply(cfg);
    }
}

struct SettingsPanelView {
    font_family: DropDown,
    font_size: gtk::SpinButton,
    line_spacing: gtk::SpinButton,
    paragraph_spacing: gtk::SpinButton,
    max_width: gtk::SpinButton,
    alignment: DropDown,
    theme: DropDown,
    pacing_mode: DropDown,
    default_wpm: gtk::SpinButton,
    phrase_size: gtk::SpinButton,
    auto_para: gtk::Switch,
    paragraph_pause: gtk::SpinButton,
    rsvp_transition: gtk::Switch,
    rsvp_focal: gtk::Switch,
    daily_goal: gtk::SpinButton,
    highlight_opacity: gtk::Scale,
}
impl SettingsPanelView {
    fn apply(&self, cfg: &mut AppConfig) {
        if let Some(s) = self
            .font_family
            .selected_item()
            .and_then(|o| o.downcast::<gtk::StringObject>().ok())
        {
            cfg.font_family = s.string().to_string();
        }
        cfg.font_size = self.font_size.value();
        cfg.line_spacing = self.line_spacing.value();
        cfg.paragraph_spacing = self.paragraph_spacing.value();
        cfg.max_reading_width = self.max_width.value();
        cfg.text_alignment = match self.alignment.selected() {
            1 => crate::config::TextAlignment::Center,
            2 => crate::config::TextAlignment::Justify,
            _ => crate::config::TextAlignment::Left,
        };
        cfg.theme = match self.theme.selected() {
            1 => ReadingTheme::Dark,
            2 => ReadingTheme::Sepia,
            _ => ReadingTheme::Light,
        };
        cfg.pacing_mode = PacingMode::all()
            .get(self.pacing_mode.selected() as usize)
            .copied()
            .unwrap_or_default();
        cfg.default_wpm = AppConfig::clamp_wpm(self.default_wpm.value() as u32);
        cfg.phrase_group_size = self.phrase_size.value() as usize;
        cfg.auto_advance_paragraph = self.auto_para.is_active();
        cfg.paragraph_pause_ms = self.paragraph_pause.value() as u64;
        cfg.rsvp_transition = self.rsvp_transition.is_active();
        cfg.rsvp_focal_letter = self.rsvp_focal.is_active();
        cfg.daily_goal_minutes = self.daily_goal.value() as u32;
        cfg.appearance.highlight_opacity = self.highlight_opacity.value();
    }
}

fn populate_library(
    list: &SidebarList,
    state: &Rc<RefCell<AppState>>,
    open_path: impl Fn(PathBuf) + Clone + 'static,
) {
    list.clear_list();
    if let Ok(recent) = state.borrow().storage.recent_documents(30) {
        if recent.is_empty() {
            list.add_list_button("No documents yet — click Open");
            return;
        }
        for entry in recent {
            let label = format!("{} · {}", entry.title, entry.format);
            let button = list.add_list_button(&label);
            let path = PathBuf::from(entry.path);
            let open_path = open_path.clone();
            button.connect_clicked(move |_| open_path(path.clone()));
        }
    }
}

fn finish_session(state: &Rc<RefCell<AppState>>) {
    let mut st = state.borrow_mut();
    if let Some(mut session) = st.session.take() {
        let idx = st.engine.word_index;
        if let Err(error) = session.finish(&st.storage, idx) {
            log::warn!("Could not save reading session: {error}");
        }
    }
}

fn navigate_word_or_pdf_page(
    state: &Rc<RefCell<AppState>>,
    direction: i32,
    refresh: &Rc<dyn Fn()>,
) {
    let mut st = state.borrow_mut();
    let is_original_pdf = st.reading.view_mode == ViewMode::Original
        && st
            .document
            .as_ref()
            .is_some_and(|doc| doc.format == DocumentFormat::Pdf);
    if is_original_pdf {
        let page_count = st
            .document
            .as_ref()
            .and_then(|doc| doc.meta.page_count)
            .unwrap_or(1)
            .max(1) as i32;
        st.reading.original_page = (st.reading.original_page + direction).clamp(0, page_count - 1);
        st.status = format!("Page {} of {page_count}.", st.reading.original_page + 1);
    } else if direction < 0 {
        st.engine.prev_word();
    } else {
        st.engine.next_word();
    }
    drop(st);
    refresh();
}

fn apply_text_style(view: &TextView, cfg: &AppConfig) {
    let css = format!(
        "textview.reading-content {{ font-family: '{}'; font-size: {}pt; }}",
        cfg.font_family, cfg.font_size
    );
    let provider = CssProvider::new();
    provider.load_from_string(&css);
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    view.set_justification(match cfg.text_alignment {
        crate::config::TextAlignment::Left => gtk::Justification::Left,
        crate::config::TextAlignment::Center => gtk::Justification::Center,
        crate::config::TextAlignment::Justify => gtk::Justification::Fill,
    });
    view.set_pixels_inside_wrap(
        ((cfg.font_size * cfg.line_spacing) - cfg.font_size)
            .round()
            .max(0.0) as i32,
    );
    view.set_width_request(cfg.max_reading_width.round() as i32);
    view.set_halign(gtk::Align::Center);
}

fn color_with_opacity(color: &str, opacity: f64) -> String {
    if color.len() == 7 && color.starts_with('#') {
        format!(
            "{color}{:02X}",
            (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
        )
    } else {
        color.to_string()
    }
}

fn apply_theme(window: &ApplicationWindow, theme: ReadingTheme) {
    for c in ["theme-light", "theme-dark", "theme-sepia"] {
        window.remove_css_class(c);
    }
    match theme {
        ReadingTheme::Light => window.add_css_class("theme-light"),
        ReadingTheme::Dark => window.add_css_class("theme-dark"),
        ReadingTheme::Sepia => window.add_css_class("theme-sepia"),
    }
}

fn load_css() {
    let provider = CssProvider::new();
    let css = include_str!("../../assets/styles/app.css");
    provider.load_from_string(css);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn welcome_text() -> &'static str {
    "Welcome to Mreader\n\n\
     Mreader is a distraction-free reader with a visual pacer to help you practise \
     faster, steadier reading.\n\n\
     Open a document with the toolbar button or press Ctrl+O, then press Space to begin.\n\n\
     Supported formats: PDF, DOCX, EPUB, TXT, Markdown, HTML and RTF."
}

fn render_pdf_page(area: &DrawingArea, path: &Path, page: i32) {
    let path = path.to_path_buf();
    area.set_draw_func(move |_area, cr, width, height| {
        match crate::document::pdf::render_page(&path, page, 1.25) {
            Ok((surface, _, _)) => {
                let sw = surface.width() as f64;
                let sh = surface.height() as f64;
                let scale = (width as f64 / sw).min(height as f64 / sh).min(1.0);
                cr.set_source_rgb(0.9, 0.9, 0.9);
                cr.paint().ok();
                cr.scale(scale, scale);
                cr.set_source_surface(&surface, 0.0, 0.0).ok();
                cr.paint().ok();
            }
            Err(_) => {
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.paint().ok();
            }
        }
    });
    area.queue_draw();
}
