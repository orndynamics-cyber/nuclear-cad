//! The Projects page of the File view: the library's real folders and designs, a search over
//! them, and Rename, Move, Duplicate, a portable copy and New Folder (`solidcraft_engine::library`).
//!
//! The page lists the library folder as the system's file manager shows it. Listing reads one
//! folder; a search walks the library. Both are read again only when the folder, the search or
//! the files change (after an action here, or every few seconds while the page is shown), never
//! every frame.

use std::path::{Path, PathBuf};

use solidcraft_engine::library::{self, Entry};

use crate::SolveApp;
use crate::ui_kit::{self, FileView};

/// How often the page reads its folder again while shown, for files changed outside Nuclear.
const REFRESH_SECONDS: f64 = 3.0;

/// The Projects page's state.
#[derive(Default)]
pub struct View {
    /// The folder shown, inside the library (`None`: the library itself).
    pub dir: Option<PathBuf>,
    /// The search text; empty lists the folder.
    pub query: String,
    /// What was last read: for which folder and search, when, and what it found.
    listed: Option<(PathBuf, String, f64, Result<Vec<Entry>, String>)>,
    /// A prompt open over the page.
    pub prompt: Option<Prompt>,
    /// The last action's outcome, shown under the toolbar (text, is an error).
    pub outcome: Option<(String, bool)>,
}

/// A question the page asks before it changes files.
#[derive(Clone, Debug)]
pub enum Prompt {
    /// A name for a new folder in the folder shown.
    NewFolder { name: String, error: Option<String> },
    /// A new name for the design or folder at `path`.
    Rename { path: PathBuf, name: String, error: Option<String> },
    /// Where to move the design or folder at `path`: the library's folders.
    Move { path: PathBuf, folders: Vec<PathBuf>, error: Option<String> },
}

/// The library folder, when it is ready to use.
fn root(app: &SolveApp) -> Option<PathBuf> {
    crate::storage_ui::dialog_folder(app)
}

/// The folder shown: the one picked, while it is still inside the library and there.
fn current(app: &SolveApp, root: &Path) -> PathBuf {
    app.home.library.dir.clone().filter(|d| d.starts_with(root) && d.is_dir()).unwrap_or_else(|| root.to_path_buf())
}

/// Read the folder or search again on the next frame.
pub fn refresh(app: &mut SolveApp) {
    app.home.library.listed = None;
}

fn entries(app: &mut SolveApp, root: &Path, dir: &Path, now: f64) -> Result<Vec<Entry>, String> {
    let query = app.home.library.query.trim().to_string();
    let fresh = app.home.library.listed.as_ref().is_some_and(|(d, q, at, _)| d == dir && *q == query && now - at < REFRESH_SECONDS);
    if !fresh {
        let found = if query.is_empty() { library::list(dir) } else { Ok(library::search(root, &query)) };
        app.home.library.listed = Some((dir.to_path_buf(), query, now, found));
    }
    app.home.library.listed.as_ref().map(|(_, _, _, e)| e.clone()).unwrap_or_else(|| Ok(Vec::new()))
}

/// Every folder in the library, the library first, for Move.
fn folders(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.to_path_buf()];
    let mut i = 0;
    while i < out.len() && out.len() < library::SEARCH_LIMIT {
        if let Ok(list) = library::list(&out[i]) {
            out.extend(list.into_iter().filter(|e| e.folder).map(|e| e.path));
        }
        i += 1;
    }
    out[1..].sort_by_key(|p| p.to_string_lossy().to_lowercase());
    out
}

/// A path as the page shows it: inside the library, from the library's name.
fn shown(root: &Path, path: &Path) -> String {
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match path.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => name,
        Ok(rel) => format!("{name}/{}", rel.to_string_lossy().replace('\\', "/")),
        Err(_) => path.display().to_string(),
    }
}

fn finish(app: &mut SolveApp, result: Result<String, String>) {
    app.home.library.outcome = Some(match result {
        Ok(text) => (text, false),
        Err(e) => (e, true),
    });
    refresh(app);
}

/// The open design was at `old`, which is now at `new`: it saves there from now on.
fn follow(app: &mut SolveApp, old: &Path, new: &Path) {
    let follow_one = |path: &mut Option<String>| {
        if let Some(p) = path.as_deref()
            && let Ok(rel) = Path::new(p).strip_prefix(old)
        {
            let moved = if rel.as_os_str().is_empty() { new.to_path_buf() } else { new.join(rel) };
            *path = Some(moved.to_string_lossy().into_owned());
        }
    };
    follow_one(&mut app.session.path);
    crate::documents::for_each_other_session(app, |s| follow_one(&mut s.path));
    for recent in app.home.recent.iter_mut() {
        if let Ok(rel) = Path::new(recent.as_str()).strip_prefix(old) {
            let moved = if rel.as_os_str().is_empty() { new.to_path_buf() } else { new.join(rel) };
            *recent = moved.to_string_lossy().into_owned();
        }
    }
}

/// The Projects page.
pub(crate) fn content(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    let Some(root) = root(app) else {
        let text = match crate::storage_ui::status(app) {
            None if crate::storage_ui::folder(app).is_some() => crate::i18n::tr("storage.checking"),
            _ => crate::i18n::tr("library.unavailable"),
        };
        ui_kit::text(ui, text, ui_kit::Text::Hint("library:unavailable"));
        if ui_kit::button(ui, crate::i18n::tr("storage.title"), ui_kit::Button::Normal).clicked() {
            app.storage.open = true;
        }
        return;
    };
    let dir = current(app, &root);
    let busy = app.file_picker_busy();
    // Where: each folder from the library down is a step back up.
    ui.horizontal_wrapped(|ui| {
        let mut steps = vec![root.clone()];
        if let Ok(rel) = dir.strip_prefix(&root) {
            let mut at = root.clone();
            for part in rel.components() {
                at = at.join(part.as_os_str());
                steps.push(at.clone());
            }
        }
        let last = steps.len() - 1;
        for (i, step) in steps.into_iter().enumerate() {
            if i > 0 {
                ui_kit::text(ui, "/", ui_kit::Text::Inline);
            }
            let label = step.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let response = ui_kit::button(ui, &label, if i == last { ui_kit::Button::SmallSelected } else { ui_kit::Button::Small });
            crate::scenario::publish_handle(&format!("library:crumb:{i}"), response.rect.center());
            if response.clicked() {
                app.home.library.dir = Some(step);
                app.home.library.query.clear();
                refresh(app);
            }
        }
    });
    ui.add_space(t.space_8);
    ui.horizontal(|ui| {
        let new_folder = ui_kit::icon_label_button(ui, crate::i18n::tr_label("New Folder"), "folder", !busy, false);
        crate::scenario::publish_handle("library:new_folder", new_folder.rect.center());
        if new_folder.clicked() {
            app.home.library.prompt = Some(Prompt::NewFolder { name: String::new(), error: None });
        }
        if app.services.show_in_folder.is_some()
            && ui_kit::icon_label_button(ui, crate::i18n::tr("storage.show_folder"), "folder", true, false).clicked()
            && let Some(show) = &app.services.show_in_folder
        {
            show(&dir.join("design.sc").display().to_string());
        }
        ui.add_space(t.space_8);
        let search = ui_kit::search_field(ui, egui::Id::new("library_search"), &mut app.home.library.query, crate::i18n::tr("library.search"));
        crate::scenario::publish_handle("library:search", search.rect.center());
    });
    if let Some((text, error)) = app.home.library.outcome.clone() {
        ui.add_space(t.space_6);
        ui_kit::text(ui, &text, if error { ui_kit::Text::Error("library:outcome") } else { ui_kit::Text::Note("library:outcome") });
    }
    ui.add_space(t.space_10);
    let now = ui.input(|i| i.time);
    let list = entries(app, &root, &dir, now);
    let searching = !app.home.library.query.trim().is_empty();
    let entries = match list {
        Ok(e) => e,
        Err(e) => {
            ui_kit::text(ui, &e, ui_kit::Text::Error("library:error"));
            return;
        }
    };
    if entries.is_empty() {
        let hint = if searching { crate::i18n::tr("library.no_results") } else { crate::i18n::tr("library.empty") };
        ui_kit::text(ui, hint, ui_kit::Text::Hint("library:empty"));
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(REFRESH_SECONDS));
        return;
    }
    let wall = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    for e in &entries {
        if !e.folder {
            crate::home::want_picture(&mut app.home, &e.path.to_string_lossy());
        }
    }
    let mut open: Option<Entry> = None;
    FileView::recent_list(ui, |ui| {
        for (index, e) in entries.iter().enumerate() {
            let key = e.path.to_string_lossy().into_owned();
            let path = shown(&root, e.path.parent().unwrap_or(&root));
            let when = if e.modified > 0 { crate::home::time_ago(app, e.modified, wall) } else { String::new() };
            let kind = if e.folder { crate::i18n::tr_label("Folder") } else { crate::i18n::tr_label("Design") };
            let id = format!("library:row:{index}");
            let response = FileView::recent_row(
                ui,
                &id,
                ui_kit::RecentRow {
                    title: &e.name,
                    path: &path,
                    when: &when,
                    workspace: kind,
                    thumbnail: if e.folder { None } else { crate::home::picture_of(app, &key) },
                    thumbnail_failed: e.folder,
                },
            );
            crate::scenario::publish_handle(&id, response.rect.center());
            crate::scenario::publish_rect(&id, response.rect);
            ui_kit::Menu::context(&response, |ui| {
                if crate::context_menu::button(ui, app.tr("ui.open"), if e.folder { "folder" } else { "open" }).clicked() {
                    open = Some(e.clone());
                    ui.close();
                }
                if crate::context_menu::button(ui, crate::i18n::tr_label("Rename"), "rename").clicked() {
                    app.home.library.prompt = Some(Prompt::Rename { path: e.path.clone(), name: e.name.clone(), error: None });
                    ui.close();
                }
                if crate::context_menu::button(ui, crate::i18n::tr_label("Move to Folder"), "move").clicked() {
                    app.home.library.prompt = Some(Prompt::Move { path: e.path.clone(), folders: folders(&root), error: None });
                    ui.close();
                }
                if crate::context_menu::button(ui, crate::i18n::tr_label("Duplicate"), "copy").clicked() {
                    let result = library::duplicate(&e.path).map(|to| crate::i18n::format("library.duplicated", &[("name", &file_label(&to))]));
                    finish(app, result);
                    ui.close();
                }
                if !e.folder && crate::context_menu::button_enabled(ui, crate::i18n::tr_label("Export Portable Copy"), "export", !busy).clicked() {
                    export_portable(app, e.path.clone());
                    ui.close();
                }
                if app.services.show_in_folder.is_some() && crate::context_menu::button(ui, app.tr_label("Show in folder"), "folder").clicked() {
                    if let Some(show) = &app.services.show_in_folder {
                        show(&key);
                    }
                    ui.close();
                }
            });
            if response.clicked() {
                open = Some(e.clone());
            }
        }
    });
    ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(REFRESH_SECONDS));
    if let Some(e) = open {
        if e.folder {
            app.home.library.dir = Some(e.path);
            app.home.library.query.clear();
            refresh(app);
        } else {
            crate::home::open_recent(app, &e.path.to_string_lossy());
        }
    }
}

/// A file's name as the page names it: a design without its extension.
fn file_label(path: &Path) -> String {
    let name = if library::is_design(path) { path.file_stem() } else { path.file_name() };
    name.map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Ask where, then copy the design with the designs it links to into a new folder there.
fn export_portable(app: &mut SolveApp, design: PathBuf) {
    app.pick_folder(move |app, picked| {
        let Some(dest) = picked.filter(|p| !p.trim().is_empty()) else { return };
        let result = library::export_portable(&design, Path::new(&dest)).map(|p| {
            let folder = p.folder.display().to_string();
            let mut text = crate::i18n::format("library.exported", &[("folder", &folder), ("count", &p.linked.to_string())]);
            if !p.missing.is_empty() {
                text.push(' ');
                text.push_str(&crate::i18n::format("library.exported_missing", &[("files", &p.missing.join(", "))]));
            }
            text
        });
        finish(app, result);
    });
}

/// The page's prompts: New Folder, Rename and Move.
pub(crate) fn show_prompt(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(prompt) = app.home.library.prompt.clone() else { return };
    let Some(root) = root(app) else {
        app.home.library.prompt = None;
        return;
    };
    let dir = current(app, &root);
    match prompt {
        Prompt::NewFolder { mut name, error } => {
            let (closed, action) = ui_kit::Confirm::new(egui::Id::new("sc_library_new_folder"), "folder", crate::i18n::tr_label("New Folder"))
                .actions("Create", "Cancel", None)
                .show(ctx, !name.trim().is_empty(), |ui| {
                    ui_kit::text_field(ui, &mut name, crate::i18n::tr("library.name"));
                    if let Some(e) = &error {
                        ui_kit::text(ui, e, ui_kit::Text::Error("library:prompt_error"));
                    }
                });
            app.home.library.prompt = Some(Prompt::NewFolder { name: name.clone(), error });
            match action {
                ui_kit::Action::Ok => match library::new_folder(&dir, &name) {
                    Ok(_) => {
                        app.home.library.prompt = None;
                        refresh(app);
                    }
                    Err(e) => app.home.library.prompt = Some(Prompt::NewFolder { name, error: Some(e) }),
                },
                _ if closed || action == ui_kit::Action::Cancel => app.home.library.prompt = None,
                _ => {}
            }
        }
        Prompt::Rename { path, mut name, error } => {
            let (closed, action) = ui_kit::Confirm::new(egui::Id::new("sc_library_rename"), "rename", crate::i18n::tr_label("Rename"))
                .actions("Rename", "Cancel", None)
                .show(ctx, !name.trim().is_empty(), |ui| {
                    ui_kit::text_field(ui, &mut name, crate::i18n::tr("library.name"));
                    ui_kit::text(ui, crate::i18n::tr("library.rename_keeps"), ui_kit::Text::Note("library:rename_keeps"));
                    if let Some(e) = &error {
                        ui_kit::text(ui, e, ui_kit::Text::Error("library:prompt_error"));
                    }
                });
            app.home.library.prompt = Some(Prompt::Rename { path: path.clone(), name: name.clone(), error });
            match action {
                ui_kit::Action::Ok => match library::rename(&path, &name, &root) {
                    Ok(to) => {
                        app.home.library.prompt = None;
                        follow(app, &path, &to);
                        refresh(app);
                    }
                    Err(e) => app.home.library.prompt = Some(Prompt::Rename { path, name, error: Some(e) }),
                },
                _ if closed || action == ui_kit::Action::Cancel => app.home.library.prompt = None,
                _ => {}
            }
        }
        Prompt::Move { path, folders, error } => {
            let mut picked = None;
            let (closed, action) = ui_kit::Confirm::new(egui::Id::new("sc_library_move"), "move", crate::i18n::tr_label("Move to Folder"))
                .actions("", "Cancel", None)
                .show(ctx, true, |ui| {
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (i, folder) in folders.iter().enumerate() {
                            let here = path.parent() == Some(folder.as_path()) || folder.starts_with(path.as_path());
                            let response = ui_kit::button(ui, shown(&root, folder), ui_kit::Button::Enabled(!here));
                            crate::scenario::publish_handle(&format!("library:move:{i}"), response.rect.center());
                            if response.clicked() {
                                picked = Some(folder.clone());
                            }
                        }
                    });
                    if let Some(e) = &error {
                        ui_kit::text(ui, e, ui_kit::Text::Error("library:prompt_error"));
                    }
                });
            if let Some(dest) = picked {
                match library::move_to(&path, &dest, &root) {
                    Ok(to) => {
                        app.home.library.prompt = None;
                        follow(app, &path, &to);
                        let name = file_label(&to);
                        finish(app, Ok(crate::i18n::format("library.moved", &[("name", &name), ("folder", &shown(&root, &dest))])));
                    }
                    Err(e) => app.home.library.prompt = Some(Prompt::Move { path, folders, error: Some(e) }),
                }
            } else if closed || action == ui_kit::Action::Cancel {
                app.home.library.prompt = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn library(h: &mut crate::scenario::Harness, name: &str) -> PathBuf {
        let lib = solidcraft_engine::io::vfs::temp_dir().join(format!("nuclear-library-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib);
        std::fs::create_dir_all(&lib).unwrap();
        h.app.preferences.storage_folder = lib.display().to_string();
        crate::storage_ui::prepare(&mut h.app);
        crate::storage_ui::wait(&mut h.app);
        lib
    }

    /// The Projects page lists the library's folders and designs, searches them, and its prompts
    /// rename and move files; the open design follows its file.
    #[test]
    fn projects_page_lists_searches_renames_and_moves_real_files() {
        let mut h = crate::scenario::Harness::new();
        let lib = library(&mut h, "page");
        h.app.run("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
        let path = lib.join("Bracket.sc");
        h.app.run("file.save_as", json!({"path": path.display().to_string()})).unwrap();
        library::new_folder(&lib, "Clients").unwrap();
        h.app.home.open = true;
        h.app.home.section = crate::home::Section::Library;
        h.frames(3);
        assert!(crate::scenario::handle_at("library:row:1").is_some(), "the folder and the design are listed");

        h.app.home.library.query = "brack".into();
        h.frames(2);
        assert!(crate::scenario::handle_at("library:row:0").is_some() && crate::scenario::handle_at("library:row:1").is_none());
        h.app.home.library.query.clear();

        h.app.home.library.prompt = Some(Prompt::Rename { path: path.clone(), name: "Housing".into(), error: None });
        h.frames(2);
        h.step(&json!({"key": "Enter"})).unwrap();
        h.frames(2);
        let renamed = lib.join("Housing.sc");
        if h.app.home.library.prompt.is_some() {
            // The prompt's own OK, should Enter not reach it in the harness.
            h.step(&json!({"click": {"handle": "confirm:ok"}})).ok();
            h.frames(2);
        }
        assert!(renamed.is_file() && !path.exists(), "renamed on disk");
        assert_eq!(h.app.session.path.as_deref(), Some(renamed.to_string_lossy().as_ref()), "the open design saves to its new name");

        h.app.home.library.prompt = Some(Prompt::Move { path: renamed.clone(), folders: folders(&lib), error: None });
        h.frames(2);
        h.step(&json!({"click": {"handle": "library:move:1"}})).unwrap();
        h.frames(2);
        let moved = lib.join("Clients").join("Housing.sc");
        assert!(moved.is_file(), "moved into the folder");
        assert_eq!(h.app.session.path.as_deref(), Some(moved.to_string_lossy().as_ref()));
        assert!(h.app.home.library.prompt.is_none());
    }

    #[test]
    fn new_folder_prompt_refuses_bad_names_and_keeps_asking() {
        let mut h = crate::scenario::Harness::new();
        let lib = library(&mut h, "folder");
        h.app.home.open = true;
        h.app.home.section = crate::home::Section::Library;
        h.app.home.library.prompt = Some(Prompt::NewFolder { name: "a/b".into(), error: None });
        h.frames(2);
        h.step(&json!({"click": {"handle": "dialog:Create"}})).unwrap();
        h.frames(2);
        assert!(matches!(&h.app.home.library.prompt, Some(Prompt::NewFolder { error: Some(_), .. })), "the reason is shown");
        assert_eq!(library::list(&lib).unwrap().len(), 0);
    }
}
