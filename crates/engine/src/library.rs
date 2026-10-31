//! Organising the project library: folders, search, rename, move, duplicate, and a portable
//! copy of a project with the designs it links to.
//!
//! Everything here works on real files and folders in the library ([`crate::storage`]), so what
//! the Start page shows is what the system's file manager shows. Nothing is ever overwritten: a
//! name that is taken is refused (rename, move) or numbered (duplicate, export). A design's
//! version history ([`crate::versions::FOLDER`]) goes with it when it is renamed or moved, and
//! the links between designs ([`solidcraft_doc::DesignLink`]) are kept pointing at the same
//! files, so renaming or moving a project never breaks its references.
//!
//! Each design carries a stable identity in its metadata ([`DOCUMENT_ID`]): given when the
//! design is created, kept by rename and move, and replaced by a fresh one in a duplicate, so
//! two files are never the same document.

use std::fs;
use std::path::{Component, Path, PathBuf};

use solidcraft_doc::Document;

/// The metadata key holding a design's stable identity (a random 128-bit hex string).
pub const DOCUMENT_ID: &str = "io.nuclear.document_id";

/// The longest name a folder or design may be given.
pub const MAX_NAME_CHARS: usize = 120;

/// How many entries a search returns at most, and how deep it looks.
pub const SEARCH_LIMIT: usize = 500;
const SEARCH_DEPTH: usize = 8;

/// How many designs a rename or move reads to keep the links to it working.
const RELINK_LIMIT: usize = 2000;

/// One thing in a library folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    /// The name shown: a folder's name, a design's file name without its extension.
    pub name: String,
    pub folder: bool,
    /// When it last changed, in seconds since 1970 (0: unknown).
    pub modified: u64,
}

/// A new random identity for a design.
pub fn new_document_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut words = [0u64; 2];
    for (i, word) in words.iter_mut().enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(seq);
        h.write_usize(i);
        #[cfg(not(target_arch = "wasm32"))]
        {
            h.write_u32(std::process::id());
            if let Ok(t) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                h.write_u128(t.as_nanos());
            }
        }
        *word = h.finish();
    }
    format!("{:016x}{:016x}", words[0], words[1])
}

/// The design's identity, when it has one.
pub fn document_id(doc: &Document) -> Option<&str> {
    doc.metadata.0.get(DOCUMENT_ID).and_then(|v| v.as_str())
}

/// Give `doc` an identity when it has none. True when one was given.
pub fn ensure_document_id(doc: &mut Document) -> bool {
    if document_id(doc).is_some() {
        return false;
    }
    doc.metadata.0.insert(DOCUMENT_ID.into(), new_document_id().into());
    true
}

/// Whether `path` names a design file (by its extension).
pub fn is_design(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(solidcraft_io::is_design_ext)
}

/// A name a folder or design can be given, trimmed; the error says why not. Names that would
/// reach another folder (`/`, `\`, `..`), that Windows refuses (`CON`, `a:b`, a trailing dot),
/// or that hold control characters are refused, so a name means the same on every system.
pub fn check_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("the name is empty".into());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("the name is longer than {MAX_NAME_CHARS} characters"));
    }
    if let Some(c) = name.chars().find(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err(if c.is_control() { "the name holds a control character".into() } else { format!("the name cannot hold {c}") });
    }
    if name == "." || name == ".." || name.ends_with('.') {
        return Err("the name cannot end with a dot".into());
    }
    if name.starts_with('.') {
        return Err("the name cannot start with a dot".into());
    }
    let stem = name.split('.').next().unwrap_or(name).trim().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem[3..].bytes().all(|b| b.is_ascii_digit()));
    if reserved {
        return Err(format!("{name} is a name Windows keeps for itself"));
    }
    Ok(name.to_string())
}

fn modified(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

fn entry(path: PathBuf, meta: &fs::Metadata) -> Option<Entry> {
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    // Hidden files, the version histories and the write probes are not projects.
    if file_name.starts_with('.') || file_name == crate::versions::FOLDER {
        return None;
    }
    if meta.is_dir() {
        return Some(Entry { name: file_name, path, folder: true, modified: modified(meta) });
    }
    if !meta.is_file() || !is_design(&path) {
        return None;
    }
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(file_name);
    Some(Entry { name, path, folder: false, modified: modified(meta) })
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| b.folder.cmp(&a.folder).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())).then_with(|| a.path.cmp(&b.path)));
}

/// The folders and designs directly in `dir`: folders first, then designs, each by name.
pub fn list(dir: &Path) -> Result<Vec<Entry>, String> {
    let read = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out: Vec<Entry> = read.flatten().filter_map(|e| e.metadata().ok().and_then(|m| entry(e.path(), &m))).collect();
    sort(&mut out);
    Ok(out)
}

/// The folders and designs anywhere under `root` whose name holds `query` (ignoring case), at
/// most [`SEARCH_LIMIT`] of them. Symbolic links are not followed, so a loop cannot trap it.
pub fn search(root: &Path, query: &str) -> Vec<Entry> {
    let query = query.trim().to_lowercase();
    let mut out = Vec::new();
    if query.is_empty() {
        return out;
    }
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else { continue };
        for e in read.flatten() {
            let Ok(meta) = fs::symlink_metadata(e.path()) else { continue };
            let Some(found) = entry(e.path(), &meta) else { continue };
            if found.folder && depth + 1 < SEARCH_DEPTH {
                stack.push((found.path.clone(), depth + 1));
            }
            if found.name.to_lowercase().contains(&query) {
                out.push(found);
                if out.len() >= SEARCH_LIMIT {
                    sort(&mut out);
                    return out;
                }
            }
        }
    }
    sort(&mut out);
    out
}

/// Make a folder named `name` in `parent`.
pub fn new_folder(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let name = check_name(name)?;
    let path = parent.join(&name);
    if fs::symlink_metadata(&path).is_ok() {
        return Err(format!("{name} is already there"));
    }
    fs::create_dir(&path).map_err(|e| format!("could not make {name}: {e}"))?;
    Ok(path)
}

/// The file or folder name `entry` would have when called `name` (a design keeps its extension).
fn target_name(path: &Path, name: &str) -> String {
    match path.extension().and_then(|e| e.to_str()).filter(|_| is_design(path)) {
        Some(ext) => format!("{name}.{ext}"),
        None => name.to_string(),
    }
}

/// Rename the folder or design at `path` to `name`. Its versions go with it and links to and
/// from it keep working. Refused when the name is taken.
pub fn rename(path: &Path, name: &str, library: &Path) -> Result<PathBuf, String> {
    let name = check_name(name)?;
    let parent = path.parent().ok_or_else(|| format!("{} has no folder", path.display()))?;
    relocate(path, &parent.join(target_name(path, &name)), library)
}

/// Move the folder or design at `path` into the folder `dest`. Refused when `dest` is inside
/// it, or holds something of the same name.
pub fn move_to(path: &Path, dest: &Path, library: &Path) -> Result<PathBuf, String> {
    let file_name = path.file_name().ok_or_else(|| format!("{} has no name", path.display()))?;
    if dest.starts_with(path) {
        return Err("a folder cannot be moved into itself".into());
    }
    if !dest.is_dir() {
        return Err(format!("{} is not a folder", dest.display()));
    }
    relocate(path, &dest.join(file_name), library)
}

fn relocate(from: &Path, to: &Path, library: &Path) -> Result<PathBuf, String> {
    if from == to {
        return Ok(to.to_path_buf());
    }
    let meta = fs::symlink_metadata(from).map_err(|e| format!("{}: {e}", from.display()))?;
    // A name differing only in case is the same file on Windows and macOS: allowed as a rename.
    let same_file = from.parent() == to.parent()
        && from.file_name().map(|n| n.to_string_lossy().to_lowercase()) == to.file_name().map(|n| n.to_string_lossy().to_lowercase());
    if !same_file && fs::symlink_metadata(to).is_ok() {
        let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return Err(format!("{name} is already there"));
    }
    // Which designs are moving, and where each one goes, so links can follow them.
    let moving: Vec<(PathBuf, PathBuf)> = if meta.is_dir() {
        designs_under(from).into_iter().filter_map(|p| p.strip_prefix(from).ok().map(|rel| (p.clone(), to.join(rel)))).collect()
    } else {
        vec![(from.to_path_buf(), to.to_path_buf())]
    };
    fs::rename(from, to).map_err(|e| format!("could not move {}: {e}", from.display()))?;
    if !meta.is_dir() {
        move_versions(from, to);
    }
    // Links from the moved designs to files that stayed, and from other designs in the library
    // to the moved ones, are written again to point at the same files.
    for (old, new) in &moving {
        let _ = rewrite_links(new, |link| {
            resolve(old, link).map(|target| moved(&target, &moving).unwrap_or(target)).map(|target| relative(new, &target))
        });
    }
    let moved_targets = |target: &Path| moved(target, &moving);
    for other in designs_under(library).into_iter().take(RELINK_LIMIT) {
        if moving.iter().any(|(_, new)| *new == other) {
            continue;
        }
        let _ = rewrite_links(&other, |link| resolve(&other, link).and_then(|t| moved_targets(&t)).map(|target| relative(&other, &target)));
    }
    Ok(to.to_path_buf())
}

/// Where `target` went, when it is one of the moved designs.
fn moved(target: &Path, moving: &[(PathBuf, PathBuf)]) -> Option<PathBuf> {
    moving.iter().find(|(old, _)| same_path(old, target)).map(|(_, new)| new.clone())
}

fn same_path(a: &Path, b: &Path) -> bool {
    normalize(a) == normalize(b)
}

/// `path` with `.` and `..` resolved by name (the files need not exist).
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The design files anywhere under `dir` (not following symbolic links, not in version
/// histories).
fn designs_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else { continue };
        for e in read.flatten() {
            let Ok(meta) = fs::symlink_metadata(e.path()) else { continue };
            let Some(found) = entry(e.path(), &meta) else { continue };
            if found.folder {
                if depth + 1 < SEARCH_DEPTH {
                    stack.push((found.path, depth + 1));
                }
            } else {
                out.push(found.path);
            }
        }
    }
    out.sort();
    out
}

/// The version history of the design at `from` follows it to `to`. A history already at `to`
/// is left alone (nothing is merged or replaced).
fn move_versions(from: &Path, to: &Path) {
    let (Ok(old), Ok(new)) = (crate::versions::History::of(from), crate::versions::History::of(to)) else { return };
    if !old.dir().is_dir() || new.dir().exists() {
        return;
    }
    if let Some(parent) = new.dir().parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::rename(old.dir(), new.dir());
}

/// The file a link in the design at `design` points at.
fn resolve(design: &Path, link: &str) -> Option<PathBuf> {
    let link = Path::new(link);
    if link.is_absolute() {
        return Some(normalize(link));
    }
    Some(normalize(&design.parent()?.join(link)))
}

/// How the design at `design` names `target` in a link: relative to its folder with `/`
/// between folders, or the full path when they share no root (another drive).
fn relative(design: &Path, target: &Path) -> String {
    let base_n = design.parent().map(normalize).unwrap_or_default();
    let base: Vec<Component> = base_n.components().collect();
    let target_n = normalize(target);
    let to: Vec<Component> = target_n.components().collect();
    let common = base.iter().zip(&to).take_while(|(a, b)| a == b).count();
    if common == 0 && !base.is_empty() {
        return target_n.to_string_lossy().replace('\\', "/");
    }
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), base.len() - common).collect();
    parts.extend(to[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/")
}

/// Write the design at `path` again with each link `fix` returns a new path for. Untouched
/// when no link changes. The picture shown in the library is kept.
fn rewrite_links(path: &Path, mut fix: impl FnMut(&str) -> Option<String>) -> Result<bool, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let Ok(mut doc) = solidcraft_io::read_design(&bytes) else { return Ok(false) };
    let mut changed = false;
    for link in doc.links.values_mut() {
        if let Some(new) = fix(&link.path)
            && new != link.path
        {
            link.path = new;
            changed = true;
        }
    }
    if changed {
        write_like(path, &doc, &bytes)?;
    }
    Ok(changed)
}

/// Write `doc` to `path`, keeping the picture from `original` (the file's earlier bytes).
fn write_like(path: &Path, doc: &Document, original: &[u8]) -> Result<(), String> {
    let thumbnail = solidcraft_io::read_design_thumbnail(original);
    let bytes = solidcraft_io::write_design_container_with_thumbnail(doc, solidcraft_io::DesignCompression::Fast, None, None, thumbnail.as_deref())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let tmp = path.with_extension(format!("{}.nuclear-tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("sc")));
    fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })
}

/// A name for a copy of `name` in `dir` that is not taken: `Name copy`, `Name copy 2`, ...
fn free_name(dir: &Path, name: &str, extension: Option<&str>, suffix: &str) -> PathBuf {
    for n in 1..10_000u32 {
        let stem = if n == 1 { format!("{name} {suffix}") } else { format!("{name} {suffix} {n}") };
        let file = match extension {
            Some(ext) => format!("{stem}.{ext}"),
            None => stem,
        };
        let path = dir.join(file);
        if fs::symlink_metadata(&path).is_err() {
            return path;
        }
    }
    dir.join(format!("{name} {suffix} {}", new_document_id()))
}

/// Copy the folder or design at `path` beside it as `Name copy`. Each copied design gets an
/// identity of its own; links inside a copied folder point at the copies.
pub fn duplicate(path: &Path) -> Result<PathBuf, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let parent = path.parent().ok_or_else(|| format!("{} has no folder", path.display()))?;
    if meta.is_dir() {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let to = free_name(parent, &name, None, "copy");
        copy_dir(path, &to, 0)?;
        for design in designs_under(&to) {
            let _ = fresh_identity(&design);
        }
        return Ok(to);
    }
    let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_string);
    let to = free_name(parent, &stem, ext.as_deref(), "copy");
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match solidcraft_io::read_design(&bytes) {
        Ok(mut doc) => {
            doc.metadata.0.insert(DOCUMENT_ID.into(), new_document_id().into());
            write_like(&to, &doc, &bytes)?;
        }
        Err(_) => fs::write(&to, &bytes).map_err(|e| format!("{}: {e}", to.display()))?,
    }
    Ok(to)
}

/// Give the design at `path` a new identity (it is a copy).
fn fresh_identity(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let Ok(mut doc) = solidcraft_io::read_design(&bytes) else { return Ok(()) };
    doc.metadata.0.insert(DOCUMENT_ID.into(), new_document_id().into());
    write_like(path, &doc, &bytes)
}

/// Copy the folder `from` to `to` (which must not exist), without following symbolic links.
fn copy_dir(from: &Path, to: &Path, depth: usize) -> Result<(), String> {
    if depth > SEARCH_DEPTH * 2 {
        return Err(format!("{} is nested too deeply", from.display()));
    }
    fs::create_dir(to).map_err(|e| format!("could not make {}: {e}", to.display()))?;
    let read = fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?;
    for e in read.flatten() {
        let Ok(meta) = fs::symlink_metadata(e.path()) else { continue };
        let dest = to.join(e.file_name());
        if meta.is_dir() {
            copy_dir(&e.path(), &dest, depth + 1)?;
        } else if meta.is_file() {
            fs::copy(e.path(), &dest).map_err(|err| format!("could not copy {}: {err}", e.path().display()))?;
        }
    }
    Ok(())
}

/// What [`export_portable`] made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Portable {
    /// The new folder holding the project.
    pub folder: PathBuf,
    /// The design itself, inside it.
    pub design: PathBuf,
    /// How many linked designs were copied with it.
    pub linked: usize,
    /// Links whose file could not be found: named, not copied.
    pub missing: Vec<String>,
}

/// Copy the design at `design` into a new folder in `dest` (`Name (portable)`), with every
/// design it links to, directly or through another linked design, in a `Linked designs` folder
/// beside it. The copies' links point at the copies, so the folder can be moved or sent to
/// another computer as it is. The original files are only read.
pub fn export_portable(design: &Path, dest: &Path) -> Result<Portable, String> {
    let stem = design
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{} names no design", design.display()))?;
    let file_name = design.file_name().ok_or_else(|| format!("{} names no design", design.display()))?;
    let folder = free_name(dest, &stem, None, "(portable)");
    fs::create_dir_all(&folder).map_err(|e| format!("could not make {}: {e}", folder.display()))?;
    let linked_dir = folder.join("Linked designs");
    // Each source design (by its normalized path) and the copy it becomes.
    let mut copies: Vec<(PathBuf, PathBuf)> = vec![(normalize(design), folder.join(file_name))];
    let mut missing = Vec::new();
    let mut i = 0;
    while i < copies.len() && copies.len() <= RELINK_LIMIT {
        let (source, copy) = copies[i].clone();
        i += 1;
        let bytes = fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;
        let Ok(mut doc) = solidcraft_io::read_design(&bytes) else {
            fs::write(&copy, &bytes).map_err(|e| format!("{}: {e}", copy.display()))?;
            continue;
        };
        if doc.links.is_empty() {
            fs::write(&copy, &bytes).map_err(|e| format!("{}: {e}", copy.display()))?;
            continue;
        }
        for link in doc.links.values_mut() {
            let Some(target) = resolve(&source, &link.path) else { continue };
            if !target.is_file() {
                if !missing.contains(&link.path) {
                    missing.push(link.path.clone());
                }
                continue;
            }
            let target_copy = match copies.iter().find(|(s, _)| *s == target) {
                Some((_, c)) => c.clone(),
                None => {
                    fs::create_dir_all(&linked_dir).map_err(|e| format!("could not make {}: {e}", linked_dir.display()))?;
                    let name = target.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let ext = target.extension().and_then(|e| e.to_str()).unwrap_or(solidcraft_io::DESIGN_EXT);
                    let mut c = linked_dir.join(format!("{name}.{ext}"));
                    let mut n = 2;
                    while copies.iter().any(|(_, o)| *o == c) {
                        c = linked_dir.join(format!("{name} {n}.{ext}"));
                        n += 1;
                    }
                    copies.push((target.clone(), c.clone()));
                    c
                }
            };
            link.path = relative(&copy, &target_copy);
        }
        write_like(&copy, &doc, &bytes)?;
    }
    let design_copy = folder.join(file_name);
    Ok(Portable { folder, design: design_copy, linked: copies.len().saturating_sub(1), missing })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = solidcraft_io::vfs::temp_dir().join(format!("nuclear-library-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn save(path: &Path, doc: &Document) {
        fs::write(path, solidcraft_io::write_design_container(doc).unwrap()).unwrap();
    }

    fn read(path: &Path) -> Document {
        solidcraft_io::read_design(&fs::read(path).unwrap()).unwrap()
    }

    fn with_link(name: &str, link: &str) -> Document {
        let mut doc = Document::new(name);
        ensure_document_id(&mut doc);
        doc.links.insert(7, solidcraft_doc::DesignLink { path: link.into(), feature: 1, inherited: Vec::new() });
        doc
    }

    #[test]
    fn names_that_reach_elsewhere_or_that_windows_refuses_are_refused() {
        assert_eq!(check_name("  Bracket v2 ").unwrap(), "Bracket v2");
        for bad in ["", "  ", "a/b", "a\\b", "..", ".hidden", "end.", "CON", "com1", "lpt9.sc", "a:b", "tab\there", "q?"] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        assert!(check_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
        assert!(check_name("Console").is_ok() && check_name("COM10").is_ok());
    }

    #[test]
    fn folders_list_first_and_search_finds_designs_in_subfolders() {
        let lib = scratch("list");
        save(&lib.join("Zeta.sc"), &Document::new("Zeta"));
        save(&lib.join("alpha.sc"), &Document::new("alpha"));
        fs::write(lib.join("notes.txt"), "not a design").unwrap();
        let sub = new_folder(&lib, "Clients").unwrap();
        save(&sub.join("Bracket for Alpha.sc"), &Document::new("Bracket"));
        fs::create_dir_all(lib.join(crate::versions::FOLDER)).unwrap();
        let names: Vec<_> = list(&lib).unwrap().into_iter().map(|e| (e.name, e.folder)).collect();
        assert_eq!(names, [("Clients".to_string(), true), ("alpha".into(), false), ("Zeta".into(), false)]);
        assert!(new_folder(&lib, "Clients").is_err(), "a taken name is refused");
        let found: Vec<_> = search(&lib, "ALPHA").into_iter().map(|e| e.name).collect();
        assert_eq!(found, ["alpha", "Bracket for Alpha"]);
        assert!(search(&lib, "  ").is_empty());
    }

    #[test]
    fn rename_and_move_keep_versions_identity_and_links() {
        let lib = scratch("rename");
        let part = lib.join("Part.sc");
        let mut part_doc = Document::new("Part");
        ensure_document_id(&mut part_doc);
        let id = document_id(&part_doc).unwrap().to_string();
        save(&part, &part_doc);
        save(&lib.join("Assembly.sc"), &with_link("Assembly", "Part.sc"));
        let history = crate::versions::History::of(&part).unwrap();
        fs::create_dir_all(history.dir()).unwrap();
        fs::write(history.dir().join("V1.sc"), b"v1").unwrap();

        let renamed = rename(&part, "Housing", &lib).unwrap();
        assert_eq!(renamed, lib.join("Housing.sc"));
        assert!(!part.exists());
        assert_eq!(document_id(&read(&renamed)), Some(id.as_str()), "the same document");
        assert!(crate::versions::History::of(&renamed).unwrap().dir().join("V1.sc").is_file(), "its versions went with it");
        assert_eq!(read(&lib.join("Assembly.sc")).links[&7].path, "Housing.sc", "the assembly still finds it");
        assert!(rename(&renamed, "Assembly", &lib).is_err(), "a taken name is refused");

        let sub = new_folder(&lib, "Parts").unwrap();
        let moved = move_to(&renamed, &sub, &lib).unwrap();
        assert_eq!(read(&lib.join("Assembly.sc")).links[&7].path, "Parts/Housing.sc");
        let assembly = move_to(&lib.join("Assembly.sc"), &sub, &lib).unwrap();
        assert_eq!(read(&assembly).links[&7].path, "Housing.sc", "a moved design's own links follow");
        assert!(moved.is_file());
        assert!(move_to(&sub, &sub, &lib).is_err(), "not into itself");
        // A whole project folder renamed keeps the links inside it.
        let project = rename(&sub, "Gearbox", &lib).unwrap();
        assert_eq!(read(&project.join("Assembly.sc")).links[&7].path, "Housing.sc");
    }

    #[test]
    fn a_duplicate_is_a_new_document_and_never_overwrites() {
        let lib = scratch("duplicate");
        let mut doc = Document::new("Plate");
        ensure_document_id(&mut doc);
        let path = lib.join("Plate.sc");
        save(&path, &doc);
        let a = duplicate(&path).unwrap();
        let b = duplicate(&path).unwrap();
        assert_eq!((a.file_name().unwrap(), b.file_name().unwrap()), ("Plate copy.sc".as_ref(), "Plate copy 2.sc".as_ref()));
        let ids = [read(&path), read(&a), read(&b)].map(|d| document_id(&d).unwrap().to_string());
        assert!(ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2], "{ids:?}");
        let folder = new_folder(&lib, "Job").unwrap();
        save(&folder.join("Plate.sc"), &doc);
        let copy = duplicate(&folder).unwrap();
        assert_eq!(copy, lib.join("Job copy"));
        assert_ne!(document_id(&read(&copy.join("Plate.sc"))), document_id(&doc));
    }

    #[test]
    fn a_portable_project_carries_its_linked_designs_and_points_at_the_copies() {
        let lib = scratch("portable");
        let parts = new_folder(&lib, "Parts").unwrap();
        save(&parts.join("Bolt.sc"), &Document::new("Bolt"));
        save(&parts.join("Housing.sc"), &with_link("Housing", "Bolt.sc"));
        save(&lib.join("Gearbox.sc"), &with_link("Gearbox", "Parts/Housing.sc"));
        let mut broken = with_link("Broken", "gone.sc");
        broken.links.insert(8, solidcraft_doc::DesignLink { path: "Parts/Bolt.sc".into(), feature: 2, inherited: Vec::new() });
        save(&lib.join("Broken.sc"), &broken);
        let out = scratch("portable-out");

        let p = export_portable(&lib.join("Gearbox.sc"), &out).unwrap();
        assert_eq!(p.folder, out.join("Gearbox (portable)"));
        assert_eq!(p.linked, 2);
        assert!(p.missing.is_empty());
        assert_eq!(read(&p.design).links[&7].path, "Linked designs/Housing.sc");
        assert_eq!(read(&p.folder.join("Linked designs/Housing.sc")).links[&7].path, "Bolt.sc");
        assert!(p.folder.join("Linked designs/Bolt.sc").is_file());
        assert_eq!(read(&lib.join("Gearbox.sc")).links[&7].path, "Parts/Housing.sc", "the original is only read");
        assert_eq!(export_portable(&lib.join("Gearbox.sc"), &out).unwrap().folder, out.join("Gearbox (portable) 2"));

        let p = export_portable(&lib.join("Broken.sc"), &out).unwrap();
        assert_eq!(p.missing, ["gone.sc"]);
        assert_eq!(p.linked, 1);
    }

    #[test]
    fn identities_are_given_once_and_differ() {
        let mut doc = Document::new("A");
        assert!(ensure_document_id(&mut doc));
        let id = document_id(&doc).unwrap().to_string();
        assert_eq!(id.len(), 32);
        assert!(!ensure_document_id(&mut doc));
        assert_eq!(document_id(&doc), Some(id.as_str()));
        assert_ne!(new_document_id(), new_document_id());
    }
}
