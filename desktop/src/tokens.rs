//! Scoped access tokens, for agents and automation that drive Verb on the owner's behalf.
//!
//! The owner's token (printed by `verb web`, or `VERB_TOKEN`) can do everything. An access token is
//! issued by the owner from the command line, for one purpose: it has a name that the audit trail
//! records instead of the owner, a scope, an expiry, and it can be revoked. Only a hash is stored;
//! the token itself is shown once.
//!
//! Scopes, from least to most:
//! - `read`: specs, Git, the file tree and previews, host and hub, Ask Verb. Not terminal screens,
//!   which can show secrets.
//! - `drive`: everything an agent needs to do work: terminals, agents, stages, proofs, commits.
//!   A terminal runs as the same OS user as Verb, so `drive` is not containment: it buys a name in
//!   the audit trail, an expiry and revocation. Isolation is a separate layer.
//! - owner only: issuing tokens, the privacy settings (agent stream, observer) and phone sharing.
//!   No access token can reach these, so a token can never mint a stronger one.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Scope {
    Read,
    Drive,
    Owner,
}

impl Scope {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Scope::Read => "read",
            Scope::Drive => "drive",
            Scope::Owner => "owner",
        }
    }
}

/// Who is making a request, as far as Verb can tell.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Access {
    pub scope: Scope,
    /// The access token's name; `None` for the owner.
    pub token: Option<String>,
    /// Unix seconds; `None` for the owner.
    pub expires: Option<u64>,
}

impl Access {
    pub(crate) fn owner() -> Self {
        Access {
            scope: Scope::Owner,
            token: None,
            expires: None,
        }
    }

    pub(crate) fn allows(&self, needed: Scope) -> bool {
        self.scope >= needed
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Token {
    pub id: String,
    pub name: String,
    pub scope: Scope,
    pub created: u64,
    pub expires: u64,
    /// Hex SHA-256 of the secret part.
    hash: String,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub last_used: Option<u64>,
}

const PREFIX: &str = "verb_at_";
pub(crate) const MAX_TTL_SECS: u64 = 30 * 24 * 3600;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn store_path() -> Result<PathBuf, String> {
    Ok(crate::state_root()?.join("access-tokens.json"))
}

fn load(path: &Path) -> Vec<Token> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save(path: &Path, tokens: &[Token]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {dir:?}: {e}"))?;
    }
    let json = serde_json::to_vec_pretty(tokens).map_err(|e| e.to_string())?;
    crate::fsutil::atomic_write(path, &json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("could not restrict {path:?}: {e}"))?;
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash(secret: &str) -> String {
    hex(&Sha256::digest(secret.as_bytes()))
}

fn random_hex(bytes: usize) -> Result<String, String> {
    use std::io::Read;
    let mut buf = vec![0_u8; bytes];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut r| r.read_exact(&mut buf))
        .map_err(|e| format!("could not read randomness: {e}"))?;
    Ok(hex(&buf))
}

/// Equal-time comparison, so response timing never hints at how much of a token was right.
pub(crate) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// "30m", "8h", "7d" (or plain seconds) to seconds, within 1 minute and 30 days.
pub(crate) fn parse_ttl(text: &str) -> Result<u64, String> {
    let text = text.trim();
    let (number, unit) = text.split_at(text.trim_end_matches(char::is_alphabetic).len());
    let n: u64 = number
        .parse()
        .map_err(|_| format!("expiry `{text}`: use a number with m, h or d, such as 8h"))?;
    let secs = match unit {
        "" | "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86400,
        _ => return Err(format!("expiry `{text}`: use m, h or d")),
    };
    if !(60..=MAX_TTL_SECS).contains(&secs) {
        return Err("an access token lasts between 1 minute and 30 days".to_owned());
    }
    Ok(secs)
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
}

/// Issues a token. Returns its record and the token itself, which is never stored or shown again.
pub(crate) fn create(name: &str, scope: Scope, ttl_secs: u64) -> Result<(Token, String), String> {
    create_in(&store_path()?, name, scope, ttl_secs)
}

fn create_in(
    store: &Path,
    name: &str,
    scope: Scope,
    ttl_secs: u64,
) -> Result<(Token, String), String> {
    let name = name.trim();
    if !valid_name(name) {
        return Err("a token name is 1–40 letters, digits, spaces, '-', '_' or '.'".to_owned());
    }
    if scope == Scope::Owner {
        return Err(
            "access tokens are read or drive; only the owner's token has owner scope".to_owned(),
        );
    }
    if !(60..=MAX_TTL_SECS).contains(&ttl_secs) {
        return Err("an access token lasts between 1 minute and 30 days".to_owned());
    }
    let mut tokens = load(store);
    if tokens
        .iter()
        .any(|t| t.name == name && !t.revoked && t.expires > now())
    {
        return Err(format!(
            "a live token is already named `{name}`; revoke it first"
        ));
    }
    let id = random_hex(4)?;
    let secret = random_hex(32)?;
    let created = now();
    let token = Token {
        id: id.clone(),
        name: name.to_owned(),
        scope,
        created,
        expires: created + ttl_secs,
        hash: hash(&secret),
        revoked: false,
        last_used: None,
    };
    // Forget long-dead records so the file stays small.
    tokens.retain(|t| t.expires + 30 * 86400 > created);
    tokens.push(token.clone());
    save(store, &tokens)?;
    Ok((token, format!("{PREFIX}{id}_{secret}")))
}

pub(crate) fn list() -> Vec<Token> {
    store_path().map(|p| load(&p)).unwrap_or_default()
}

/// Revokes by id or name. Revoked tokens stop working on their next request.
pub(crate) fn revoke(which: &str) -> Result<Token, String> {
    revoke_in(&store_path()?, which)
}

fn revoke_in(store: &Path, which: &str) -> Result<Token, String> {
    let mut tokens = load(store);
    let found = tokens
        .iter_mut()
        .filter(|t| !t.revoked)
        .find(|t| t.id == which || t.name == which)
        .ok_or_else(|| format!("no live token `{which}`"))?;
    found.revoked = true;
    let out = found.clone();
    save(store, &tokens)?;
    Ok(out)
}

/// The access a presented access token grants, if it is genuine, live and unrevoked.
pub(crate) fn verify(presented: &str) -> Option<Access> {
    verify_in(&store_path().ok()?, presented)
}

fn verify_in(store: &Path, presented: &str) -> Option<Access> {
    let rest = presented.strip_prefix(PREFIX)?;
    let (id, secret) = rest.split_once('_')?;
    let mut tokens = load(store);
    let at = now();
    let token = tokens.iter_mut().find(|t| t.id == id)?;
    if !constant_time_eq(token.hash.as_bytes(), hash(secret).as_bytes())
        || token.revoked
        || token.expires <= at
    {
        return None;
    }
    let access = Access {
        scope: token.scope,
        token: Some(token.name.clone()),
        expires: Some(token.expires),
    };
    // Record use at most once a minute; a failed write never refuses a genuine token.
    if token.last_used.is_none_or(|t| at.saturating_sub(t) >= 60) {
        token.last_used = Some(at);
        let _ = save(store, &tokens);
    }
    Some(access)
}

/// The least scope a request needs. Anything not listed needs `drive`; the privacy settings,
/// token management and phone sharing need the owner.
pub(crate) fn required_scope(method: &str, path: &str) -> Scope {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let owner_only = path.starts_with("/api/tokens")
        || (method != "GET" && (path == "/api/stream" || path == "/api/observer"))
        || matches!(parts.as_slice(), ["api", "terminals", _, "phone", ..]);
    if owner_only {
        return Scope::Owner;
    }
    let read = match method {
        "GET" => {
            matches!(
                path,
                "/api/state"
                    | "/api/workspace"
                    | "/api/specs"
                    | "/api/git"
                    | "/api/git/diff"
                    | "/api/files"
                    | "/api/files/preview"
                    | "/api/host"
                    | "/api/hub"
                    | "/api/checks"
                    | "/api/meters"
                    | "/api/observer"
                    | "/api/stream"
                    | "/api/whoami"
            ) || matches!(
                parts.as_slice(),
                ["api", "specs", _] | ["api", "specs", _, "stage-check"]
            )
        }
        // Ask Verb only reads the project's evidence.
        "POST" => path == "/api/ask",
        _ => false,
    };
    if read {
        Scope::Read
    } else {
        Scope::Drive
    }
}

/// `verb token create|list|revoke`.
pub(crate) fn command(args: &[String]) -> Result<(), String> {
    let usage = "usage: verb token create --name NAME [--scope read|drive] [--expires 8h]\n       verb token list\n       verb token revoke ID|NAME";
    match args.first().map(String::as_str) {
        Some("create") => {
            let mut name = None;
            let mut scope = Scope::Drive;
            let mut ttl = 8 * 3600;
            let mut rest = args[1..].iter();
            while let Some(flag) = rest.next() {
                let value = rest.next().ok_or(usage)?;
                match flag.as_str() {
                    "--name" => name = Some(value.clone()),
                    "--scope" => {
                        scope = match value.as_str() {
                            "read" => Scope::Read,
                            "drive" => Scope::Drive,
                            _ => return Err("scope is read or drive".to_owned()),
                        }
                    }
                    "--expires" => ttl = parse_ttl(value)?,
                    _ => return Err(usage.to_owned()),
                }
            }
            let (token, secret) = create(&name.ok_or(usage)?, scope, ttl)?;
            println!("{secret}");
            eprintln!(
                "Access token `{}` ({}), expires {}. Shown once; Verb keeps only its hash.\nRevoke with: verb token revoke {}",
                token.name,
                token.scope.name(),
                crate::iso8601(u128::from(token.expires) * 1000),
                token.id
            );
            Ok(())
        }
        Some("list") => {
            let at = now();
            let tokens = list();
            if tokens.is_empty() {
                println!("No access tokens.");
            }
            for t in tokens {
                let state = if t.revoked {
                    "revoked".to_owned()
                } else if t.expires <= at {
                    "expired".to_owned()
                } else {
                    format!("expires {}", crate::iso8601(u128::from(t.expires) * 1000))
                };
                let used = t
                    .last_used
                    .map(|u| format!("last used {}", crate::iso8601(u128::from(u) * 1000)))
                    .unwrap_or_else(|| "never used".to_owned());
                println!(
                    "{}  {:<20} {:<6} {state} · {used}",
                    t.id,
                    t.name,
                    t.scope.name()
                );
            }
            Ok(())
        }
        Some("revoke") => {
            let token = revoke(args.get(1).ok_or(usage)?)?;
            println!("Revoked `{}` ({}).", token.name, token.id);
            Ok(())
        }
        _ => Err(usage.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_order_and_the_route_policy() {
        assert!(Access::owner().allows(Scope::Owner));
        let drive = Access {
            scope: Scope::Drive,
            token: Some("claude".into()),
            expires: Some(1),
        };
        assert!(drive.allows(Scope::Read) && drive.allows(Scope::Drive));
        assert!(!drive.allows(Scope::Owner));

        assert_eq!(required_scope("GET", "/api/specs"), Scope::Read);
        assert_eq!(required_scope("GET", "/api/specs/001"), Scope::Read);
        assert_eq!(required_scope("POST", "/api/ask"), Scope::Read);
        assert_eq!(
            required_scope("GET", "/api/terminals/abc/output"),
            Scope::Drive,
            "screens can hold secrets"
        );
        assert_eq!(required_scope("POST", "/api/terminals"), Scope::Drive);
        assert_eq!(required_scope("POST", "/api/specs/001/stage"), Scope::Drive);
        assert_eq!(required_scope("POST", "/api/git/commit"), Scope::Drive);
        assert_eq!(
            required_scope("POST", "/api/stream"),
            Scope::Owner,
            "privacy setting"
        );
        assert_eq!(
            required_scope("POST", "/api/observer"),
            Scope::Owner,
            "privacy setting"
        );
        assert_eq!(required_scope("GET", "/api/stream"), Scope::Read);
        assert_eq!(
            required_scope("POST", "/api/terminals/abc/phone"),
            Scope::Owner
        );
        assert_eq!(
            required_scope("POST", "/api/tokens"),
            Scope::Owner,
            "no token mints a token"
        );
        assert_eq!(required_scope("DELETE", "/api/talk/x"), Scope::Drive);
    }

    #[test]
    fn expiry_parsing_is_bounded() {
        assert_eq!(parse_ttl("8h").unwrap(), 8 * 3600);
        assert_eq!(parse_ttl("30m").unwrap(), 1800);
        assert_eq!(parse_ttl("7d").unwrap(), 7 * 86400);
        assert!(parse_ttl("31d").is_err());
        assert!(parse_ttl("10s").is_err());
        assert!(parse_ttl("soon").is_err());
    }

    #[test]
    fn constant_time_comparison() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn a_token_works_until_revoked_and_only_its_hash_is_stored() {
        let dir =
            std::env::temp_dir().join(format!("verb-tokens-{}-{}", std::process::id(), now()));
        let store = dir.join("access-tokens.json");
        let (record, secret) = create_in(&store, "claude", Scope::Drive, 3600).unwrap();
        let stored = std::fs::read_to_string(&store).unwrap();
        assert!(
            !stored.contains(secret.rsplit('_').next().unwrap()),
            "secret never stored"
        );
        assert_eq!(
            verify_in(&store, &secret),
            Some(Access {
                scope: Scope::Drive,
                token: Some("claude".into()),
                expires: Some(record.expires)
            })
        );
        assert!(
            verify_in(&store, &secret.replace(&record.id, "00000000")).is_none(),
            "unknown id"
        );
        let mut wrong = secret.clone();
        wrong.pop();
        wrong.push('x');
        assert!(verify_in(&store, &wrong).is_none(), "wrong secret");
        assert!(verify_in(&store, "not-a-token").is_none());
        assert!(
            create_in(&store, "claude", Scope::Read, 3600).is_err(),
            "one live token per name"
        );
        assert!(
            create_in(&store, "root", Scope::Owner, 3600).is_err(),
            "no owner tokens"
        );
        revoke_in(&store, "claude").unwrap();
        assert!(verify_in(&store, &secret).is_none(), "revoked");
        let _ = std::fs::remove_dir_all(dir);
    }
}
