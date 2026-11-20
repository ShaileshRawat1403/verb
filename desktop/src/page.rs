//! An optional page folder that replaces the built-in page without a host restart.
//!
//! See docs/PAGE_ONLY_RELEASE.md. With `VERB_PAGE_DIR` unset (the default) nothing here runs and
//! the page compiled into the binary is served. When it is set, the host serves the release that
//! `<VERB_PAGE_DIR>/current` points at, but only after every check passes:
//!
//! - the manifest pairs the page with this exact binary (`hostRevision`), and this binary was built
//!   from a clean tree, so the host the page was tested against is the host that runs it;
//! - the manifest names exactly the files the built-in page serves, and each SHA-256 matches;
//! - every folder, the `current` link and every file are owned by root and are not group- or
//!   world-writable, so the page carries the same integrity as the binary.
//!
//! Anything else falls back to the built-in page and logs why, once per reason. The page has full
//! authority over terminals in the browser, so failing closed to the built-in page is the rule.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The files a page release must contain: exactly the paths the built-in page serves.
pub(crate) const PAGE_FILES: [&str; 9] = [
    "index.html",
    "app.js",
    "app.css",
    "favicon.svg",
    "fonts/barlow-400.woff2",
    "fonts/barlow-500.woff2",
    "fonts/barlow-600.woff2",
    "fonts/barlow-condensed-600.woff2",
    "fonts/barlow-condensed-700.woff2",
];

/// No page file is anywhere near this; the bound stops a bad release from exhausting memory.
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MANIFEST_MAX_BYTES: u64 = 64 * 1024;

/// The page this host serves: the built-in one, or a verified release from the page folder.
pub(crate) struct PageSource {
    root: Option<PathBuf>,
    host_revision: String,
    /// The owner every path must have. Root in production; tests use their own uid.
    owner: u32,
    loaded: Option<Loaded>,
    /// The last reason a release was refused, so the log names each new reason once.
    last_refusal: Option<String>,
}

struct Loaded {
    target: PathBuf,
    revision: String,
    files: HashMap<&'static str, Vec<u8>>,
}

impl PageSource {
    /// Reads `VERB_PAGE_DIR` once at start. A host built from a dirty or unknown tree never uses
    /// a page folder: the pairing would not name the code that is actually running.
    pub(crate) fn from_env() -> Self {
        let root = std::env::var_os("VERB_PAGE_DIR")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        let clean = env!("VERB_BUILD_DIRTY") == "false";
        let revision = env!("VERB_BUILD_REVISION");
        if root.is_some() && !clean {
            eprintln!(
                "verb: VERB_PAGE_DIR is set but this binary was not built from a clean tree; serving the built-in page"
            );
        }
        Self::new(root.filter(|_| clean), revision.to_owned(), 0)
    }

    fn new(root: Option<PathBuf>, host_revision: String, owner: u32) -> Self {
        Self {
            root,
            host_revision,
            owner,
            loaded: None,
            last_refusal: None,
        }
    }

    /// Follows `current` and loads its release if it changed. Called for `/` and `/healthz`, so a
    /// switch is picked up by the next page load; other assets come from the set loaded then.
    pub(crate) fn refresh(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let target = match fs::canonicalize(root.join("current")) {
            Ok(target) => target,
            Err(error) => {
                self.refuse(format!("no current page release ({error})"));
                return;
            }
        };
        if self.loaded.as_ref().is_some_and(|l| l.target == target) {
            return;
        }
        match load(&root, &target, &self.host_revision, self.owner) {
            Ok(loaded) => {
                eprintln!("verb: serving page release {}", loaded.revision);
                self.loaded = Some(loaded);
                self.last_refusal = None;
            }
            Err(reason) => self.refuse(reason),
        }
    }

    fn refuse(&mut self, reason: String) {
        if self.loaded.take().is_some() || self.last_refusal.as_deref() != Some(&reason) {
            eprintln!("verb: page folder refused, serving the built-in page: {reason}");
        }
        self.last_refusal = Some(reason);
    }

    /// The bytes for `name` from the loaded release, or `None` to serve the built-in file.
    pub(crate) fn file(&self, name: &str) -> Option<&[u8]> {
        self.loaded
            .as_ref()
            .and_then(|l| l.files.get(name))
            .map(Vec::as_slice)
    }

    /// Which page is served, for `/healthz`.
    pub(crate) fn status(&self) -> serde_json::Value {
        match &self.loaded {
            Some(loaded) => serde_json::json!({"source": "folder", "revision": loaded.revision}),
            None => serde_json::json!({"source": "built-in"}),
        }
    }
}

fn load(root: &Path, target: &Path, host_revision: &str, owner: u32) -> Result<Loaded, String> {
    let root = fs::canonicalize(root).map_err(|e| format!("page folder unreadable ({e})"))?;
    let releases = root.join("releases");
    if target.parent() != Some(releases.as_path()) {
        return Err("current must point at a folder directly inside releases/".into());
    }
    for dir in [
        &root,
        &releases,
        &target.to_path_buf(),
        &target.join("fonts"),
    ] {
        guarded(dir, owner, true)?;
    }
    guarded(&root.join("current"), owner, false)?;

    let manifest_path = target.join("manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&read_guarded(&manifest_path, owner, MANIFEST_MAX_BYTES)?)
            .map_err(|e| format!("manifest is not valid JSON ({e})"))?;
    if manifest.get("format").and_then(|v| v.as_u64()) != Some(1) {
        return Err("manifest format is not 1".into());
    }
    let paired = manifest.get("hostRevision").and_then(|v| v.as_str());
    if paired != Some(host_revision) {
        return Err(format!(
            "manifest hostRevision {} does not match this host {host_revision}",
            paired.unwrap_or("(missing)")
        ));
    }
    let revision = manifest
        .get("pageRevision")
        .and_then(|v| v.as_str())
        .filter(|r| r.len() == 40 && r.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("manifest pageRevision is not a full commit hash")?
        .to_owned();
    let listed = manifest
        .get("files")
        .and_then(|v| v.as_object())
        .ok_or("manifest has no files map")?;
    if listed.len() != PAGE_FILES.len() || PAGE_FILES.iter().any(|f| !listed.contains_key(*f)) {
        return Err("manifest must list exactly the page files".into());
    }

    let mut files = HashMap::new();
    for name in PAGE_FILES {
        let expected = listed[name].as_str().unwrap_or_default();
        let bytes = read_guarded(&target.join(name), owner, MAX_FILE_BYTES)?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(format!("{name} does not match its manifest hash"));
        }
        files.insert(name, bytes);
    }
    Ok(Loaded {
        target: target.to_path_buf(),
        revision,
        files,
    })
}

/// Owned by `owner`, not group- or world-writable, and the expected kind. A link is checked as
/// the link itself; the `current` link is the only one allowed.
fn guarded(path: &Path, owner: u32, dir: bool) -> Result<fs::Metadata, String> {
    use std::os::unix::fs::MetadataExt;
    let meta =
        fs::symlink_metadata(path).map_err(|e| format!("{} unreadable ({e})", path.display()))?;
    let kind_ok = if dir {
        meta.is_dir()
    } else {
        meta.is_file() || (meta.file_type().is_symlink() && path.ends_with("current"))
    };
    if !kind_ok {
        return Err(format!(
            "{} is not the expected kind of entry",
            path.display()
        ));
    }
    if meta.uid() != owner {
        return Err(format!("{} is not owned by uid {owner}", path.display()));
    }
    if !meta.file_type().is_symlink() && meta.mode() & 0o022 != 0 {
        return Err(format!("{} is group- or world-writable", path.display()));
    }
    Ok(meta)
}

fn read_guarded(path: &Path, owner: u32, max: u64) -> Result<Vec<u8>, String> {
    let meta = guarded(path, owner, false)?;
    if !meta.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    if meta.len() > max {
        return Err(format!("{} is too large", path.display()));
    }
    fs::read(path).map_err(|e| format!("{} unreadable ({e})", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    const HOST: &str = "1111111111111111111111111111111111111111";
    const PAGE_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const PAGE_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn uid() -> u32 {
        unsafe { libc::geteuid() }
    }

    fn scratch(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "verb-page-{label}-{}-{}",
            std::process::id(),
            crate::now_millis()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("releases")).unwrap();
        for dir in [&root, &root.join("releases")] {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).unwrap();
        }
        root
    }

    /// Writes a release whose files say which release they are, with a correct manifest.
    fn release(root: &Path, page: &str, host: &str) -> PathBuf {
        let dir = root.join("releases").join(page);
        fs::create_dir_all(dir.join("fonts")).unwrap();
        let mut hashes = serde_json::Map::new();
        for name in PAGE_FILES {
            let body = format!("{name} from {page}");
            fs::write(dir.join(name), &body).unwrap();
            fs::set_permissions(dir.join(name), fs::Permissions::from_mode(0o644)).unwrap();
            hashes.insert(
                name.into(),
                format!("{:x}", Sha256::digest(body.as_bytes())).into(),
            );
        }
        let manifest = serde_json::json!({"format": 1, "pageRevision": page, "hostRevision": host, "files": hashes});
        fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
        fs::set_permissions(dir.join("manifest.json"), fs::Permissions::from_mode(0o644)).unwrap();
        for d in [&dir, &dir.join("fonts")] {
            fs::set_permissions(d, fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    }

    fn point(root: &Path, page: &str) {
        let link = root.join("current");
        let _ = fs::remove_file(&link);
        symlink(Path::new("releases").join(page), &link).unwrap();
    }

    fn source(root: &Path) -> PageSource {
        PageSource::new(Some(root.to_path_buf()), HOST.into(), uid())
    }

    fn served(page: &PageSource) -> Option<String> {
        page.file("app.js")
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }

    #[test]
    fn without_a_page_folder_the_built_in_page_is_served() {
        let mut page = PageSource::new(None, HOST.into(), uid());
        page.refresh();
        assert!(page.file("index.html").is_none());
        assert_eq!(page.status()["source"], "built-in");
    }

    #[test]
    fn a_paired_release_is_served_and_reported() {
        let root = scratch("paired");
        release(&root, PAGE_A, HOST);
        point(&root, PAGE_A);
        let mut page = source(&root);
        page.refresh();
        assert_eq!(
            served(&page).as_deref(),
            Some(&*format!("app.js from {PAGE_A}"))
        );
        assert_eq!(page.status()["source"], "folder");
        assert_eq!(page.status()["revision"], PAGE_A);
        assert!(page.file("fonts/barlow-400.woff2").is_some());
        assert!(page.file("manifest.json").is_none());
    }

    #[test]
    fn switching_current_is_picked_up_without_a_restart_and_back_again() {
        let root = scratch("switch");
        release(&root, PAGE_A, HOST);
        release(&root, PAGE_B, HOST);
        point(&root, PAGE_A);
        let mut page = source(&root);
        page.refresh();
        assert_eq!(page.status()["revision"], PAGE_A);
        point(&root, PAGE_B);
        page.refresh();
        assert_eq!(page.status()["revision"], PAGE_B);
        assert_eq!(
            served(&page).as_deref(),
            Some(&*format!("app.js from {PAGE_B}"))
        );
        fs::remove_file(root.join("current")).unwrap();
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");
    }

    #[test]
    fn a_page_paired_with_another_host_is_refused() {
        let root = scratch("other-host");
        release(&root, PAGE_A, "2222222222222222222222222222222222222222");
        point(&root, PAGE_A);
        let mut page = source(&root);
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");
        assert!(page
            .last_refusal
            .as_deref()
            .unwrap()
            .contains("hostRevision"));
    }

    #[test]
    fn a_changed_file_is_refused() {
        let root = scratch("tampered");
        let dir = release(&root, PAGE_A, HOST);
        fs::write(dir.join("app.js"), "something else").unwrap();
        point(&root, PAGE_A);
        let mut page = source(&root);
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");
        assert!(page.last_refusal.as_deref().unwrap().contains("app.js"));
    }

    #[test]
    fn a_manifest_with_a_missing_or_extra_file_is_refused() {
        for change in ["missing", "extra"] {
            let root = scratch(change);
            let dir = release(&root, PAGE_A, HOST);
            let path = dir.join("manifest.json");
            let mut manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let files = manifest["files"].as_object_mut().unwrap();
            if change == "missing" {
                files.remove("app.css");
            } else {
                files.insert("extra.js".into(), "00".into());
            }
            fs::write(&path, manifest.to_string()).unwrap();
            point(&root, PAGE_A);
            let mut page = source(&root);
            page.refresh();
            assert_eq!(page.status()["source"], "built-in", "{change}");
        }
    }

    #[test]
    fn a_writable_file_or_folder_is_refused() {
        for target in ["file", "folder"] {
            let root = scratch(&format!("writable-{target}"));
            let dir = release(&root, PAGE_A, HOST);
            let path = if target == "file" {
                dir.join("app.js")
            } else {
                dir.clone()
            };
            let mode = if target == "file" { 0o666 } else { 0o777 };
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            point(&root, PAGE_A);
            let mut page = source(&root);
            page.refresh();
            assert_eq!(page.status()["source"], "built-in", "{target}");
            assert!(page.last_refusal.as_deref().unwrap().contains("writable"));
        }
    }

    #[test]
    fn a_folder_owned_by_someone_else_is_refused() {
        let root = scratch("owner");
        release(&root, PAGE_A, HOST);
        point(&root, PAGE_A);
        let mut page = PageSource::new(Some(root), HOST.into(), uid().wrapping_add(1));
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");
        assert!(page.last_refusal.as_deref().unwrap().contains("owned"));
    }

    #[test]
    fn a_dangling_current_or_one_outside_releases_is_refused() {
        let root = scratch("dangling");
        point(&root, PAGE_A);
        let mut page = source(&root);
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");

        let outside = scratch("outside-target");
        release(&outside, PAGE_A, HOST);
        let link = root.join("current");
        fs::remove_file(&link).unwrap();
        symlink(outside.join("releases").join(PAGE_A), &link).unwrap();
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");
        assert!(page.last_refusal.as_deref().unwrap().contains("releases/"));
    }

    #[test]
    fn a_file_replaced_by_a_link_is_refused() {
        let root = scratch("file-link");
        let dir = release(&root, PAGE_A, HOST);
        let elsewhere = root.join("elsewhere.js");
        fs::write(&elsewhere, "app.js from elsewhere").unwrap();
        fs::remove_file(dir.join("app.js")).unwrap();
        symlink(&elsewhere, dir.join("app.js")).unwrap();
        point(&root, PAGE_A);
        let mut page = source(&root);
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");
    }

    #[test]
    fn a_refused_switch_drops_the_previous_release_rather_than_keeping_it() {
        let root = scratch("refused-switch");
        release(&root, PAGE_A, HOST);
        let bad = release(&root, PAGE_B, HOST);
        fs::write(bad.join("index.html"), "tampered").unwrap();
        point(&root, PAGE_A);
        let mut page = source(&root);
        page.refresh();
        assert_eq!(page.status()["revision"], PAGE_A);
        point(&root, PAGE_B);
        page.refresh();
        assert_eq!(page.status()["source"], "built-in");
    }
}
