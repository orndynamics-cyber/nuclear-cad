//! Where new designs are saved: the project library.
//!
//! By default the library is a folder named [`LIBRARY_NAME`] in the user's Documents folder, as
//! the operating system reports it (on Windows the known-folder API, which follows a Documents
//! folder redirected to OneDrive; the desktop app passes that path in). The user can instead pick
//! any folder, such as one a Google Drive or OneDrive app keeps in sync. Nuclear only ever writes
//! files there through the file system: the provider's own app does the syncing, so Nuclear never
//! claims a design reached the cloud.
//!
//! Preparing the library is idempotent: a missing folder is created, an existing one is reused
//! as it is, and nothing in it is moved, replaced or deleted. Changing the library only changes
//! where new designs go; designs already saved stay where they are.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The library folder's name inside Documents.
pub const LIBRARY_NAME: &str = "Nuclear Projects";

/// What kind of folder the library is. Only a label for the user and for what Nuclear promises:
/// both are written the same way, through the file system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A folder on this computer (which may still be synced by something else).
    #[default]
    Local,
    /// A folder a sync app (Google Drive, OneDrive, Dropbox, ...) keeps in step with the cloud.
    Synced,
}

impl Kind {
    /// Its stored name: `local` or `synced`; anything else reads as local.
    pub fn parse(s: &str) -> Kind {
        if s == "synced" { Kind::Synced } else { Kind::Local }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Local => "local",
            Kind::Synced => "synced",
        }
    }
}

/// Whether the library can take new designs right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// There and writable.
    Ready,
    /// Not there (a disconnected drive, a sync folder that was removed, never created).
    Missing,
    /// Something that is not a folder has the library's name.
    NotAFolder,
    /// There, but a file cannot be written in it (the reason as the system gave it).
    Unwritable(String),
}

impl Status {
    pub fn is_ready(&self) -> bool {
        matches!(self, Status::Ready)
    }
}

/// The Documents folder on systems where no native API is needed: `SOLIDCRAFT_DOCUMENTS_DIR`
/// (tests, portable setups), else on macOS `~/Documents` (what the system's API answers there),
/// else on Linux and the BSDs `XDG_DOCUMENTS_DIR` from `user-dirs.dirs`, else an existing
/// `~/Documents`, else the home folder (the XDG fallback). Windows: the desktop app asks the known-folder API and passes the answer
/// to [`library_dir`]; this answers only the override there.
pub fn documents_dir() -> Option<PathBuf> {
    if let Some(d) = solidcraft_geom::env::var_os("DOCUMENTS_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    if cfg!(windows) || cfg!(target_arch = "wasm32") {
        return None;
    }
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)?;
    if cfg!(target_os = "macos") {
        return Some(home.join("Documents"));
    }
    let config = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    let text = fs::read_to_string(config.join("user-dirs.dirs")).unwrap_or_default();
    // Without xdg-user-dirs, a Documents folder already in the home folder is the one people
    // use; else the home folder itself, as the XDG default says.
    Some(xdg_documents(&text, &home).unwrap_or_else(|| Some(home.join("Documents")).filter(|d| d.is_dir()).unwrap_or(home)))
}

/// `XDG_DOCUMENTS_DIR` from the text of a `user-dirs.dirs` file: `"$HOME/..."` or an absolute
/// path, as the xdg-user-dirs format allows.
fn xdg_documents(text: &str, home: &Path) -> Option<PathBuf> {
    let value = text.lines().map(str::trim).find_map(|l| l.strip_prefix("XDG_DOCUMENTS_DIR="))?;
    let value = value.trim().trim_matches('"');
    if let Some(rest) = value.strip_prefix("$HOME") {
        let rest = rest.trim_start_matches('/');
        // "$HOME/" means the home folder itself.
        return Some(if rest.is_empty() { home.to_path_buf() } else { home.join(rest) });
    }
    value.starts_with('/').then(|| PathBuf::from(value))
}

/// The default library: [`LIBRARY_NAME`] inside `documents`.
pub fn library_dir(documents: &Path) -> PathBuf {
    documents.join(LIBRARY_NAME)
}

/// Whether `dir` can take new designs. Writes, then removes, one small probe file with a name
/// of its own, created only if no file has that name: nothing already there is touched.
pub fn check(dir: &Path) -> Status {
    // The browser build has no file system (and no process id to name the probe with).
    if cfg!(target_arch = "wasm32") {
        return Status::Unwritable("no file system in the browser".into());
    }
    match fs::metadata(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Status::Missing,
        Err(e) => return Status::Unwritable(e.to_string()),
        Ok(m) if !m.is_dir() => return Status::NotAFolder,
        Ok(_) => {}
    }
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let probe = dir.join(format!(".nuclear-write-check-{}-{seq}", std::process::id()));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(&probe);
            Status::Ready
        }
        Err(e) => Status::Unwritable(e.to_string()),
    }
}

/// What [`prepare`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// The folder was made now (false: an existing folder was reused as it is).
    pub created: bool,
}

/// Make the library ready: create `dir` (and its missing parents) when it is not there, reuse it
/// when it is. Never deletes, moves or replaces anything. The error says why the folder cannot be
/// used, for the user to pick another one.
pub fn prepare(dir: &Path) -> std::result::Result<Prepared, String> {
    let created = match fs::metadata(dir) {
        Ok(m) if m.is_dir() => false,
        Ok(_) => return Err(format!("{} is a file, not a folder", dir.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
            true
        }
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    match check(dir) {
        Status::Ready => Ok(Prepared { created }),
        Status::Missing => Err(format!("{} is not there", dir.display())),
        Status::NotAFolder => Err(format!("{} is a file, not a folder", dir.display())),
        Status::Unwritable(e) => Err(format!("cannot write in {}: {e}", dir.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = solidcraft_io::vfs::temp_dir().join(format!("nuclear-storage-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn first_run_creates_the_library_inside_documents() {
        let docs = scratch("first");
        let lib = library_dir(&docs);
        assert_eq!(lib.file_name().unwrap(), LIBRARY_NAME);
        assert!(!lib.exists());
        assert_eq!(prepare(&lib), Ok(Prepared { created: true }));
        assert!(lib.is_dir());
        assert_eq!(fs::read_dir(&lib).unwrap().count(), 0, "the write check leaves nothing behind");
        fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn a_second_run_keeps_everything_already_in_the_library() {
        let docs = scratch("second");
        let lib = library_dir(&docs);
        prepare(&lib).unwrap();
        fs::write(lib.join("bracket.sc"), b"design").unwrap();
        fs::create_dir(lib.join("Customer A")).unwrap();
        fs::write(lib.join("Customer A").join("notes.txt"), b"unknown").unwrap();
        assert_eq!(prepare(&lib), Ok(Prepared { created: false }));
        assert_eq!(fs::read(lib.join("bracket.sc")).unwrap(), b"design");
        assert_eq!(fs::read(lib.join("Customer A").join("notes.txt")).unwrap(), b"unknown");
        assert_eq!(fs::read_dir(&lib).unwrap().count(), 2);
        fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn a_file_in_the_way_is_reported_not_replaced() {
        let docs = scratch("file");
        let lib = library_dir(&docs);
        fs::write(&lib, b"keep me").unwrap();
        assert!(prepare(&lib).unwrap_err().contains("not a folder"));
        assert_eq!(check(&lib), Status::NotAFolder);
        assert_eq!(fs::read(&lib).unwrap(), b"keep me");
        fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn an_unavailable_folder_reads_as_missing_without_being_created() {
        let docs = scratch("missing");
        let gone = docs.join("Drive that is not mounted").join("Projects");
        assert_eq!(check(&gone), Status::Missing);
        assert!(!gone.exists(), "checking never creates");
        fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn documents_come_from_xdg_user_dirs() {
        let home = Path::new("/home/ana");
        let text = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOCUMENTS_DIR=\"$HOME/Documentos\"\n";
        assert_eq!(xdg_documents(text, home), Some(PathBuf::from("/home/ana/Documentos")));
        assert_eq!(xdg_documents("XDG_DOCUMENTS_DIR=\"/data/docs\"", home), Some(PathBuf::from("/data/docs")));
        assert_eq!(xdg_documents("XDG_DOCUMENTS_DIR=\"$HOME/\"", home), Some(PathBuf::from("/home/ana")));
        assert_eq!(xdg_documents("XDG_DOCUMENTS_DIR=\"relative\"", home), None);
        assert_eq!(xdg_documents("", home), None);
    }

    #[test]
    fn kinds_round_trip_and_unknown_reads_as_local() {
        for k in [Kind::Local, Kind::Synced] {
            assert_eq!(Kind::parse(k.as_str()), k);
        }
        assert_eq!(Kind::parse("google_drive_api"), Kind::Local);
    }
}
