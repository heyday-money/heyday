CREATE TABLE institutions (
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
 name_key TEXT NOT NULL UNIQUE,
 short_name TEXT,
 bank_code TEXT,
 swift_code TEXT,
 logo TEXT,
 is_archived INTEGER NOT NULL DEFAULT 0 CHECK(is_archived IN (0,1))
);
ALTER TABLE accounts ADD COLUMN institution_id TEXT REFERENCES institutions(id);
CREATE INDEX accounts_institution_id ON accounts(institution_id);

INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-002','Bangkok Bank Public Company Limited','bangkok bank public company limited','BBL','002','BKKBTHBK','bbl.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-004','Kasikornbank Public Company Limited','kasikornbank public company limited','KBANK','004','KASITHBK','kbank.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-006','Krungthai Bank Public Company Limited','krungthai bank public company limited','KTB','006','KRUTTHBK','ktb.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-014','The Siam Commercial Bank Public Company Limited','the siam commercial bank public company limited','SCB','014','SICOTHBK','scb.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-024','United Overseas Bank (Thai) Public Company Limited','united overseas bank (thai) public company limited','UOB','024','UOVBTHBK','uob.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-025','Bank of Ayudhya Public Company Limited','bank of ayudhya public company limited','BAY','025','AYUDTHBK','bay.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-033','Government Housing Bank','government housing bank','GHB','033','GHBATHBK','ghb.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-067','TISCO Bank Public Company Limited','tisco bank public company limited','TISCO','067','TISCTHBK','tisco.svg');
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('bank-069','Kiatnakin Phatra Bank Public Company Limited','kiatnakin phatra bank public company limited','KKP','069','KKPBTHBK','kkp.svg');

-- Normalize old free-text names without dropping accounts or changing amounts.
CREATE TEMP TABLE legacy_institutions AS
WITH RECURSIVE normalized(id,name) AS (
 SELECT id,trim(replace(replace(replace(institution,char(9),' '),char(10),' '),char(13),' '))
 FROM accounts WHERE type <> 'cash' AND institution IS NOT NULL
 UNION ALL
 SELECT id,replace(name,'  ',' ') FROM normalized WHERE instr(name,'  ') > 0
)
SELECT id,name,lower(name) AS name_key FROM normalized WHERE instr(name,'  ')=0 AND name <> '';
UPDATE accounts SET institution_id=(
 SELECT i.id FROM institutions i JOIN legacy_institutions l ON l.id=accounts.id
 WHERE l.name_key=i.name_key OR l.name_key=lower(i.short_name) OR l.name_key=i.bank_code OR l.name_key=lower(i.swift_code)
 LIMIT 1
) WHERE id IN (SELECT id FROM legacy_institutions);
INSERT INTO institutions(id,name,name_key)
 SELECT lower(hex(randomblob(16))),min(l.name),l.name_key FROM legacy_institutions l
 JOIN accounts a ON a.id=l.id WHERE a.institution_id IS NULL GROUP BY l.name_key;
UPDATE accounts SET institution_id=(SELECT i.id FROM institutions i JOIN legacy_institutions l ON l.name_key=i.name_key WHERE l.id=accounts.id)
 WHERE institution_id IS NULL AND id IN (SELECT id FROM legacy_institutions);
UPDATE accounts SET institution=(SELECT name FROM institutions WHERE id=accounts.institution_id) WHERE institution_id IS NOT NULL;
DROP TABLE legacy_institutions;

-- Keep the legacy display column synchronized for existing financial snapshots.
CREATE TRIGGER institution_rename AFTER UPDATE OF name ON institutions BEGIN
 UPDATE accounts SET institution=NEW.name WHERE institution_id=NEW.id;
END;
