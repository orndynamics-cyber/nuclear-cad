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
    #[cfg(not(target_arch = "wasm32"))]
    checking: Option<std::sync::mpsc::Receiver<(PathBuf, Status, Option<String>)>>,
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

/// The Storage window.
pub fn show(app: &mut SolveApp, ctx: &egui::Context) {
    poll(app);
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
}
