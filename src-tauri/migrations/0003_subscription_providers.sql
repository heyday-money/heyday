CREATE TABLE subscription_providers (
 id TEXT PRIMARY KEY NOT NULL,
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
 name_key TEXT NOT NULL UNIQUE,
 is_archived INTEGER NOT NULL DEFAULT 0 CHECK(is_archived IN (0,1)),
 builtin_icon TEXT,
 logo_mode TEXT NOT NULL DEFAULT 'default' CHECK(logo_mode IN ('default','custom','none')),
 logo_asset_id TEXT REFERENCES logo_assets(id),
 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
 updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
ALTER TABLE subscriptions ADD COLUMN provider_id TEXT REFERENCES subscription_providers(id);
CREATE INDEX subscriptions_provider ON subscriptions(provider_id);
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-netflix','Netflix','netflix','netflix');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-spotify','Spotify','spotify','spotify');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-youtube','YouTube Premium','youtube premium','youtube');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-apple-music','Apple Music','apple music','apple-music');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-icloud','iCloud+','icloud+','icloud');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-google-one','Google One','google one','google-one');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-microsoft-365','Microsoft 365','microsoft 365','microsoft-365');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-disney','Disney+','disney+','disney');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-prime-video','Prime Video','prime video','prime-video');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-adobe','Adobe Creative Cloud','adobe creative cloud','adobe');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-dropbox','Dropbox','dropbox','dropbox');
INSERT INTO subscription_providers(id,name,name_key,builtin_icon) VALUES('provider-canva','Canva','canva','canva');
