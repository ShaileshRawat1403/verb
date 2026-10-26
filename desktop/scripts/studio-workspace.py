"""Fixed, embedded Studio adapter. Run only via the registered project's execution boundary.

The host supplies this source as a literal Python -I -S -c argument, never project text.
Only JSON data enters stdin. No installation, imports from the project, or host file access.
"""
import hashlib
import json
import os
import stat
import subprocess
import sys
import uuid

MAX_BYTES = 200 * 1024
MAX_INPUT = MAX_BYTES * 6 + 2048
CONFLICT = "studio conflict: "
FLAGS = os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK


def revision(raw):
    return hashlib.sha256(raw).hexdigest()


def document(fd, path):
    before = os.fstat(fd)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise ValueError("Only regular files without hard links are supported")
    if before.st_size > MAX_BYTES:
        raise ValueError("File exceeds the 200 KB editor limit")
    os.lseek(fd, 0, os.SEEK_SET)
    raw = bytearray()
    while len(raw) <= MAX_BYTES:
        part = os.read(fd, min(8192, MAX_BYTES + 1 - len(raw)))
        if not part:
            break
        raw.extend(part)
    after = os.fstat(fd)
    if (before.st_size, before.st_mtime_ns, before.st_ctime_ns, before.st_nlink) != (
        after.st_size, after.st_mtime_ns, after.st_ctime_ns, after.st_nlink
    ):
        raise ValueError(CONFLICT + "File changed while reading; reload before retrying")
    if len(raw) > MAX_BYTES or b"\0" in raw:
        raise ValueError("File is too large or binary")
    return {"path": path, "text": raw.decode("utf-8"), "bytes": len(raw), "revision": revision(raw)}


def location(path):
    if not isinstance(path, str) or not path or "\0" in path:
        raise ValueError("Invalid project-relative path")
    parts = path.split("/")
    if any(p in ("", ".", "..") for p in parts):
        raise ValueError("Invalid project-relative path")
    env = {"PATH": "/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM": "1",
           "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_TERMINAL_PROMPT": "0",
           "GIT_OPTIONAL_LOCKS": "0", "LANG": "C.UTF-8"}
    # Only the requested literal path is listed, so huge repositories cannot produce unbounded
    # output. With --literal-pathspecs, repository names cannot inject pathspec magic.
    with subprocess.Popen(["/usr/bin/git", "--literal-pathspecs", "-c", "core.fsmonitor=false",
                           "-c", "core.hooksPath=/dev/null", "ls-files", "-z", "--cached",
                           "--others", "--exclude-standard", "--", path],
                          env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          stderr=subprocess.DEVNULL) as git:
        try:
            listing = git.stdout.read(8193)
            if len(listing) > 8192:
                raise ValueError("Requested path listing exceeds limit")
            if git.wait(timeout=10) != 0:
                raise ValueError("Could not verify project file visibility")
        finally:
            if git.poll() is None:
                git.kill()
                git.wait()
    if path.encode("utf-8") not in listing.split(b"\0"):
        raise ValueError("File is ignored, absent, or outside the visible listing")
    directory = os.open(".", os.O_RDONLY | os.O_DIRECTORY | FLAGS)
    try:
        for component in parts[:-1]:
            child = os.open(component, os.O_RDONLY | os.O_DIRECTORY | FLAGS, dir_fd=directory)
            os.close(directory)
            directory = child
        return directory, parts[-1]
    except BaseException:
        os.close(directory)
        raise


def read(path):
    directory, leaf = location(path)
    try:
        fd = os.open(leaf, os.O_RDONLY | FLAGS, dir_fd=directory)
        try:
            return document(fd, path)
        finally:
            os.close(fd)
    finally:
        os.close(directory)


def save(request):
    if not isinstance(request, dict) or set(request) != {"workspaceId", "path", "expectedRevision", "text"}:
        raise ValueError("Invalid save request")
    path, text, expected = request["path"], request["text"], request["expectedRevision"]
    if not isinstance(text, str) or not isinstance(expected, str):
        raise ValueError("Invalid save fields")
    raw = text.encode("utf-8")
    if len(raw) > MAX_BYTES or b"\0" in raw:
        raise ValueError("Save requires UTF-8 text up to 200 KB without NUL bytes")
    directory, leaf = location(path)
    original = output = None
    temp = None
    try:
        original = os.open(leaf, os.O_RDONLY | FLAGS, dir_fd=directory)
        before = os.fstat(original)
        if before.st_uid != os.geteuid() or not before.st_mode & stat.S_IWUSR:
            raise ValueError("Only writable files owned by the project user can be replaced")
        if document(original, path)["revision"] != expected:
            raise ValueError(CONFLICT + "File changed on disk; your draft is retained")
        candidate = ".verb-studio-" + uuid.uuid4().hex + ".tmp"
        output = os.open(candidate, os.O_WRONLY | os.O_CREAT | os.O_EXCL | FLAGS, 0o600, dir_fd=directory)
        temp = candidate  # Never unlink a name unless exclusive creation succeeded.
        with os.fdopen(output, "wb", closefd=False) as stream:
            stream.write(raw)
            stream.flush()
        os.fchmod(output, stat.S_IMODE(before.st_mode) & 0o777)
        os.fsync(output)
        current = os.open(leaf, os.O_RDONLY | FLAGS, dir_fd=directory)
        try:
            after = os.fstat(current)
            if (before.st_dev, before.st_ino, before.st_mode, before.st_uid) != (
                after.st_dev, after.st_ino, after.st_mode, after.st_uid
            ) or document(current, path)["revision"] != expected:
                raise ValueError(CONFLICT + "File changed before saving; your draft is retained")
        finally:
            os.close(current)
        os.replace(temp, leaf, src_dir_fd=directory, dst_dir_fd=directory)
        temp = None
        os.fsync(directory)  # Failure here is ambiguous, never auto-retry.
        return {"path": path, "text": text, "bytes": len(raw), "revision": revision(raw)}
    finally:
        if output is not None:
            os.close(output)
        if original is not None:
            os.close(original)
        if temp is not None:
            os.unlink(temp, dir_fd=directory)
        os.close(directory)


def main():
    raw = sys.stdin.buffer.read(MAX_INPUT + 1)
    if len(raw) > MAX_INPUT:
        raise ValueError("Studio helper input exceeds limit")
    request = json.loads(raw)
    if not isinstance(request, dict):
        raise ValueError("Invalid Studio helper request")
    action = request.get("action")
    if action == "capabilities" and set(request) == {"action"}:
        return {"maxBytes": MAX_BYTES, "kind": "single-file-html", "projectUid": os.geteuid()}
    if action == "read" and set(request) == {"action", "path"}:
        return {"file": read(request["path"])}
    if action == "save" and set(request) == {"action", "request"}:
        return {"file": save(request["request"])}
    raise ValueError("Invalid Studio helper operation")


try:
    reply = main()
except (OSError, ValueError, UnicodeError, subprocess.SubprocessError) as error:
    reply = {"error": str(error)}
reply.update(protocol=1, sourceRevision=sys.argv[1])
sys.stdout.write(json.dumps(reply, ensure_ascii=True))
