-- Presentation and management metadata only; existing schedules remain unchanged.
ALTER TABLE subscriptions ADD COLUMN logo_asset_id TEXT REFERENCES logo_assets(id);
ALTER TABLE subscriptions ADD COLUMN managed_via TEXT
 CHECK(managed_via IS NULL OR managed_via IN ('apple_app_store','google_play','website','in_app','other'));
ALTER TABLE subscriptions ADD COLUMN management_url TEXT
 CHECK(management_url IS NULL OR length(management_url) BETWEEN 1 AND 2048);
