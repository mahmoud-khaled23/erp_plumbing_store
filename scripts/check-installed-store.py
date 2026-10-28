"""Read-only integrity check for the disposable Windows installation smoke test."""
from pathlib import Path
from contextlib import closing
import hashlib
import sqlite3
import sys

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


path = Path(sys.argv[1]).resolve()
expected = [(int(source.name.split('_')[0]), source.name, hashlib.sha256(source.read_bytes()).hexdigest())
            for source in sorted((ROOT / 'src-tauri/migrations').glob('*.sql'))]
require(bool(expected), 'No reference migrations found beside the test scripts.')
with closing(sqlite3.connect(path.as_uri() + "?mode=ro", uri=True)) as db:
    require(db.execute("PRAGMA integrity_check").fetchall() == [("ok",)], 'SQLite integrity failed.')
    require(not db.execute("PRAGMA foreign_key_check").fetchall(), 'Foreign-key integrity failed.')
    require(db.execute("PRAGMA journal_mode").fetchone()[0] == "wal", 'WAL was not enabled.')
    actual = db.execute("SELECT version,name,sha256 FROM schema_migrations ORDER BY version").fetchall()
    require(actual == expected, 'Installed migration hashes differ from this source checkout.')
    require(db.execute("SELECT setup_completed_at FROM store_settings").fetchone()[0] is None,
            'Expected a fresh first-run database.')
print("Installed application created an intact WAL database and the first-run schema.")
