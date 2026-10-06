CREATE TABLE logo_assets (
 id TEXT PRIMARY KEY,
 content_hash TEXT NOT NULL UNIQUE,
 mime_type TEXT NOT NULL CHECK(mime_type='image/png'),
 width INTEGER NOT NULL CHECK(width BETWEEN 1 AND 128),
 height INTEGER NOT NULL CHECK(height BETWEEN 1 AND 128),
 data BLOB NOT NULL CHECK(length(data) BETWEEN 1 AND 102400),
 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
ALTER TABLE institutions ADD COLUMN logo_asset_id TEXT REFERENCES logo_assets(id);
ALTER TABLE institutions ADD COLUMN logo_mode TEXT NOT NULL DEFAULT 'default'
 CHECK(logo_mode IN ('default','custom','none') AND ((logo_mode='custom')=(logo_asset_id IS NOT NULL)));
ALTER TABLE payees ADD COLUMN logo_asset_id TEXT REFERENCES logo_assets(id);
