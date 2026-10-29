//! Version history: durable snapshots of a design's file, kept beside it.
//!
//! A design `Bracket.sc` keeps its versions in `Nuclear Versions/Bracket.sc/` in the same folder:
//! `v0001.sc`, `v0002.sc`, ... and `index.json`. Each version is a whole copy of the file as it
//! was saved, so no version depends on another and pruning one never breaks the rest.
//!
//! Rules (Nuclear 3D spec, section 8):
//! - A number is never reused: `next` only grows, even after pruning.
//! - A version's file is never rewritten: it is created once, with a name no file has yet.
//! - A new version is read back and checked (hash and a full parse) before anything is pruned.
//! - Retention keeps the newest N ordinary versions; protected versions are kept on top of them
//!   and never pruned. "Keep all" prunes nothing. The newest version is never pruned.
//! - Saving bytes identical to the newest version makes no new version.
//! - Restoring an old version makes a new version with its content; the old one is untouched.
//! - Autosave never makes a version: it keeps only a local recovery checkpoint (`recovery`).
//!
//! In a folder a sync app keeps in step with the cloud, deleting a pruned version may delete it
//! there too; Nuclear cannot know whether a version reached the cloud before it was pruned.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{EngineError, Result};

/// The folder, beside a design, that holds the versions of the designs in that folder.
pub const FOLDER: &str = "Nuclear Versions";
const INDEX: &str = "index.json";
const FORMAT: &str = "nuclear-versions/1";
/// An index is small; a damaged or hostile one must not exhaust memory.
const MAX_INDEX_BYTES: u64 = 4 << 20;
/// Versions one index may list.
const MAX_VERSIONS: usize = 100_000;

/// Why a version was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Ctrl+S (or File > Save) with changes.
    #[default]
    Manual,
    /// Restoring an older version.
    Restore,
}

/// One version, as the index lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub number: u32,
    /// Seconds since 1970.
    pub saved_at: u64,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub protected: bool,
    /// For a restore: the version whose content it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored_from: Option<u32>,
    pub bytes: u64,
    /// SHA-256 of the file, hex.
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Index {
    format: String,
    /// The design file's name when the history was started.
    design: String,
    /// The number the next version gets.
    next: u32,
    versions: Vec<Version>,
}

/// How many versions to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retention {
    /// Never prune.
    All,
    /// Keep the newest N ordinary (unprotected) versions; protected ones are kept besides.
    Last(u32),
}

impl Default for Retention {
    fn default() -> Self {
        Retention::Last(20)
    }
}

/// A design's version history.
pub struct History {
    dir: PathBuf,
    design: String,
}

fn io_err(path: &Path, e: std::io::Error) -> EngineError {
    EngineError::Other(format!("versions: {}: {e}", path.display()))
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn version_file(number: u32) -> String {
    format!("v{number:04}.{}", solidcraft_io::DESIGN_EXT)
}

impl History {
    /// The history of the design at `design` (which need not exist yet).
    pub fn of(design: &Path) -> Result<History> {
        let name = design.file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.is_empty());
        let name = name.ok_or_else(|| EngineError::Other(format!("versions: {} names no file", design.display())))?;
        let parent = design.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        Ok(History { dir: parent.join(FOLDER).join(&name), design: name })
    }

    /// The folder holding this design's versions.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn read_index(&self) -> Result<Index> {
        let path = self.dir.join(INDEX);
        let file = match fs::File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Index { format: FORMAT.into(), design: self.design.clone(), next: 1, versions: Vec::new() });
            }
            Err(e) => return Err(io_err(&path, e)),
        };
        let mut bytes = Vec::new();
        file.take(MAX_INDEX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| io_err(&path, e))?;
        if bytes.len() as u64 > MAX_INDEX_BYTES {
            return Err(EngineError::Other(format!("versions: {} is too large", path.display())));
        }
        let mut index: Index =
            serde_json::from_slice(&bytes).map_err(|e| EngineError::Other(format!("versions: {} is damaged: {e}", path.display())))?;
        if index.format != FORMAT || index.versions.len() > MAX_VERSIONS {
            return Err(EngineError::Other(format!("versions: {} is not a version index this release reads", path.display())));
        }
        // `next` is past every number listed, whatever the file says: numbers are never reused.
        let past = index.versions.iter().map(|v| v.number).max().unwrap_or(0).saturating_add(1);
        index.next = index.next.max(past).max(1);
        index.versions.sort_by_key(|v| v.number);
        Ok(index)
    }

    fn write_index(&self, index: &Index) -> Result<()> {
        let path = self.dir.join(INDEX);
        let bytes = serde_json::to_vec_pretty(index).map_err(|e| EngineError::Other(e.to_string()))?;
        solidcraft_io::write_atomic(&path, &bytes, false).map_err(|e| io_err(&path, e))
    }

    /// Every version, oldest first.
    pub fn list(&self) -> Result<Vec<Version>> {
        Ok(self.read_index()?.versions)
    }

    /// Record `bytes` (the design file as just saved) as a new version and prune by `retention`.
    /// `None` when they equal the newest version (nothing new to keep).
    pub fn publish(&self, bytes: &[u8], kind: Kind, restored_from: Option<u32>, retention: Retention) -> Result<Option<Version>> {
        let mut index = self.read_index()?;
        let sha256 = sha256_hex(bytes);
        if kind != Kind::Restore && index.versions.last().is_some_and(|v| v.sha256 == sha256) {
            return Ok(None);
        }
        fs::create_dir_all(&self.dir).map_err(|e| io_err(&self.dir, e))?;
        // A number whose file exists (left by a publish that crashed before its index was
        // written) is skipped, never overwritten.
        let mut number = index.next;
        while self.dir.join(version_file(number)).exists() {
            number = number.checked_add(1).ok_or_else(|| EngineError::Other("versions: out of version numbers".into()))?;
        }
        let path = self.dir.join(version_file(number));
        write_new(&path, bytes)?;
        // Validate before the index points at it, and before anything is pruned.
        let back = fs::read(&path).map_err(|e| io_err(&path, e))?;
        if sha256_hex(&back) != sha256 {
            let _ = fs::remove_file(&path);
            return Err(EngineError::Other(format!("versions: {} did not read back as written", path.display())));
        }
        if let Err(e) = solidcraft_io::read_design(&back) {
            let _ = fs::remove_file(&path);
            return Err(EngineError::Other(format!("versions: the new version does not open: {e}")));
        }
        let version = Version { number, saved_at: unix_now(), kind, protected: false, restored_from, bytes: bytes.len() as u64, sha256 };
        index.versions.push(version.clone());
        index.next = number.saturating_add(1);
        self.write_index(&index)?;
        self.prune(retention)?;
        Ok(Some(version))
    }

    /// The versions `retention` would remove now, oldest first: what a lower setting would cost,
    /// shown before it is applied.
    pub fn plan_prune(&self, retention: Retention) -> Result<Vec<u32>> {
        Ok(plan(&self.read_index()?.versions, retention))
    }

    /// Remove the versions `retention` no longer keeps. Returns their numbers.
    pub fn prune(&self, retention: Retention) -> Result<Vec<u32>> {
        let mut index = self.read_index()?;
        let gone = plan(&index.versions, retention);
        if gone.is_empty() {
            return Ok(gone);
        }
        // The index first: a crash between the two leaves unlisted files, never a listed
        // version without its file.
        index.versions.retain(|v| !gone.contains(&v.number));
        self.write_index(&index)?;
        for n in &gone {
            let _ = fs::remove_file(self.dir.join(version_file(*n)));
        }
        Ok(gone)
    }

    /// Protect a version from pruning, or release it.
    pub fn set_protected(&self, number: u32, protected: bool) -> Result<()> {
        let mut index = self.read_index()?;
        let v = index.versions.iter_mut().find(|v| v.number == number).ok_or_else(|| EngineError::Other(format!("no version {number}")))?;
        v.protected = protected;
        self.write_index(&index)
    }

    /// A version's file, checked against its hash.
    pub fn read(&self, number: u32) -> Result<Vec<u8>> {
        let index = self.read_index()?;
        let v = index.versions.iter().find(|v| v.number == number).ok_or_else(|| EngineError::Other(format!("no version {number}")))?;
        let path = self.dir.join(version_file(number));
        let bytes = solidcraft_io::limits::read(&path.to_string_lossy(), solidcraft_io::limits::Kind::Design)
            .map_err(|e| EngineError::Other(format!("version {number}: {e}")))?;
        if sha256_hex(&bytes) != v.sha256 {
            return Err(EngineError::Other(format!("version {number} is damaged (its contents changed)")));
        }
        Ok(bytes)
    }
}

/// Create `path` with `bytes`, refusing to replace a file already there.
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| io_err(path, e))?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|e| {
        let _ = fs::remove_file(path);
        io_err(path, e)
    })
}

/// The versions `retention` removes from `versions` (sorted oldest first).
fn plan(versions: &[Version], retention: Retention) -> Vec<u32> {
    let Retention::Last(keep) = retention else { return Vec::new() };
    let newest = versions.iter().map(|v| v.number).max();
    let ordinary: Vec<u32> = versions.iter().filter(|v| !v.protected).map(|v| v.number).collect();
    let excess = ordinary.len().saturating_sub(keep.max(1) as usize);
    ordinary.into_iter().take(excess).filter(|n| Some(*n) != newest).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = solidcraft_io::vfs::temp_dir().join(format!("nuclear-versions-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A real design file whose name makes its bytes differ.
    fn design(name: &str) -> Vec<u8> {
        let doc = solidcraft_doc::Document { name: name.into(), ..Default::default() };
        solidcraft_io::write_design_container(&doc).unwrap()
    }

    fn numbers(h: &History) -> Vec<u32> {
        h.list().unwrap().iter().map(|v| v.number).collect()
    }

    #[test]
    fn keeping_five_after_six_saves_keeps_two_to_six() {
        let dir = scratch("five");
        let h = History::of(&dir.join("Bracket.sc")).unwrap();
        for i in 1..=6 {
            h.publish(&design(&format!("v{i}")), Kind::Manual, None, Retention::Last(5)).unwrap().unwrap();
        }
        assert_eq!(numbers(&h), [2, 3, 4, 5, 6]);
        assert!(!h.dir().join("v0001.sc").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_protected_version_survives_and_does_not_count() {
        let dir = scratch("protected");
        let h = History::of(&dir.join("Bracket.sc")).unwrap();
        h.publish(&design("v1"), Kind::Manual, None, Retention::Last(5)).unwrap();
        h.set_protected(1, true).unwrap();
        for i in 2..=7 {
            h.publish(&design(&format!("v{i}")), Kind::Manual, None, Retention::Last(5)).unwrap();
        }
        assert_eq!(numbers(&h), [1, 3, 4, 5, 6, 7], "V1 plus the five newest ordinary versions");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn numbers_are_never_reused_and_saving_the_same_bytes_adds_nothing() {
        let dir = scratch("numbers");
        let h = History::of(&dir.join("Bracket.sc")).unwrap();
        let a = design("a");
        assert_eq!(h.publish(&a, Kind::Manual, None, Retention::Last(1)).unwrap().unwrap().number, 1);
        assert_eq!(h.publish(&a, Kind::Manual, None, Retention::Last(1)).unwrap(), None, "no duplicate");
        assert_eq!(h.publish(&design("b"), Kind::Manual, None, Retention::Last(1)).unwrap().unwrap().number, 2);
        assert_eq!(numbers(&h), [2]);
        assert_eq!(h.publish(&design("c"), Kind::Manual, None, Retention::Last(1)).unwrap().unwrap().number, 3, "1 is gone but not reused");
        // A file a crashed publish left behind is skipped, not replaced.
        fs::write(h.dir().join("v0004.sc"), b"left over").unwrap();
        assert_eq!(h.publish(&design("d"), Kind::Manual, None, Retention::All).unwrap().unwrap().number, 5);
        assert_eq!(fs::read(h.dir().join("v0004.sc")).unwrap(), b"left over");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restoring_makes_a_new_version_and_leaves_the_old_ones_as_they_were() {
        let dir = scratch("restore");
        let h = History::of(&dir.join("Bracket.sc")).unwrap();
        for i in 1..=8 {
            h.publish(&design(&format!("v{i}")), Kind::Manual, None, Retention::All).unwrap();
        }
        let v3 = h.read(3).unwrap();
        let v8 = h.read(8).unwrap();
        let v9 = h.publish(&v3, Kind::Restore, Some(3), Retention::All).unwrap().unwrap();
        assert_eq!((v9.number, v9.kind, v9.restored_from), (9, Kind::Restore, Some(3)));
        assert_eq!(h.read(9).unwrap(), v3);
        assert_eq!(h.read(3).unwrap(), v3);
        assert_eq!(h.read(8).unwrap(), v8);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn lowering_retention_shows_its_cost_first_and_keep_all_prunes_nothing() {
        let dir = scratch("plan");
        let h = History::of(&dir.join("Bracket.sc")).unwrap();
        for i in 1..=4 {
            h.publish(&design(&format!("v{i}")), Kind::Manual, None, Retention::All).unwrap();
        }
        assert_eq!(h.plan_prune(Retention::All).unwrap(), Vec::<u32>::new());
        assert_eq!(h.plan_prune(Retention::Last(2)).unwrap(), [1, 2]);
        assert_eq!(numbers(&h), [1, 2, 3, 4], "planning removes nothing");
        assert_eq!(h.plan_prune(Retention::Last(0)).unwrap(), [1, 2, 3], "the newest is always kept");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_damaged_version_or_index_is_reported_not_trusted() {
        let dir = scratch("damaged");
        let h = History::of(&dir.join("Bracket.sc")).unwrap();
        h.publish(&design("a"), Kind::Manual, None, Retention::All).unwrap();
        fs::write(h.dir().join("v0001.sc"), design("tampered")).unwrap();
        assert!(h.read(1).unwrap_err().to_string().contains("damaged"));
        assert!(h.publish(b"not a design", Kind::Manual, None, Retention::All).is_err(), "a version that does not open is refused");
        assert_eq!(numbers(&h), [1]);
        fs::write(h.dir().join(INDEX), b"{ broken").unwrap();
        assert!(h.list().is_err());
        assert!(h.publish(&design("b"), Kind::Manual, None, Retention::Last(1)).is_err(), "nothing is pruned from a history it cannot read");
        fs::remove_dir_all(&dir).unwrap();
    }
}
