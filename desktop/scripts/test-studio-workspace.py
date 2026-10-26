#!/usr/bin/env python3
"""Real guest adapter contract, using disposable Git fixtures and the fixed embedded source."""
import base64
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
import zlib

SOURCE = Path(__file__).with_name("studio-workspace.wire").read_text()


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
        result = subprocess.run([sys.executable, "-I", "-S", "-c", SOURCE, "fixture-revision"],
                                input=json.dumps(request).encode(), stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, cwd=self.root, timeout=20, check=True)
        reply = json.loads(result.stdout)
        self.assertEqual(reply["protocol"], 1)
        self.assertEqual(reply["sourceRevision"], "fixture-revision")
        return reply

    def save(self, path, revision, text):
        return self.call({"action": "save", "request": {"workspaceId": "fixture", "path": path,
                         "expectedRevision": revision, "text": text}})

    def test_embedded_wire_is_exact_source_and_fits_boundary_argument(self):
        prefix = "import base64,zlib;exec(zlib.decompress(base64.b64decode("
        self.assertTrue(SOURCE.startswith(prefix) and SOURCE.endswith(")))"))
        packed = json.loads(SOURCE[len(prefix):-3])
        self.assertEqual(zlib.decompress(base64.b64decode(packed)),
                         Path(__file__).with_name("studio-workspace.py").read_bytes())
        self.assertLess(len(SOURCE.encode()), 4096)

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
