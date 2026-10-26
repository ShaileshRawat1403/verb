//! Studio operations run as the registered project user through the existing fixed client.
//! No host-side filesystem fallback or guest installation. Fixed Python source is embedded in Verb.
use crate::studio::{Document, SaveRequest, MAX_BYTES};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Mutex;

const MAX_INPUT: usize = MAX_BYTES * 6 + 2048;
static SAVES: Mutex<()> = Mutex::new(());

pub(crate) fn configured() -> bool {
    std::env::var("VERB_STUDIO_BOUNDARY").as_deref() == Ok("monitored-v1")
        && matches!(
            crate::isolation::profile(),
            Ok(Some(crate::isolation::Profile::External))
        )
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
enum Operation {
    Capabilities,
    Read { path: String },
    Save { request: SaveRequest },
}

fn exchange(project: &Path, operation: &Operation) -> Result<Value, String> {
    if !configured() {
        return Err("Studio boundary bridge is disabled".into());
    }
    let input = serde_json::to_vec(operation).map_err(|e| e.to_string())?;
    if input.len() > MAX_INPUT {
        return Err("Studio helper request exceeds limit".into());
    }
    let mut command = crate::isolation::command(Path::new("/usr/bin/timeout"), project, "")?;
    command.args([
        "--kill-after=1s",
        "15s",
        "/usr/bin/python3",
        "-I",
        "-S",
        "-c",
        include_str!("../scripts/studio-workspace.wire"),
        env!("VERB_BUILD_REVISION"),
    ]);
    let output = crate::exec::studio_exchange(&mut command, &input).map_err(|_| {
        "Studio boundary exchange failed or timed out. A save may have completed; reload and compare before retrying.".to_owned()
    })?;
    if !output.status.success() {
        return Err(
            if crate::isolation::refused(output.status.code(), &output.stderr) {
                "Verb execution unavailable: capacity".to_owned()
            } else {
                "Studio helper unavailable or interrupted. A save may have completed; reload and compare before retrying.".to_owned()
            },
        );
    }
    let reply: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Invalid Studio helper reply; reload before retrying a save")?;
    if reply["protocol"] != 1 || reply["sourceRevision"] != env!("VERB_BUILD_REVISION") {
        return Err(
            "Studio helper version differs from the host; bridge remains unavailable".into(),
        );
    }
    if let Some(error) = reply["error"].as_str() {
        return Err(error.to_owned());
    }
    Ok(reply)
}

pub(crate) fn probe(project: &Path) -> Result<(), String> {
    let reply = exchange(project, &Operation::Capabilities)?;
    if reply["maxBytes"] != MAX_BYTES
        || reply["kind"] != "single-file-html"
        || reply["projectUid"].as_u64()
            != Some(
                std::fs::metadata(project)
                    .map_err(|_| "Could not verify project identity")?
                    .uid() as u64,
            )
    {
        return Err("Unsupported Studio helper capabilities".into());
    }
    Ok(())
}

fn document(reply: Value, path: &str) -> Result<Document, String> {
    let doc: Document = serde_json::from_value(reply["file"].clone())
        .map_err(|_| "Invalid Studio document reply")?;
    if doc.path != path
        || doc.bytes != doc.text.len()
        || doc.bytes > MAX_BYTES
        || doc.text.contains('\0')
        || doc.revision != format!("{:x}", Sha256::digest(doc.text.as_bytes()))
    {
        return Err("Studio document identity or revision mismatch".into());
    }
    Ok(doc)
}

pub(crate) fn read(project: &Path, path: &str) -> Result<Document, String> {
    document(
        exchange(project, &Operation::Read { path: path.into() })?,
        path,
    )
}
pub(crate) fn save(project: &Path, request: SaveRequest) -> Result<Document, String> {
    // Serialize all writes admitted by this host, even though each helper is a fresh process.
    let _guard = SAVES
        .lock()
        .map_err(|_| "Studio boundary save lock unavailable")?;
    probe(project)?;
    let path = request.path.clone();
    let expected = request.text.clone();
    let doc = document(exchange(project, &Operation::Save { request })?, &path)?;
    if doc.text != expected {
        return Err(
            "Studio save reply does not match the submitted text; reload before retrying".into(),
        );
    }
    Ok(doc)
}
