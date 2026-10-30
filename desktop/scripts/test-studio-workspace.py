#!/usr/bin/env python3
"""Real guest adapter contract, using disposable Git fixtures and the fixed embedded source."""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).with_name("studio-workspace.py").read_bytes()
BOOTSTRAP = f"import sys;exec(compile(sys.stdin.buffer.read({len(SOURCE)}),'<verb-studio-adapter>','exec'))"


class StudioWorkspace(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="verb-studio-helper-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        (self.root / ".gitignore").write_text("private.txt\n")
        (self.root / "private.txt").write_text("fixture-only")
        (self.root / "app.html").write_text("<h1>Before</h1>")

    def call(self, request):
        result = subprocess.run([sys.executable, "-I", "-S", "-c", BOOTSTRAP, "fixture-revision"],
                                input=SOURCE + json.dumps(request).encode(), stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, cwd=self.root, timeout=20, check=True)
        reply = json.loads(result.stdout)
        self.assertEqual(reply["protocol"], 1)
        self.assertEqual(reply["sourceRevision"], "fixture-revision")
        return reply

    def save(self, path, revision, text):
        return self.call({"action": "save", "request": {"workspaceId": "fixture", "path": path,
                         "expectedRevision": revision, "text": text}})

    def test_trusted_source_prefix_is_separate_from_json(self):
        self.assertLess(len(BOOTSTRAP), 200)
        self.assertIn("error", self.call({"action":"read","path":"x; print(123)"}))

    def test_save_large_unicode_preserves_mode_and_conflicts(self):
        path = self.root / "app.html"
        path.chmod(0o750)
        read = self.call({"action": "read", "path": "app.html"})["file"]
        text = "₹" * 60_000
        saved = self.save("app.html", read["revision"], text)["file"]
        self.assertEqual(path.read_text(), text)
        self.assertEqual(saved["bytes"], 180_000)
        self.assertEqual(saved["revision"], hashlib.sha256(text.encode()).hexdigest())
        self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o750)
        self.assertTrue(self.save("app.html", read["revision"], "stale")["error"].startswith("studio conflict: "))
        self.assertEqual(path.read_text(), text)
        self.assertEqual(list(self.root.glob(".verb-studio-*.tmp")), [])

    def test_ignored_traversal_links_binary_and_readonly_fail(self):
        os.symlink("app.html", self.root / "link.html")
        os.link(self.root / "app.html", self.root / "hard.html")
        (self.root / "binary").write_bytes(b"x\0y")
        (self.root / "readonly").write_text("readable")
        (self.root / "readonly").chmod(0o400)
        os.symlink(self.root, self.root / "directory-link")
        for path in ("private.txt", "../escape", "/etc/passwd", "link.html", "hard.html",
                     "directory-link/readonly", "binary", "missing"):
            self.assertIn("error", self.call({"action": "read", "path": path}), path)
        read = self.call({"action": "read", "path": "readonly"})["file"]
        self.assertIn("error", self.save("readonly", read["revision"], "changed"))
        self.assertEqual((self.root / "readonly").read_text(), "readable")

    def test_image_magic_bounds_links_and_revision(self):
        raw = b"\x89PNG\r\n\x1a\n" + b"fixture"
        (self.root / "picture.png").write_bytes(raw)
        result = self.call({"action":"image", "path":"picture.png"})["image"]
        self.assertEqual(result["mime"], "image/png")
        self.assertEqual(result["revision"], hashlib.sha256(raw).hexdigest())
        os.symlink("picture.png", self.root / "linked.png")
        (self.root / "not-image.png").write_text('<svg onload="alert(1)"/>')
        (self.root / "too-big.png").write_bytes(raw + b"x" * (2 * 1024 * 1024))
        for path in ("linked.png", "not-image.png", "too-big.png", "private.txt", "../escape.png"):
            self.assertIn("error", self.call({"action":"image", "path":path}), path)

    def test_capabilities_schema_hooks_and_limits(self):
        marker = self.root / "hook-ran"
        hook = self.root / "probe.sh"
        hook.write_text("#!/bin/sh\ntouch hook-ran\n")
        hook.chmod(0o755)
        subprocess.run(["git", "config", "core.fsmonitor", str(hook)], cwd=self.root, check=True)
        self.assertEqual(self.call({"action": "capabilities"})["maxBytes"], 204800)
        self.assertIn("file", self.call({"action": "read", "path": "app.html"}))
        self.assertFalse(marker.exists())
        self.assertIn("error", self.call({"action": "capabilities", "extra": True}))
        self.assertIn("error", self.call({"action": "shell", "command": "anything"}))
        self.assertIn("error", self.save("app.html", "wrong", "x" * 204801))


if __name__ == "__main__":
    unittest.main()
