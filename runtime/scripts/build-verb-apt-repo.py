#!/usr/bin/env python3
"""Build a flat Verb APT repository into an output directory.

Produces hand-crafted ``.deb`` packages that install under Verb's private
prefix (``/data/data/com.aistudio.verb.app/files``) plus a flat-format
``Packages`` index suitable for an apt source of the form::

    deb [trusted=yes] <repo-url>/ ./ 

The payloads are pure shell/data files (no native code), so they validate the
whole apt pipeline - index fetch, trusted fetch without gpgv, download,
dependency resolution and install - without needing a full termux-packages
source build. Native packages built by the CI pipeline are appended to this
same repository in a follow-up.

Inside the proot guest the Verb files directory appears at the Termux guest
path, so installed scripts use the guest interpreter path
``/data/data/com.termux/files/usr/bin/sh`` even though the archive places them
at the real Verb prefix (dpkg installs to disk, proot re-maps at runtime).
"""

from __future__ import annotations

import argparse
import hashlib
import io
import os
import sys
import tarfile

ARCH = "aarch64"
VERB_PREFIX = "data/data/com.aistudio.verb.app/files"
GUEST_SH = "/data/data/com.termux/files/usr/bin/sh"

PKG_NAME = "Verb"
PKG_MAINTAINER = "Verb <verb@aistudio.local>"


def tar_gz_members(files: dict[str, bytes], modes: dict[str, int]) -> bytes:
    """Build a gzip'd tar from {archive_path: content}, with per-path modes."""
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz", format=tarfile.GNU_FORMAT) as tar:
        for path, content in files.items():
            info = tarfile.TarInfo(path)
            info.size = len(content)
            info.mode = modes.get(path, 0o644)
            info.mtime = 0
            tar.addfile(info, io.BytesIO(content))
    return buf.getvalue()


def ar_member(name: str, data: bytes) -> bytes:
    header = name.ljust(16)[:16]
    header += str(0).rjust(12)
    header += "0".rjust(6)
    header += "0".rjust(6)
    header += str(0o100644).rjust(8)
    header += str(len(data)).rjust(10)
    header += "`\n"
    if len(data) % 2 != 0:
        data += b"\n"
    return header.encode("ascii") + data


def build_deb(out_dir: str, package: str, version: str, control: dict,
              payload: dict[str, bytes], modes: dict[str, int]) -> str:
    """Write one .deb and return its absolute path."""
    control_fields = "\n".join(f"{k}: {v}" for k, v in control.items()) + "\n"
    control_tar = tar_gz_members({"./control": control_fields.encode()}, {"./control": 0o644})
    data_tar = tar_gz_members(payload, modes)
    ar_data = (
        b"!<arch>\n"
        + ar_member("debian-binary", b"2.0\n")
        + ar_member("control.tar.gz", control_tar)
        + ar_member("data.tar.gz", data_tar)
    )
    os.makedirs(out_dir, exist_ok=True)
    path = os.path.join(out_dir, f"{package}_{version}_{ARCH}.deb")
    with open(path, "wb") as fh:
        fh.write(ar_data)
    return path


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha1(data: bytes) -> str:
    return hashlib.sha1(data).hexdigest()


def md5(data: bytes) -> str:
    return hashlib.md5(data).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description="Build the Verb flat APT repository")
    parser.add_argument("--out", default="./apt-repo", help="output directory")
    parser.add_argument("--version", default="0.1.0", help="package version")
    args = parser.parse_args()

    version = args.version
    installed_size = 8

    hello_script = (
        "#!/data/data/com.termux/files/usr/bin/sh\n"
        "# Verb package-registry test payload.\n"
        "git_ok=$(command -v git >/dev/null 2>&1 && echo present || echo missing)\n"
        "printf 'verb-hello: apt install works (git=%s, prefix=%s)\\n' \"$git_ok\" \"$PREFIX\"\n"
    )
    status_script = (
        "#!/data/data/com.termux/files/usr/bin/sh\n"
        "# Verb package-registry test payload depending on verb-hello.\n"
        "hello_ok=$(command -v verb-hello >/dev/null 2>&1 && echo present || echo missing)\n"
        "printf 'verb-tools: dependency verb-hello=%s\\n' \"$hello_ok\"\n"
    )

    hello_payload = {
        f"{VERB_PREFIX}/usr/bin/verb-hello": hello_script.encode(),
    }
    hello_modes = {f"{VERB_PREFIX}/usr/bin/verb-hello": 0o755}

    tools_payload = {
        f"{VERB_PREFIX}/usr/bin/verb-status": status_script.encode(),
    }
    tools_modes = {f"{VERB_PREFIX}/usr/bin/verb-status": 0o755}

    built = []

    hello_deb = build_deb(
        args.out, "verb-hello", version,
        control={
            "Package": "verb-hello",
            "Version": version,
            "Architecture": ARCH,
            "Maintainer": PKG_MAINTAINER,
            "Installed-Size": str(installed_size),
            "Depends": "git",
            "Section": "misc",
            "Priority": "optional",
            "Description": "Verb package-registry test",
        },
        payload=hello_payload,
        modes=hello_modes,
    )
    built.append(hello_deb)

    tools_deb = build_deb(
        args.out, "verb-tools", version,
        control={
            "Package": "verb-tools",
            "Version": version,
            "Architecture": ARCH,
            "Maintainer": PKG_MAINTAINER,
            "Installed-Size": str(installed_size),
            "Depends": "verb-hello",
            "Section": "misc",
            "Priority": "optional",
            "Description": "Verb package-registry dependency test",
        },
        payload=tools_payload,
        modes=tools_modes,
    )
    built.append(tools_deb)

    # Flat-format Packages index. Filename entries are relative to the repo URL.
    def control_of(deb: str) -> dict[str, str]:
        # Parse the control member out of the ar archive.
        with open(deb, "rb") as fh:
            raw = fh.read()
        idx = raw.find(b"control.tar.gz")
        # find member size for control.tar.gz
        # ar header is 60 bytes; scan members sequentially.
        pos = 8  # after !<arch>
        while pos < len(raw):
            header = raw[pos:pos + 60]
            if len(header) < 60:
                break
            mname = header[:16].decode().strip()
            size = int(header[48:58].strip() or b"0")
            start = pos + 60
            if mname == "control.tar.gz":
                import tarfile as _tf
                with _tf.open(fileobj=io.BytesIO(raw[start:start + size]), mode="r:gz") as tar:
                    member = next((m for m in tar.getmembers()
                                   if m.name.strip("./") == "control"), None)
                    if member is None:
                        raise RuntimeError(f"control member missing in {deb}")
                    ctl_text = tar.extractfile(member).read().decode()
                return dict(line.split(": ", 1) for line in ctl_text.splitlines() if ": " in line)
            pos = start + size
            if size % 2 != 0:
                pos += 1
        raise RuntimeError(f"control not found in {deb}")

    packages_lines = []
    for deb in built:
        name = os.path.basename(deb)
        ctl = control_of(deb)
        with open(deb, "rb") as fh:
            data = fh.read()
        lines = [
            f"Package: {ctl['Package']}",
            f"Version: {ctl['Version']}",
            f"Architecture: {ctl['Architecture']}",
            f"Maintainer: {ctl['Maintainer']}",
            f"Installed-Size: {ctl['Installed-Size']}",
            f"Depends: {ctl.get('Depends', '')}".rstrip(),
            f"Section: {ctl.get('Section', 'misc')}",
            f"Priority: {ctl.get('Priority', 'optional')}",
            f"Filename: ./{name}",
            f"Size: {len(data)}",
            f"MD5sum: {md5(data)}",
            f"SHA1: {sha1(data)}",
            f"SHA256: {sha256(data)}",
            f"Description: {ctl['Description']}",
        ]
        packages_lines.append("\n".join(lines))

    packages_path = os.path.join(args.out, "Packages")
    with open(packages_path, "w") as fh:
        fh.write("\n\n".join(packages_lines) + "\n")

    print(f"Wrote {len(built)} packages + Packages index into {args.out}/")
    for deb in built:
        print(f"  {os.path.basename(deb)} ({os.path.getsize(deb)} bytes)")
    print(f"  Packages ({os.path.getsize(packages_path)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
