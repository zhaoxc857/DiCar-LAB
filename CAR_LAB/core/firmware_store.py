"""Firmware version library: every flash attempt gets a snapshot, a note,
and a result - enabling history review and one-click rollback.

Snapshots live in data_root()/firmware_library/ named by SHA256 + original
suffix, so re-flashing the identical image never duplicates storage. Rows
record the vehicle, chip family, user note, source path, size, digest and
final result (pending/ok/cancelled/failed).

Connections are opened and closed per operation (sqlite3's `with con`
only commits, it does not close) so the database file never stays locked.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import sqlite3
import time
from pathlib import Path

from core.paths import data_root

DB_PATH = data_root() / "data" / "firmware_library.db"
LIBRARY_DIR = data_root() / "firmware_library"

RESULT_OK = "ok"
RESULT_CANCELLED = "cancelled"
RESULT_FAILED = "failed"


class FirmwareStore:
    def __init__(self, path: Path | str | None = None, library_dir: Path | str | None = None):
        self.path = Path(path) if path else DB_PATH
        self.library_dir = Path(library_dir) if library_dir else LIBRARY_DIR
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.library_dir.mkdir(parents=True, exist_ok=True)
        self._init_db()

    def _connect(self):
        con = sqlite3.connect(self.path)
        con.row_factory = sqlite3.Row
        return con

    def _run(self, sql: str, params: tuple = ()):
        """Execute one statement in a transaction and close the connection."""
        con = self._connect()
        try:
            with con:
                return con.execute(sql, params)
        finally:
            con.close()

    def _fetchone(self, sql: str, params: tuple = ()):
        con = self._connect()
        try:
            return con.execute(sql, params).fetchone()
        finally:
            con.close()

    def _fetchall(self, sql: str, params: tuple = ()):
        con = self._connect()
        try:
            return con.execute(sql, params).fetchall()
        finally:
            con.close()

    def _init_db(self):
        self._run(
            """
            CREATE TABLE IF NOT EXISTS firmware_versions(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at REAL NOT NULL,
                vehicle TEXT NOT NULL DEFAULT '',
                family TEXT NOT NULL DEFAULT '',
                note TEXT NOT NULL DEFAULT '',
                source_path TEXT NOT NULL DEFAULT '',
                sha256 TEXT NOT NULL,
                size INTEGER NOT NULL,
                snapshot_path TEXT NOT NULL DEFAULT '',
                result TEXT NOT NULL DEFAULT 'pending'
            )
            """
        )
        self._run(
            "CREATE INDEX IF NOT EXISTS idx_firmware_created "
            "ON firmware_versions(created_at DESC)"
        )

    def record(self, vehicle: str, family: str, firmware_path: str, note: str = "") -> int:
        """Snapshot the image (dedup by SHA256) and insert a pending row."""
        source = Path(firmware_path)
        data = source.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        suffix = source.suffix.lower() or ".bin"
        snapshot = self.library_dir / f"{digest}{suffix}"
        if not snapshot.exists():
            tmp = snapshot.with_name(snapshot.name + ".tmp")
            shutil.copyfile(source, tmp)
            os.replace(tmp, snapshot)
        cur = self._run(
            "INSERT INTO firmware_versions(created_at,vehicle,family,note,"
            "source_path,sha256,size,snapshot_path,result) VALUES(?,?,?,?,?,?,?,?,?)",
            (time.time(), str(vehicle), str(family), str(note), str(source),
             digest, len(data), str(snapshot), "pending"),
        )
        return int(cur.lastrowid)

    def set_result(self, version_id: int, result: str) -> None:
        self._run(
            "UPDATE firmware_versions SET result=? WHERE id=?",
            (str(result), int(version_id)),
        )

    def update_note(self, version_id: int, note: str) -> None:
        self._run(
            "UPDATE firmware_versions SET note=? WHERE id=?",
            (str(note), int(version_id)),
        )

    def list(self, vehicle: str | None = None, limit: int = 200) -> list:
        query = (
            "SELECT id,created_at,vehicle,family,note,source_path,sha256,size,"
            "snapshot_path,result FROM firmware_versions"
        )
        params: tuple = ()
        if vehicle is not None:
            query += " WHERE vehicle=?"
            params = (str(vehicle),)
        query += " ORDER BY id DESC LIMIT ?"
        params = params + (int(limit),)
        return [dict(r) for r in self._fetchall(query, params)]

    def get(self, version_id: int):
        row = self._fetchone(
            "SELECT * FROM firmware_versions WHERE id=?", (int(version_id),)
        )
        return dict(row) if row else None

    def delete(self, version_id: int) -> None:
        row = self.get(version_id)
        if row is None:
            return
        self._run("DELETE FROM firmware_versions WHERE id=?", (int(version_id),))
        snapshot = row.get("snapshot_path", "")
        if snapshot and not self._snapshot_in_use(snapshot):
            try:
                Path(snapshot).unlink(missing_ok=True)
            except OSError:
                pass

    def _snapshot_in_use(self, snapshot: str) -> bool:
        return self._fetchone(
            "SELECT 1 FROM firmware_versions WHERE snapshot_path=? LIMIT 1",
            (str(snapshot),),
        ) is not None
