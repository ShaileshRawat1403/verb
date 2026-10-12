#!/usr/bin/env python3
"""Private, offline Verb ledger snapshots. Not a project/provider credential backup.

Stop writers before snapshotting. Restore only into a new directory; never into live state.
Only projects, sessions, events and work are included. Logs, tokens, caches, session locks,
provider homes and project source remain outside this deliberately narrow boundary.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import tarfile
import tempfile

ROOTS = {"projects", "sessions", "events", "work"}
MAX_BYTES = 2 * 1024**3
MAX_FILES = 100000
MAX_MANIFEST = 8 * 1024**2


def safe_name(name):
    parts = PurePosixPath(name).parts
    if (not parts or PurePosixPath(name).is_absolute() or
            any(p in (".", "..") for p in parts) or
            str(PurePosixPath(name)) != name or parts[0] not in ROOTS):
        raise ValueError("invalid ledger member")
    if parts[:2] == ("sessions", "locks"):
        raise ValueError("session locks are not restorable state")
    if any(p.startswith(".env") and not p.endswith(".example") for p in parts):
        raise ValueError("environment files are outside the snapshot boundary")
    return name


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def copy_exact(incoming, output, size):
    remaining = size
    while remaining:
        block = incoming.read(min(remaining, 1024 * 1024))
        if not block:
            raise ValueError("file shortened during copy")
        output.write(block)
        remaining -= len(block)
    if incoming.read(1):
        raise ValueError("file grew during copy")


def snapshot(state, destination, quiesced):
    if not quiesced:
        raise ValueError("stop all writers, then explicitly pass --quiesced")
    state = state.resolve(strict=True)
    if not state.is_dir():
        raise ValueError("state is not a directory")
    destination = destination.resolve()
    if destination == state or state in destination.parents:
        raise ValueError("snapshot destination must be outside live state")
    destination.mkdir(mode=0o700)  # refuses an existing destination
    os.chmod(destination, 0o700)
    records = []
    total = 0
    # Stage bytes once: the digest and archive always describe the same copy.
    with tempfile.TemporaryDirectory(prefix=".staging-", dir=destination) as staging:
        staging = Path(staging)
        for root in sorted(ROOTS):
            source_root = state / root
            if not source_root.exists() and not source_root.is_symlink():
                continue
            if source_root.is_symlink() or not source_root.is_dir():
                raise ValueError("ledger root is not a regular directory")
            for directory, dirs, files in os.walk(source_root, followlinks=False):
                directory = Path(directory)
                if directory.is_symlink():
                    raise ValueError("symlink in ledger")
                if directory == state / "sessions":
                    dirs[:] = [name for name in dirs if name != "locks"]
                for name in dirs:
                    if (directory / name).is_symlink():
                        raise ValueError("symlink in ledger")
                for name in sorted(files):
                    source = directory / name
                    relative = safe_name(source.relative_to(state).as_posix())
                    before = source.lstat()
                    if not stat.S_ISREG(before.st_mode):
                        raise ValueError("non-regular ledger file")
                    total += before.st_size
                    if total > MAX_BYTES or len(records) >= MAX_FILES:
                        raise ValueError("snapshot exceeds its budget")
                    copied = staging / relative
                    copied.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
                    with os.fdopen(os.open(source, os.O_RDONLY | os.O_NOFOLLOW), "rb") as incoming:
                        if os.fstat(incoming.fileno()) != before:
                            raise ValueError("ledger changed during snapshot")
                        with copied.open("xb") as output:
                            copy_exact(incoming, output, before.st_size)
                        if source.lstat() != before:
                            raise ValueError("ledger changed during snapshot")
                    os.chmod(copied, 0o600)
                    records.append({"path": relative, "size": before.st_size, "sha256": digest(copied)})
        manifest = json.dumps({"format": 1, "scope": "verb-ledger-only", "files": records}, sort_keys=True).encode()
        if len(manifest) > MAX_MANIFEST:
            raise ValueError("manifest exceeds its budget")
        (staging / "manifest.json").write_bytes(manifest)
        archive = destination / "ledger.tgz"
        with tarfile.open(archive, "w:gz") as output:
            output.add(staging / "manifest.json", arcname="manifest.json", recursive=False)
            for record in records:
                output.add(staging / record["path"], arcname=record["path"], recursive=False)
        os.chmod(archive, 0o600)
        checksum = destination / "ledger.sha256"
        checksum.write_text(digest(archive) + "\n")
        os.chmod(checksum, 0o600)
    return {"files": len(records), "bytes": total, "scope": "verb-ledger-only"}


def restore(snapshot_dir, destination):
    archive = snapshot_dir / "ledger.tgz"
    expected = (snapshot_dir / "ledger.sha256").read_text().strip()
    if len(expected) != 64 or digest(archive) != expected:
        raise ValueError("archive checksum mismatch")
    with tarfile.open(archive, "r:gz") as source:
        members = []
        expanded = 0
        for member in source:
            expanded += member.size
            if (len(members) > MAX_FILES or not member.isfile() or
                    not 0 <= member.size <= MAX_BYTES or expanded > MAX_BYTES + MAX_MANIFEST):
                raise ValueError("invalid archive member")
            members.append(member)
        names = [member.name for member in members]
        if len(set(names)) != len(names) or "manifest.json" not in names:
            raise ValueError("duplicate member or missing manifest")
        metadata = source.getmember("manifest.json")
        if metadata.size > MAX_MANIFEST:
            raise ValueError("manifest exceeds its budget")
        manifest = json.load(source.extractfile(metadata))
        if manifest.get("format") != 1 or manifest.get("scope") != "verb-ledger-only":
            raise ValueError("unsupported snapshot format")
        records = manifest["files"]
        paths = [safe_name(record["path"]) for record in records]
        if len(paths) != len(set(paths)) or set(names) != {"manifest.json", *paths}:
            raise ValueError("archive does not match manifest")
        total = sum(source.getmember(path).size for path in paths)
        if total > MAX_BYTES:
            raise ValueError("restore exceeds its budget")
        destination.mkdir(mode=0o700)  # no merge, overwrite or live-state extraction
        os.chmod(destination, 0o700)
        for record in records:
            member = source.getmember(record["path"])
            if member.size != record["size"]:
                raise ValueError("member size mismatch")
            target = destination / record["path"]
            target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            with source.extractfile(member) as incoming, target.open("xb") as output:
                copy_exact(incoming, output, member.size)
            os.chmod(target, 0o600)
            if digest(target) != record["sha256"]:
                raise ValueError("restored file checksum mismatch")
    return {"verified": True, "files": len(records), "bytes": total, "scope": "verb-ledger-only"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    create = sub.add_parser("snapshot")
    create.add_argument("--state", type=Path, required=True)
    create.add_argument("--destination", type=Path, required=True)
    create.add_argument("--quiesced", action="store_true")
    check = sub.add_parser("restore")
    check.add_argument("--snapshot", type=Path, required=True)
    check.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = (snapshot(args.state, args.destination, args.quiesced) if args.command == "snapshot"
                  else restore(args.snapshot, args.destination))
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError):
        parser.exit(1, "snapshot/restore failed; no live state was overwritten. Review private files locally.\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
