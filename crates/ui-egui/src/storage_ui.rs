//! The project library: where new designs are saved, and the Storage window beside Start.
//!
//! The library is Documents/Nuclear Projects unless the user picks another folder, such as one
//! a sync app keeps in step with the cloud (`solidcraft_engine::storage`). The first launch
//! creates the default library; a folder the user picked is only checked, never created, so a
//! sync folder or drive that is not there reads as unavailable instead of being made again
//! somewhere else. Checks run off the window thread: a disconnected network folder can take
//! seconds to answer.

use std::path::PathBuf;

use solidcraft_engine::storage::{self, Kind, Status};
use solidcraft_engine::versions;

use crate::SolveApp;
use crate::ui_kit;

/// The library's state, and the Storage window's.
#[derive(Default)]
pub struct Storage {
    /// The Documents folder, as the host's system API reported it (Windows); `None`: ask
    /// [`storage::documents_dir`].
    pub documents: Option<PathBuf>,
    /// The Storage window is open.
    pub open: bool,
    /// The last check of the library: the folder it was for, and what it found.
    pub status: Option<(PathBuf, Status)>,
    /// Why the library could not be prepared, for the window to show.
    pub error: Option<String>,
    /// The Advanced details are shown.
    advanced: bool,
    /// What the retention setting would remove from the open design's versions: for which
    /// design and setting, and the version numbers. Read only when one of them changes.
    impact: Option<(String, u32, Vec<u32>)>,
    /// The Version History window, when open.
    pub versions: Option<VersionsView>,
    #[cfg(not(target_arch = "wasm32"))]
    checking: Option<std::sync::mpsc::Receiver<(PathBuf, Status, Option<String>)>>,
}

/// The Version History window: the open design's versions, as last read.
pub struct VersionsView {
    path: String,
    list: Vec<solidcraft_engine::versions::Version>,
    error: Option<String>,
}

/// The Documents folder: the host's answer, else the engine's.
pub fn documents(app: &SolveApp) -> Option<PathBuf> {
    app.storage.documents.clone().or_else(storage::documents_dir)
}

/// The library new designs are saved in: the folder the user picked, else
/// Documents/Nuclear Projects. `None` when no Documents folder is known and none was picked.
pub fn folder(app: &SolveApp) -> Option<PathBuf> {
    let picked = app.preferences.storage_folder.trim();
    if picked.is_empty() { documents(app).map(|d| storage::library_dir(&d)) } else { Some(PathBuf::from(picked)) }
}

pub fn kind(app: &SolveApp) -> Kind {
    Kind::parse(&app.preferences.storage_kind)
}

/// Whether the library is the default one (Documents/Nuclear Projects).
pub fn is_default(app: &SolveApp) -> bool {
    app.preferences.storage_folder.trim().is_empty()
}

/// Make the library ready (at launch, and after the user picks a folder): the default library is
/// created when missing; a picked folder is only checked. Runs on a worker thread.
pub fn prepare(app: &mut SolveApp) {
    let Some(dir) = folder(app) else {
        app.storage.error = Some(crate::i18n::tr("storage.no_documents").to_string());
        return;
    };
    let create = is_default(app);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = dir.clone();
        let started = std::thread::Builder::new().name("storage-check".into()).spawn(move || {
            let _ = tx.send(check_library(worker, create));
        });
        match started {
            Ok(_) => app.storage.checking = Some(rx),
            Err(_) => {
                let (dir, status, error) = check_library(dir, create);
                finish(app, dir, status, error);
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let (dir, status, error) = check_library(dir, create);
        finish(app, dir, status, error);
    }
}

/// Prepare (`create`) or only check the library at `dir`: the folder, its status, and why it
/// could not be prepared.
fn check_library(dir: PathBuf, create: bool) -> (PathBuf, Status, Option<String>) {
    if !create {
        let status = storage::check(&dir);
        return (dir, status, None);
    }
    match storage::prepare(&dir) {
        Ok(_) => (dir, Status::Ready, None),
        Err(e) => {
            let status = storage::check(&dir);
            (dir, status, Some(e))
        }
    }
}

/// Wait for the check under way (tests, and the moment before the window needs its answer).
pub fn wait(app: &mut SolveApp) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rx) = app.storage.checking.take()
        && let Ok((dir, status, error)) = rx.recv()
    {
        finish(app, dir, status, error);
    }
}

fn poll(app: &mut SolveApp) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rx) = &app.storage.checking {
        match rx.try_recv() {
            Ok((dir, status, error)) => {
                app.storage.checking = None;
                finish(app, dir, status, error);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => app.storage.checking = None,
        }
    }
}

fn finish(app: &mut SolveApp, dir: PathBuf, status: Status, error: Option<String>) {
    // A check for a folder the user has since changed away from says nothing about the new one.
    if folder(app).as_ref() != Some(&dir) {
        return;
    }
    if let Some(e) = &error {
        log::warn!("storage: {e}");
        app.set_status(crate::i18n::format("storage.prepare_failed", &[("reason", e)]), true);
    }
    app.storage.error = error;
    app.storage.status = Some((dir, status));
}

/// The library's current status, once known.
pub fn status(app: &SolveApp) -> Option<&Status> {
    let dir = folder(app)?;
    app.storage.status.as_ref().filter(|(d, _)| *d == dir).map(|(_, s)| s)
}

/// The folder a save or open dialog starts in when nothing else chose one: the library, when
/// it is ready.
pub fn dialog_folder(app: &SolveApp) -> Option<PathBuf> {
    status(app).filter(|s| s.is_ready()).and(folder(app))
}

/// The name a new design is given: `Untitled`, or `Untitled 2`, `Untitled 3`, ... when that
/// name is already an open design's, or a design's in the library, so two new designs never
/// offer to save over each other.
pub fn provisional_name(app: &SolveApp) -> String {
    let open: Vec<String> = crate::documents::tabs_info(app).into_iter().map(|(name, _)| name).collect();
    let library = dialog_folder(app);
    let taken = |name: &str| {
        open.iter().filter(|n| n.as_str() == name).count() > usize::from(app.session.doc.name == name)
            || library.as_ref().is_some_and(|dir| solidcraft_engine::io::DESIGN_EXTS.iter().any(|ext| dir.join(format!("{name}.{ext}")).exists()))
    };
    (1..1000u32)
        .map(|n| if n == 1 { "Untitled".to_string() } else { format!("Untitled {n}") })
        .find(|name| !taken(name))
        .unwrap_or_else(|| "Untitled".into())
}

/// A new design (the active one, still blank and unsaved) gets its stable identity and a name
/// no open design or library design has. Neither counts as a change to save.
pub fn name_new_design(app: &mut SolveApp) {
    if !crate::documents::active_is_blank(app) {
        return;
    }
    let name = provisional_name(app);
    let doc = std::sync::Arc::make_mut(&mut app.session.doc);
    let mut changed = solidcraft_engine::library::ensure_document_id(doc);
    if doc.name.starts_with("Untitled") && doc.name != name {
        doc.name = name;
        changed = true;
    }
    if changed {
        app.session.mark_saved();
    }
}

/// Use `dir` as the library, of `kind`. Only where new designs go changes: nothing is moved.
fn set_folder(app: &mut SolveApp, dir: Option<PathBuf>, kind: Kind) {
    app.preferences.storage_folder = dir.map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    app.preferences.storage_kind = kind.as_str().into();
    app.storage.status = None;
    app.storage.error = None;
    prepare(app);
}

/// Ask for a folder and make it the library, of `kind`. Cancelling keeps the library as it was.
fn choose_folder(app: &mut SolveApp, kind: Kind) {
    app.pick_folder(move |app, picked| {
        if let Some(dir) = picked.filter(|p| !p.trim().is_empty()) {
            set_folder(app, Some(PathBuf::from(dir)), kind);
        }
    });
}

/// The button beside the Start title: what kind of folder the library is; opens the window.
/// The browser build keeps designs in the browser and has no library.
pub(crate) fn button(app: &mut SolveApp, ui: &mut egui::Ui) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    let label = match kind(app) {
        Kind::Local => crate::i18n::tr("storage.button_local"),
        Kind::Synced => crate::i18n::tr("storage.button_synced"),
    };
    let response = ui_kit::icon_label_button(ui, label, "folder", true, false);
    let tip = folder(app).map(|d| d.display().to_string()).unwrap_or_default();
    let response = if tip.is_empty() { response } else { response.on_hover_text(tip) };
    crate::scenario::publish_handle("home:storage", response.rect.center());
    crate::scenario::publish_rect("home:storage", response.rect);
    if response.clicked() {
        app.storage.open = !app.storage.open;
    }
}

/// What lowering the retention would remove from the open design's history at its next save,
/// shown before it happens. Read from disk only when the design or the setting changes.
fn retention_impact(app: &mut SolveApp) -> Option<String> {
    let path = app.session.path.clone()?;
    let keep = app.preferences.versions_keep;
    let fresh = app.storage.impact.as_ref().is_some_and(|(p, k, _)| *p == path && *k == keep);
    if !fresh {
        let retention = if keep == 0 { versions::Retention::All } else { versions::Retention::Last(keep) };
        let gone = versions::History::of(std::path::Path::new(&path)).and_then(|h| h.plan_prune(retention)).unwrap_or_default();
        app.storage.impact = Some((path, keep, gone));
    }
    let (_, _, gone) = app.storage.impact.as_ref()?;
    if gone.is_empty() {
        return None;
    }
    let list = gone.iter().map(|n| format!("V{n}")).collect::<Vec<_>>().join(", ");
    Some(crate::i18n::format("storage.versions_impact", &[("versions", &list)]))
}

/// Open the Version History window on the open design's file.
pub(crate) fn open_versions(app: &mut SolveApp) {
    let Some(path) = app.session.path.clone() else { return };
    let mut view = VersionsView { path, list: Vec::new(), error: None };
    reload(&mut view);
    app.storage.versions = Some(view);
}

fn reload(view: &mut VersionsView) {
    match versions::History::of(std::path::Path::new(&view.path)).and_then(|h| h.list()) {
        Ok(list) => (view.list, view.error) = (list, None),
        Err(e) => (view.list, view.error) = (Vec::new(), Some(e.to_string())),
    }
}

/// The Version History window: each version, newest first, with Protect, Open and Restore.
fn show_versions(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(view) = app.storage.versions.as_ref() else { return };
    // Another design opened since: the window was about that one.
    let current = app.session.path.clone();
    let reading = current.as_deref() == Some(view.path.as_str());
    let list = view.list.clone();
    let error = view.error.clone();
    let name = std::path::Path::new(&view.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dirty = app.session.is_dirty();
    let mut act: Option<(&'static str, serde_json::Value)> = None;
    let title = format!("{} · {name}", crate::i18n::tr("cmd.file_versions"));
    let (closed, action) = ui_kit::Window::new(egui::Id::new("sc_versions"), "restore", &title)
        .cards(2.0)
        .actions("", crate::i18n::tr("ui.close"), None)
        .show(ctx, true, |ui| {
            ui.spacing_mut().item_spacing.y = crate::theme::Tokens::get().space_6;
            if let Some(e) = &error {
                ui_kit::text(ui, e, ui_kit::Text::Error("versions:error"));
            }
            if list.is_empty() && error.is_none() {
                ui_kit::empty_box(ui, "versions:none", crate::i18n::tr("versions.none"));
            }
            for v in list.iter().rev() {
                ui.push_id(v.number, |ui| {
                    ui.horizontal(|ui| {
                        let mut line = format!("V{} · {}", v.number, crate::recovery_ui::ago(v.saved_at));
                        if let Some(from) = v.restored_from {
                            line.push_str(" · ");
                            line.push_str(&crate::i18n::format("versions.restored_from", &[("version", &format!("V{from}"))]));
                        }
                        if v.protected {
                            line.push_str(" · ");
                            line.push_str(crate::i18n::tr("versions.protected"));
                        }
                        ui_kit::text(ui, &line, ui_kit::Text::Inline);
                        let protect = if v.protected { "versions.unprotect" } else { "versions.protect" };
                        if ui_kit::button(ui, crate::i18n::tr(protect), ui_kit::Button::Small).clicked() {
                            act = Some(("file.version_protect", serde_json::json!({"number": v.number, "protected": !v.protected})));
                        }
                        if ui_kit::button(ui, crate::i18n::tr("versions.open"), ui_kit::Button::Small).clicked() {
                            act = Some(("file.version_open", serde_json::json!({"number": v.number})));
                        }
                        let restore = ui_kit::button(ui, crate::i18n::tr("versions.restore"), ui_kit::Button::Enabled(reading && !dirty));
                        let restore = if dirty { restore.on_disabled_hover_text(crate::i18n::tr("versions.restore_needs_save")) } else { restore };
                        if restore.clicked() {
                            act = Some(("file.version_restore", serde_json::json!({"number": v.number, "keep": app.preferences.versions_keep})));
                        }
                    });
                });
            }
        });
    if let Some((command, mut params)) = act {
        let path = app.storage.versions.as_ref().map(|v| v.path.clone()).unwrap_or_default();
        if command != "file.version_restore" {
            params["path"] = path.into();
        }
        if let Err(e) = app.run(command, params)
            && let Some(view) = app.storage.versions.as_mut()
        {
            view.error = Some(e);
        }
        if command == "file.version_open" {
            app.storage.versions = None;
            app.storage.open = false;
            app.home.open = false;
            return;
        }
        if let Some(view) = app.storage.versions.as_mut() {
            reload(view);
        }
        app.storage.impact = None;
    }
    if closed || matches!(action, ui_kit::Action::Cancel) || !reading {
        app.storage.versions = None;
    }
}

/// The Storage window.
pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    poll(app);
    show_versions(app, ctx);
    crate::library_ui::show_prompt(app, ctx);
    if !app.storage.open {
        return;
    }
    let t = crate::theme::Tokens::get();
    let mut recover = false;
    let title = crate::i18n::tr("storage.title").to_string();
    let (closed, action) = ui_kit::Window::new(egui::Id::new("sc_storage"), "folder", &title)
        .cards(2.0)
        .actions("", crate::i18n::tr("ui.close"), None)
        .show(ctx, true, |ui| {
            ui.spacing_mut().item_spacing.y = t.space_10;
            ui_kit::text(ui, crate::i18n::tr("storage.intro"), ui_kit::Text::Hint("storage:intro"));
            let dir = folder(app);
            match &dir {
                Some(d) => {
                    let path = d.display().to_string();
                    // The whole path, wrapped: a chip would cut it short.
                    ui_kit::text(ui, &path, ui_kit::Text::MonoNote("storage:path"));
                    ui.horizontal(|ui| {
                        if ui_kit::icon_label_button(ui, crate::i18n::tr("crash.copy_folder"), "copy", true, false).clicked() {
                            ctx.copy_text(path.clone());
                        }
                        let ready = status(app).is_some_and(Status::is_ready);
                        if app.services.show_in_folder.is_some()
                            && ui_kit::icon_label_button(ui, crate::i18n::tr("storage.show_folder"), "folder", ready, false).clicked()
                            && let Some(show) = &app.services.show_in_folder
                        {
                            // `show_in_folder` opens the parent of the path it is given, so the
                            // argument is a name inside the library (it need not exist).
                            show(&d.join("design.sc").display().to_string());
                        }
                    });
                }
                None => {
                    ui_kit::text(ui, crate::i18n::tr("storage.no_documents"), ui_kit::Text::Error("storage:no_documents"));
                }
            }
            let state = match status(app) {
                None if dir.is_some() => Some((crate::i18n::tr("storage.checking").to_string(), false)),
                None => None,
                Some(Status::Ready) => Some((crate::i18n::tr("storage.ready").to_string(), false)),
                Some(Status::Missing) => Some((crate::i18n::tr("storage.missing").to_string(), true)),
                Some(Status::NotAFolder) => Some((crate::i18n::tr("storage.not_a_folder").to_string(), true)),
                Some(Status::Unwritable(e)) => Some((crate::i18n::format("storage.unwritable", &[("reason", e)]), true)),
            };
            if let Some((text, error)) = state {
                ui_kit::text(ui, &text, if error { ui_kit::Text::Error("storage:status") } else { ui_kit::Text::Note("storage:status") });
            }
            if let Some(e) = app.storage.error.clone() {
                ui_kit::text(ui, &e, ui_kit::Text::Error("storage:error"));
            }

            // Local folder or synced folder.
            let mut selected = usize::from(kind(app) == Kind::Synced);
            let options = [crate::i18n::tr("storage.kind_local"), crate::i18n::tr("storage.kind_synced")];
            let targets = ["storage:local", "storage:synced"];
            if ui_kit::segmented(ui, &mut selected, &options, ui_kit::Segments { targets: &targets, ..Default::default() }) {
                let picked = if selected == 1 { Kind::Synced } else { Kind::Local };
                if picked == Kind::Synced && is_default(app) {
                    // A synced folder is one the sync app made: the user points at it.
                    choose_folder(app, Kind::Synced);
                } else {
                    app.preferences.storage_kind = picked.as_str().into();
                }
            }
            let explain = match kind(app) {
                Kind::Local => crate::i18n::tr("storage.explain_local"),
                Kind::Synced => crate::i18n::tr("storage.explain_synced"),
            };
            ui_kit::text(ui, explain, ui_kit::Text::Hint("storage:explain"));
            ui.horizontal(|ui| {
                let busy = app.file_picker_busy();
                if ui_kit::button(ui, crate::i18n::tr("storage.choose"), ui_kit::Button::Enabled(!busy)).clicked() {
                    choose_folder(app, kind(app));
                }
                if !is_default(app) && ui_kit::button(ui, crate::i18n::tr("storage.use_default"), ui_kit::Button::Normal).clicked() {
                    set_folder(app, None, Kind::Local);
                }
            });
            ui_kit::text(ui, crate::i18n::tr("storage.nothing_moves"), ui_kit::Text::Note("storage:nothing_moves"));

            // Autosave into the recovery folder (the existing preference, shown here too).
            ui_kit::text(ui, crate::i18n::tr("storage.autosave"), ui_kit::Text::Heading("storage:autosave"));
            ui.horizontal(|ui| {
                let mut on = app.autosave_minutes > 0.0;
                if ui_kit::checkbox(ui, crate::i18n::tr("storage.autosave_on"), &mut on).changed() {
                    app.autosave_minutes = if on { 5.0 } else { 0.0 };
                    crate::prefs::set_autosave(app);
                }
                if on {
                    if ui_kit::settings_number(ui, &mut app.autosave_minutes, 0.5..=120.0, 0.5).changed() {
                        crate::prefs::set_autosave(app);
                    }
                    ui_kit::text(ui, crate::i18n::tr("prefs.minutes"), ui_kit::Text::Inline);
                }
            });
            ui_kit::text(ui, crate::i18n::tr("storage.autosave_note"), ui_kit::Text::Note("storage:autosave_note"));
            let label = if app.recoverable.is_empty() {
                crate::i18n::tr("storage.recover").to_string()
            } else {
                format!("{} ({})", crate::i18n::tr("storage.recover"), app.recoverable.len())
            };
            if ui_kit::button(ui, &label, ui_kit::Button::Normal).clicked() {
                recover = true;
            }

            // Permanent versions: made by Save, kept by this setting; autosave never makes one.
            ui_kit::text(ui, crate::i18n::tr("storage.versions"), ui_kit::Text::Heading("storage:versions"));
            ui.horizontal(|ui| {
                ui_kit::text(ui, crate::i18n::tr("storage.keep_versions"), ui_kit::Text::Inline);
                ui_kit::settings_number(ui, &mut app.preferences.versions_keep, 0..=1000, 1.0);
            });
            ui_kit::text(ui, crate::i18n::tr("storage.versions_note"), ui_kit::Text::Note("storage:versions_note"));
            if let Some(text) = retention_impact(app) {
                ui_kit::text(ui, &text, ui_kit::Text::Error("storage:versions_impact"));
            }
            let has_file = app.session.path.is_some();
            let history = ui_kit::button(ui, crate::i18n::tr("storage.version_history"), ui_kit::Button::Enabled(has_file));
            crate::scenario::publish_handle("storage:version_history", history.rect.center());
            if history.clicked() {
                open_versions(app);
            }

            ui_kit::checkbox(ui, crate::i18n::tr("storage.advanced"), &mut app.storage.advanced);
            if app.storage.advanced {
                let recovery = app.autosave.as_ref().map(|a| a.dir.clone()).or_else(solidcraft_engine::recovery::default_dir);
                let rows = [
                    ("storage.documents_folder", documents(app)),
                    ("storage.recovery_folder", recovery),
                    ("storage.crash_folder", solidcraft_engine::crash::default_dir()),
                ];
                for (id, path) in rows {
                    let path = path.map(|p| p.display().to_string()).unwrap_or_else(|| "—".into());
                    ui_kit::text(ui, &format!("{}: {path}", crate::i18n::tr(id)), ui_kit::Text::MonoNote(id));
                }
            }
        });
    if closed || matches!(action, ui_kit::Action::Cancel) {
        app.storage.open = false;
    }
    if recover {
        app.storage.open = false;
        crate::home::show_recovery(app, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_documents(name: &str) -> (SolveApp, PathBuf) {
        let docs = solidcraft_engine::io::vfs::temp_dir().join(format!("nuclear-storage-ui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&docs);
        std::fs::create_dir_all(&docs).unwrap();
        let mut app = SolveApp::new(solidcraft_engine::Session::default(), Default::default());
        app.storage.documents = Some(docs.clone());
        (app, docs)
    }

    #[test]
    fn the_default_library_is_created_in_documents_and_offered_to_dialogs() {
        let (mut app, docs) = app_with_documents("default");
        let lib = docs.join(storage::LIBRARY_NAME);
        assert_eq!(folder(&app), Some(lib.clone()));
        assert_eq!(dialog_folder(&app), None, "not offered before it is known to be ready");
        prepare(&mut app);
        wait(&mut app);
        assert!(lib.is_dir());
        assert_eq!(status(&app), Some(&Status::Ready));
        assert_eq!(dialog_folder(&app), Some(lib));
        std::fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn a_picked_folder_that_is_gone_is_unavailable_and_not_recreated() {
        let (mut app, docs) = app_with_documents("gone");
        let synced = docs.join("Google Drive").join("Projects");
        set_folder(&mut app, Some(synced.clone()), Kind::Synced);
        wait(&mut app);
        assert_eq!(kind(&app), Kind::Synced);
        assert_eq!(status(&app), Some(&Status::Missing));
        assert!(!synced.exists());
        assert_eq!(dialog_folder(&app), None);
        assert!(!docs.join(storage::LIBRARY_NAME).exists(), "the default library is not made either");
        std::fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn changing_the_library_moves_nothing_and_survives_a_restart() {
        let (mut app, docs) = app_with_documents("change");
        prepare(&mut app);
        wait(&mut app);
        let lib = docs.join(storage::LIBRARY_NAME);
        std::fs::write(lib.join("bracket.sc"), b"design").unwrap();
        let other = docs.join("Synced");
        std::fs::create_dir_all(&other).unwrap();
        set_folder(&mut app, Some(other.clone()), Kind::Synced);
        wait(&mut app);
        assert_eq!(status(&app), Some(&Status::Ready));
        assert_eq!(std::fs::read(lib.join("bracket.sc")).unwrap(), b"design", "the old library keeps its designs");
        assert_eq!(std::fs::read_dir(&other).unwrap().count(), 0, "nothing was copied or moved");

        let saved = app.prefs();
        let mut again = SolveApp::new(solidcraft_engine::Session::default(), Default::default());
        again.storage.documents = Some(docs.clone());
        again.load_prefs(&saved);
        assert_eq!(folder(&again), Some(other));
        assert_eq!(kind(&again), Kind::Synced);

        set_folder(&mut app, None, Kind::Local);
        wait(&mut app);
        assert_eq!(folder(&app), Some(lib));
        assert_eq!(kind(&app), Kind::Local);
        std::fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn save_makes_a_version_autosave_does_not_and_lowering_retention_shows_its_cost() {
        let (mut app, docs) = app_with_documents("versions");
        let file = docs.join("Bracket.sc");
        app.run("solid.box", serde_json::json!({"length":10,"width":10,"height":10})).unwrap();
        crate::save_job::start(&mut app, "file.save_as", serde_json::json!({"path": file}));
        crate::save_job::tick(&mut app, true);
        app.run("solid.box", serde_json::json!({"length":5,"width":5,"height":5})).unwrap();
        // An autosave of the change: a recovery checkpoint only.
        let recovery = docs.join("recovery");
        let mut saver = solidcraft_engine::recovery::Autosaver::new(&recovery, std::time::Duration::ZERO).unwrap();
        assert!(saver.save(&app.session).unwrap());
        let history = versions::History::of(&file).unwrap();
        assert_eq!(history.list().unwrap().len(), 1, "autosave made no version");
        crate::save_job::start(&mut app, "file.save", serde_json::json!({}));
        crate::save_job::tick(&mut app, true);
        assert_eq!(history.list().unwrap().iter().map(|v| v.number).collect::<Vec<_>>(), [1, 2]);
        assert!(!app.session.is_dirty());

        app.preferences.versions_keep = 1;
        let impact = retention_impact(&mut app).unwrap();
        assert!(impact.contains("V1"), "{impact}");
        assert_eq!(history.list().unwrap().len(), 2, "showing the cost removes nothing");
        app.preferences.versions_keep = 0;
        assert_eq!(retention_impact(&mut app), None);

        open_versions(&mut app);
        assert_eq!(app.storage.versions.as_ref().unwrap().list.len(), 2);
        saver.discard();
        std::fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn no_documents_folder_asks_for_a_folder() {
        let mut app = SolveApp::new(solidcraft_engine::Session::default(), Default::default());
        app.storage.documents = None;
        app.preferences.storage_folder.clear();
        if storage::documents_dir().is_none() {
            prepare(&mut app);
            assert!(app.storage.error.is_some());
            assert_eq!(dialog_folder(&app), None);
        }
    }

    /// A new design gets a stable identity and a name that no open design and no design in the
    /// library has; neither is a change to save, and a rename keeps the identity.
    #[test]
    fn new_designs_get_an_identity_and_a_name_no_other_design_has() {
        let (mut app, docs) = app_with_documents("provisional");
        prepare(&mut app);
        wait(&mut app);
        let lib = storage::library_dir(&docs);
        std::fs::write(lib.join("Untitled.sc"), b"saved before").unwrap();
        crate::documents::new_design(&mut app);
        assert_eq!(app.session.doc.name, "Untitled 2", "the library already has Untitled");
        let first = solidcraft_engine::library::document_id(&app.session.doc).map(str::to_string);
        assert!(first.is_some());
        assert!(!app.session.is_dirty(), "naming a new design is not a change");
        app.run("solid.box", serde_json::json!({"length":1,"width":1,"height":1})).unwrap();
        crate::documents::new_tab(&mut app);
        assert_eq!(app.session.doc.name, "Untitled 3", "another tab is Untitled 2");
        let second = solidcraft_engine::library::document_id(&app.session.doc).map(str::to_string);
        assert!(second.is_some() && second != first, "each design is its own document");
        assert_eq!(
            crate::i18n::Locale::for_language("pt-BR").given_name("Untitled 3"),
            format!("{} 3", crate::i18n::Locale::for_language("pt-BR").text("ui.untitled"))
        );
        let path = lib.join("Bracket.sc");
        app.run("file.save_as", serde_json::json!({"path": path.display().to_string()})).unwrap();
        let renamed = solidcraft_engine::library::rename(&path, "Housing", &lib).unwrap();
        let saved = solidcraft_engine::io::read_design(&std::fs::read(renamed).unwrap()).unwrap();
        assert_eq!(solidcraft_engine::library::document_id(&saved).map(str::to_string), second, "saving and renaming keep the identity");
    }
}
