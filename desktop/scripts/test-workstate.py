#!/usr/bin/env python3
"""Offline recovery and hostile archive checks; no developer or provider state."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("workstate", Path(__file__).with_name("workstate.py"))
workstate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(workstate)


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.state = self.root / "live"
        (self.state / "sessions/locks").mkdir(parents=True)
        (self.state / "sessions/s-example.session").write_bytes(b"id=example\nresume_id=native-id\n")
        (self.state / "work").mkdir()
        (self.state / "work/tasks.json").write_text('{"task":"retained"}')
        (self.state / "access-tokens.json").write_text("SYNTHETIC_SECRET")
        (self.state / "launch.log").write_text("SYNTHETIC_SECRET")
        (self.state / "sessions/locks/example.lock").write_text("transient")
        self.backup = self.root / "backup"
        self.restored = self.root / "restored"

    def tearDown(self):
        self.temp.cleanup()

    def snapshot(self):
        return workstate.snapshot(self.state, self.backup, True)

    def malicious(self, name):
        self.backup.mkdir()
        record = {"path": name, "size": 1, "sha256": "a" * 64}
        metadata = json.dumps({"format": 1, "scope": "verb-ledger-only", "files": [record]}).encode()
        archive = self.backup / "ledger.tgz"
        with tarfile.open(archive, "w:gz") as output:
            for path, value in [("manifest.json", metadata), (name, b"x")]:
                member = tarfile.TarInfo(path)
                member.size = len(value)
                output.addfile(member, io.BytesIO(value))
        (self.backup / "ledger.sha256").write_text(workstate.digest(archive))

    def test_roundtrip_retains_identity_tasks_and_private_permissions_without_credentials(self):
        self.assertEqual(self.snapshot()["files"], 2)
        self.assertTrue(workstate.restore(self.backup, self.restored)["verified"])
        for path in ["sessions/s-example.session", "work/tasks.json"]:
            self.assertEqual((self.state/path).read_bytes(), (self.restored/path).read_bytes())
            self.assertEqual((self.restored/path).stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.restored.stat().st_mode & 0o777, 0o700)
        self.assertFalse((self.restored/"access-tokens.json").exists())
        self.assertFalse((self.restored/"launch.log").exists())
        self.assertFalse((self.restored/"sessions/locks").exists())

    def test_requires_explicit_quiescence_and_destination_outside_state(self):
        with self.assertRaises(ValueError):
            workstate.snapshot(self.state, self.backup, False)
        with self.assertRaises(ValueError):
            workstate.snapshot(self.state, self.state/"backup", True)

    def test_never_merges_or_overwrites_existing_restore_destination(self):
        self.snapshot()
        with self.assertRaises(FileExistsError):
            workstate.restore(self.backup, self.state)
        self.assertEqual((self.state/"work/tasks.json").read_text(), '{"task":"retained"}')

    def test_read_access_time_updates_do_not_reject_a_quiesced_snapshot(self):
        source = self.state / "work/tasks.json"
        os.utime(source, ns=(0, source.stat().st_mtime_ns))
        self.snapshot()
        self.assertTrue(workstate.restore(self.backup, self.restored)["verified"])
        self.assertEqual((self.restored / "work/tasks.json").read_bytes(), source.read_bytes())

    def test_concurrent_write_is_rejected_even_when_file_size_is_unchanged(self):
        copy = workstate.copy_exact
        def mutate(incoming, output, size):
            copy(incoming, output, size)
            # The staging path identifies the fixture being copied from the descriptor.
            if output.name.endswith("work/tasks.json"):
                source = self.state / "work/tasks.json"
                source.write_bytes(b"x" * size)
        with mock.patch.object(workstate, "copy_exact", side_effect=mutate):
            with self.assertRaisesRegex(ValueError, "ledger changed during snapshot"):
                self.snapshot()
        self.assertFalse((self.backup / "ledger.tgz").exists())

    def test_archive_corruption_rejected_before_destination_creation(self):
        self.snapshot()
        with (self.backup/"ledger.tgz").open("ab") as output:
            output.write(b"corrupt")
        with self.assertRaises(ValueError):
            workstate.restore(self.backup, self.restored)
        self.assertFalse(self.restored.exists())

    def test_traversal_and_out_of_scope_credentials_rejected_before_extract(self):
        for name in ["../escape", "/absolute", "sessions/../../escape", "access-tokens.json", "work/.env", "sessions/locks/a", "sessions//a"]:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory(dir=self.root) as directory:
                    self.backup = Path(directory)/"backup"
                    self.malicious(name)
                    with self.assertRaises(ValueError):
                        workstate.restore(self.backup, self.restored)
                    self.assertFalse(self.restored.exists())

    def test_symlink_to_private_file_is_not_followed(self):
        (self.state/"work/shortcut").symlink_to(self.state/"access-tokens.json")
        with self.assertRaises(ValueError):
            self.snapshot()
        self.assertFalse((self.backup/"ledger.tgz").exists())

    def test_unbounded_growth_is_refused(self):
        with self.assertRaises(ValueError):
            workstate.copy_exact(io.BytesIO(b"too long"), io.BytesIO(), 3)


if __name__ == "__main__":
    unittest.main()
