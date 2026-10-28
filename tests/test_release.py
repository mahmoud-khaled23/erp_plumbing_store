"""Release checks exercise parsers and translation drift without building an installer."""
import importlib.util
from pathlib import Path
import struct
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from test_schema import migrate

ROOT = Path(__file__).resolve().parents[1]


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"scripts/{name}.py")
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


class ReleaseTests(unittest.TestCase):
    def test_installed_store_verifier_rejects_migration_drift(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "store.db"
            db = sqlite3.connect(path, isolation_level=None)
            try:
                db.execute("PRAGMA journal_mode=WAL")
                migrate(db)
            finally:
                db.close()
            command = [sys.executable, str(ROOT / "scripts/check-installed-store.py"), str(path)]
            valid = subprocess.run(command, capture_output=True)
            self.assertEqual(valid.returncode, 0, valid.stderr)
            db = sqlite3.connect(path, isolation_level=None)
            try:
                db.execute("UPDATE schema_migrations SET sha256=? WHERE version=1", ("0" * 64,))
            finally:
                db.close()
            invalid = subprocess.run(command, capture_output=True)
            self.assertNotEqual(invalid.returncode, 0)
            self.assertIn(b"migration hashes differ", invalid.stderr)

    def test_installer_translation_matches_dictionary(self):
        generator = module("generate_installer")
        self.assertEqual(generator.DESTINATION.read_text(encoding="utf-8"), generator.content())

    def test_pe_reader_rejects_text_and_truncation(self):
        reader = module("verify_release")
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "invalid.exe"
            for contents in (b"not an executable", b"MZ" + bytes(62)):
                path.write_bytes(contents)
                with self.assertRaises((ValueError, struct.error)):
                    reader.pe_info(path)

    def test_pe_reader_reads_architecture_and_import_rvas(self):
        reader = module("verify_release")
        image = bytearray(1024)
        image[:2] = b"MZ"
        struct.pack_into("<I", image, 60, 128)
        image[128:132] = b"PE\0\0"
        struct.pack_into("<HH", image, 132, 0x8664, 1)
        struct.pack_into("<H", image, 148, 240)
        struct.pack_into("<H", image, 152, 0x20B)
        struct.pack_into("<I", image, 212, 512)
        struct.pack_into("<I", image, 272, 4096)
        struct.pack_into("<IIII", image, 400, 512, 4096, 512, 512)
        struct.pack_into("<I", image, 524, 4196)
        image[612:625] = b"KERNEL32.dll\0"
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "probe.exe"
            path.write_bytes(image)
            info = reader.pe_info(path)
        self.assertEqual(info["machine"], "0x8664")
        self.assertEqual(info["imports"], ["KERNEL32.dll"])
        self.assertEqual(info["bytes"], 1024)


if __name__ == "__main__":
    unittest.main()
