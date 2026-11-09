-- Device-local bookkeeping. Backup export/restore clears these tables.
CREATE TABLE cloud_sync_binding (
 id INTEGER PRIMARY KEY CHECK(id=1),
 origin TEXT NOT NULL,
 account_id TEXT NOT NULL,
 dataset_id TEXT NOT NULL,
 key_id TEXT NOT NULL,
 enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
 bootstrapped INTEGER NOT NULL DEFAULT 0 CHECK(bootstrapped IN (0,1)),
 applying INTEGER NOT NULL DEFAULT 0 CHECK(applying IN (0,1)),
 sequence INTEGER NOT NULL DEFAULT 0 CHECK(sequence>=0),
 hash TEXT NOT NULL,
 cursor TEXT,
 last_success TEXT
);
CREATE TABLE cloud_sync_dirty (
 record_id TEXT PRIMARY KEY NOT NULL,
 revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0)
);
CREATE TABLE cloud_sync_shadow (
 record_id TEXT PRIMARY KEY NOT NULL,
 version INTEGER NOT NULL CHECK(version>0),
 payload TEXT NOT NULL,
 conflict INTEGER NOT NULL DEFAULT 0 CHECK(conflict IN (0,1))
);
CREATE TABLE cloud_sync_pending (
 id INTEGER PRIMARY KEY CHECK(id=1),
 record_id TEXT NOT NULL,
 revision INTEGER NOT NULL,
 request TEXT NOT NULL
);
CREATE TRIGGER cloud_sync_category_insert AFTER INSERT ON categories
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES(NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER cloud_sync_category_update AFTER UPDATE OF name,icon,is_archived ON categories
WHEN (OLD.name<>NEW.name OR OLD.icon<>NEW.icon OR OLD.is_archived<>NEW.is_archived)
 AND EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES(NEW.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER cloud_sync_category_delete AFTER DELETE ON categories
WHEN EXISTS(SELECT 1 FROM cloud_sync_binding WHERE applying=0)
BEGIN
 INSERT INTO cloud_sync_dirty(record_id) VALUES(OLD.id)
 ON CONFLICT(record_id) DO UPDATE SET revision=revision+1;
END;
