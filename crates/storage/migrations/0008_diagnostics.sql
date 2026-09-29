CREATE TABLE diagnostic_logs (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 code TEXT NOT NULL, message TEXT NOT NULL,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TRIGGER diagnostics_limit AFTER INSERT ON diagnostic_logs BEGIN
 DELETE FROM diagnostic_logs WHERE id <= new.id - 1000;
END;
