//! Native-local Studio bridge. No access through the external execution boundary.
//! Saves compare a content revision, refuse links, and publish atomically. Arbitrary
//! editors do not share our lock: this detects completed outside edits, not an OS-wide CAS.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(crate) const MAX_BYTES: usize = 200 * 1024;
pub(crate) const CONFLICT: &str = "studio conflict: ";
static SAVES: Mutex<()> = Mutex::new(());

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Document {
    pub path: String,
    pub text: String,
    pub revision: String,
    pub bytes: usize,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct FileRequest {
    pub workspace_id: String,
    pub path: String,
    pub expected_revision: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct SaveRequest {
    pub workspace_id: String,
    pub path: String,
    pub expected_revision: String,
    pub text: String,
}

pub(crate) const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
#[derive(Deserialize, Serialize)]
pub(crate) struct Image {
    pub path: String,
    pub mime: String,
    pub data: String,
    pub revision: String,
    pub bytes: usize,
}
fn image_mime(raw: &[u8]) -> Option<&'static str> {
    if raw.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if raw.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if raw.starts_with(b"GIF87a") || raw.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if raw.starts_with(b"RIFF") && raw.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}
pub(crate) fn validate_image(image: &Image, path: &str) -> Result<(), String> {
    use base64::Engine;
    if image.path != path
        || image.bytes > MAX_IMAGE_BYTES
        || image.data.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4
    {
        return Err("Invalid image identity or size".into());
    }
    let raw = base64::engine::general_purpose::STANDARD
        .decode(&image.data)
        .map_err(|_| "Invalid image encoding")?;
    if raw.len() != image.bytes
        || image_mime(&raw) != Some(image.mime.as_str())
        || format!("{:x}", Sha256::digest(&raw)) != image.revision
    {
        return Err("Image content or revision mismatch".into());
    }
    Ok(())
}
pub(crate) fn image(project: &Path, path: &str) -> Result<Image, String> {
    use base64::Engine;
    let (directory, leaf) = location(project, path)?;
    let mut file = open_at(&directory, &leaf, libc::O_RDONLY | libc::O_NONBLOCK)?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || before.nlink() != 1 || before.len() > MAX_IMAGE_BYTES as u64 {
        return Err("Images must be regular files without links, up to 2 MB".into());
    }
    let mut raw = Vec::new();
    (&mut file)
        .take((MAX_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    let after = file.metadata().map_err(|e| e.to_string())?;
    if raw.len() > MAX_IMAGE_BYTES
        || raw.len() as u64 != before.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
        || after.nlink() != 1
    {
        return Err(format!("{CONFLICT}Image changed while reading; reopen it"));
    }
    let mime = image_mime(&raw).ok_or(
        "Preview supports PNG, JPEG, GIF and WebP; active formats such as SVG are not executed",
    )?;
    Ok(Image {
        path: path.into(),
        mime: mime.into(),
        data: base64::engine::general_purpose::STANDARD.encode(&raw),
        revision: format!("{:x}", Sha256::digest(&raw)),
        bytes: raw.len(),
    })
}

fn error() -> String {
    std::io::Error::last_os_error().to_string()
}
fn name(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| "invalid file path".to_owned())
}
fn open_at(parent: &File, name: &CString, flags: i32) -> Result<File, String> {
    // SAFETY: parent is a live descriptor, name is NUL-terminated, and ownership of
    // the new descriptor transfers exactly once to File. No symlinks are followed.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        Err(error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
fn location(project: &Path, path: &str) -> Result<(File, CString), String> {
    let parts: Vec<_> = Path::new(path).components().collect();
    if parts.is_empty()
        || parts.iter().any(|p| !matches!(p, Component::Normal(_)))
        || path.contains('\0')
    {
        return Err("invalid project-relative path".to_owned());
    }
    if !crate::hub::files(project)?
        .files
        .iter()
        .any(|f| f.path == path)
    {
        return Err("file is ignored, absent, or outside the visible listing".to_owned());
    }
    let mut directory = File::open(project.canonicalize().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    for part in &parts[..parts.len() - 1] {
        directory = open_at(
            &directory,
            &name(&part.as_os_str().to_string_lossy())?,
            libc::O_RDONLY | libc::O_DIRECTORY,
        )?;
    }
    Ok((
        directory,
        name(&parts.last().unwrap().as_os_str().to_string_lossy())?,
    ))
}
fn read_open(file: &mut File, path: &str) -> Result<Document, String> {
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err("only regular files without hard links are supported".to_owned());
    }
    if metadata.len() > MAX_BYTES as u64 {
        return Err("file exceeds the 200 KB editor limit".to_owned());
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    (&mut *file)
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.len() > MAX_BYTES || raw.contains(&0) {
        return Err("file is too large or binary".to_owned());
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    if metadata.len() != after.len()
        || metadata.mtime() != after.mtime()
        || metadata.mtime_nsec() != after.mtime_nsec()
        || metadata.ctime() != after.ctime()
        || metadata.ctime_nsec() != after.ctime_nsec()
        || after.nlink() != 1
    {
        return Err(format!(
            "{CONFLICT}the file changed while it was being read; retry the read"
        ));
    }
    let revision = format!("{:x}", Sha256::digest(&raw));
    let text = String::from_utf8(raw).map_err(|_| "the editor requires UTF-8 text".to_owned())?;
    Ok(Document {
        path: path.to_owned(),
        bytes: text.len(),
        text,
        revision,
    })
}
pub(crate) fn read(project: &Path, path: &str) -> Result<Document, String> {
    let (directory, leaf) = location(project, path)?;
    read_open(
        &mut open_at(&directory, &leaf, libc::O_RDONLY | libc::O_NONBLOCK)?,
        path,
    )
}
pub(crate) fn expect_revision(actual: &str, expected: &str) -> Result<(), String> {
    if actual != expected {
        Err(format!("{CONFLICT}the file changed on disk. Your draft is retained; reload or copy it before resolving the conflict."))
    } else {
        Ok(())
    }
}
pub(crate) fn save(project: &Path, request: SaveRequest) -> Result<Document, String> {
    if request.text.len() > MAX_BYTES || request.text.contains('\0') {
        return Err("save requires UTF-8 text up to 200 KB, without NUL bytes".to_owned());
    }
    let _lock = SAVES
        .lock()
        .map_err(|_| "Studio save lock is unavailable".to_owned())?;
    let (directory, leaf) = location(project, &request.path)?;
    let mut original = open_at(&directory, &leaf, libc::O_RDONLY | libc::O_NONBLOCK)?;
    let before = original.metadata().map_err(|e| e.to_string())?;
    if before.mode() & 0o200 == 0 || before.uid() != unsafe { libc::geteuid() } {
        return Err("the editor only replaces writable files owned by this host user".to_owned());
    }
    expect_revision(
        &read_open(&mut original, &request.path)?.revision,
        &request.expected_revision,
    )?;
    let temp = name(&format!(".verb-studio-{}.tmp", crate::new_id()))?;
    // If exclusive creation fails, do not unlink a name we did not create.
    let mut output = open_at(
        &directory,
        &temp,
        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
    )?;
    let result = (|| {
        output
            .write_all(request.text.as_bytes())
            .and_then(|()| output.sync_all())
            .map_err(|e| e.to_string())?;
        output
            .set_permissions(std::fs::Permissions::from_mode(before.mode() & 0o777))
            .map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        // Reopen the name: another editor may have replaced the inode while we prepared the save.
        let mut current = open_at(&directory, &leaf, libc::O_RDONLY | libc::O_NONBLOCK)?;
        let now = current.metadata().map_err(|e| e.to_string())?;
        if before.dev() != now.dev() || before.ino() != now.ino() || before.mode() != now.mode() {
            return Err(format!("{CONFLICT}the file was replaced or its permissions changed; your draft is retained"));
        }
        expect_revision(
            &read_open(&mut current, &request.path)?.revision,
            &request.expected_revision,
        )?;
        // SAFETY: both names are relative to the same live directory descriptor.
        if unsafe {
            libc::renameat(
                directory.as_raw_fd(),
                temp.as_ptr(),
                directory.as_raw_fd(),
                leaf.as_ptr(),
            )
        } != 0
        {
            return Err(error());
        }
        directory.sync_all().map_err(|e| {
            format!("Save may have landed but directory sync failed: {e}. Reload before retrying.")
        })?;
        Ok(Document {
            path: request.path,
            bytes: request.text.len(),
            revision: format!("{:x}", Sha256::digest(request.text.as_bytes())),
            text: request.text,
        })
    })();
    if result.is_err() {
        // SAFETY: unlink only our random temporary name, never the requested file.
        unsafe {
            libc::unlinkat(directory.as_raw_fd(), temp.as_ptr(), 0);
        }
    }
    result
}

// A readiness message is a rendering signal, never evidence that the project's code is correct.
// Keep the original doctype first and leave the saved file/revision untouched.
fn preview_document(text: &str, ticket: &str) -> Vec<u8> {
    let token = serde_json::to_string(ticket)
        .unwrap()
        .replace('<', "\\u003c");
    let shim = format!("<script>addEventListener('DOMContentLoaded',()=>parent.postMessage({{type:'verb:preview-ready',ticket:{token}}},'*'),{{once:true}})</script>");
    let trimmed = text.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let offset = if trimmed
        .get(..9)
        .is_some_and(|s| s.eq_ignore_ascii_case("<!doctype"))
    {
        trimmed
            .find('>')
            .map(|end| text.len() - trimmed.len() + end + 1)
            .unwrap_or(0)
    } else {
        0
    };
    format!("{}{}{}", &text[..offset], shim, &text[offset..]).into_bytes()
}

struct Snapshot {
    html: Vec<u8>,
    expires: Instant,
}
#[derive(Default)]
pub(crate) struct Previews {
    entries: HashMap<String, Snapshot>,
}
impl Previews {
    pub(crate) fn insert(&mut self, ticket: String, doc: Document) -> Result<(), String> {
        self.entries.retain(|_, v| v.expires > Instant::now());
        if self.entries.len() >= 4 {
            return Err("four previews are pending; open one or wait two minutes".to_owned());
        }
        if !doc.path.to_lowercase().ends_with(".html") && !doc.path.to_lowercase().ends_with(".htm")
        {
            return Err(
                "preview supports a self-contained HTML file with inline CSS and JavaScript"
                    .to_owned(),
            );
        }
        let html = preview_document(&doc.text, &ticket);
        self.entries.insert(
            ticket,
            Snapshot {
                html,
                expires: Instant::now() + Duration::from_secs(120),
            },
        );
        Ok(())
    }
    pub(crate) fn take(&mut self, ticket: &str) -> Option<Vec<u8>> {
        self.entries.retain(|_, v| v.expires > Instant::now());
        self.entries.remove(ticket).map(|v| v.html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("verb-studio-native-{}", crate::new_id()));
        fs::create_dir(&dir).unwrap();
        for args in [
            ["init", "-q", "-b", "main"].as_slice(),
            ["config", "user.name", "Fixture"].as_slice(),
            ["config", "user.email", "fixture@example.invalid"].as_slice(),
        ] {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .status()
                .unwrap()
                .success());
        }
        fs::write(dir.join("index.html"), "<h1>Before</h1>").unwrap();
        dir
    }
    #[test]
    fn preview_readiness_preserves_doctype_and_escapes_the_message() {
        let html = " \n<!DOCTYPE html><h1>Saved content</h1>";
        let rendered =
            String::from_utf8(super::preview_document(html, "ticket</script>test")).unwrap();
        assert!(rendered.starts_with(" \n<!DOCTYPE html><script>"));
        assert!(rendered.ends_with("<h1>Saved content</h1>"));
        assert!(rendered.contains("DOMContentLoaded"));
        assert!(rendered.contains("verb:preview-ready"));
        assert!(rendered.contains("ticket\\u003c/script>test"));
        assert_eq!(html, " \n<!DOCTYPE html><h1>Saved content</h1>");
    }

    #[test]
    fn image_access_validates_signature_links_limit_and_reply_identity() {
        let dir = fixture();
        fs::write(dir.join("picture.png"), b"\x89PNG\r\n\x1a\nfixture").unwrap();
        let mut picture = image(&dir, "picture.png").unwrap();
        validate_image(&picture, "picture.png").unwrap();
        assert!(validate_image(&picture, "another.png").is_err());
        picture.revision = "wrong".into();
        assert!(validate_image(&picture, "picture.png").is_err());
        std::os::unix::fs::symlink("picture.png", dir.join("linked.png")).unwrap();
        assert!(image(&dir, "linked.png").is_err());
        fs::write(dir.join("fake.png"), b"<svg onload='alert(1)'/>").unwrap();
        assert!(image(&dir, "fake.png").is_err());
        fs::write(dir.join("large.png"), vec![0; MAX_IMAGE_BYTES + 1]).unwrap();
        assert!(image(&dir, "large.png").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn save_is_revision_checked_and_preserves_mode() {
        let dir = fixture();
        fs::set_permissions(dir.join("index.html"), fs::Permissions::from_mode(0o751)).unwrap();
        let before = read(&dir, "index.html").unwrap();
        let request = |revision| SaveRequest {
            workspace_id: "test".into(),
            path: "index.html".into(),
            expected_revision: revision,
            text: "<h1>After</h1>".into(),
        };
        let after = save(&dir, request(before.revision.clone())).unwrap();
        assert_ne!(before.revision, after.revision);
        assert_eq!(
            fs::metadata(dir.join("index.html")).unwrap().mode() & 0o777,
            0o751
        );
        assert!(save(&dir, request(before.revision))
            .unwrap_err()
            .starts_with(CONFLICT));
        assert_eq!(
            fs::read_to_string(dir.join("index.html")).unwrap(),
            after.text
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unsafe_or_nontext_files_are_refused_without_modification() {
        let dir = fixture();
        fs::write(dir.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(dir.join("ignored.txt"), "private").unwrap();
        fs::write(dir.join("binary"), [0, 1, 2]).unwrap();
        fs::write(dir.join("invalid"), [255]).unwrap();
        std::os::unix::fs::symlink("index.html", dir.join("link.html")).unwrap();
        fs::hard_link(dir.join("index.html"), dir.join("hard.html")).unwrap();
        for path in [
            "../index.html",
            "/tmp/no",
            "ignored.txt",
            "binary",
            "invalid",
            "link.html",
            "hard.html",
        ] {
            assert!(read(&dir, path).is_err(), "{path}");
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn preview_tickets_are_bounded_expiring_and_single_use() {
        let mut store = Previews::default();
        for i in 0..4 {
            store
                .insert(
                    i.to_string(),
                    Document {
                        path: "app.html".into(),
                        text: "<h1>app</h1>".into(),
                        bytes: 12,
                        revision: "r".into(),
                    },
                )
                .unwrap();
        }
        assert!(store
            .insert(
                "fifth".into(),
                Document {
                    path: "app.html".into(),
                    text: "".into(),
                    bytes: 0,
                    revision: "r".into()
                }
            )
            .is_err());
        assert!(store.take("0").is_some());
        assert!(store.take("0").is_none());
        store.entries.get_mut("1").unwrap().expires = Instant::now();
        assert!(store.take("1").is_none());
    }
}
