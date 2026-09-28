CREATE TABLE card_limit_groups (
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
 name_key TEXT NOT NULL UNIQUE,
 credit_limit INTEGER NOT NULL CHECK(credit_limit >= 0)
);
CREATE TABLE card_limit_members (
 account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE RESTRICT,
 group_id TEXT NOT NULL REFERENCES card_limit_groups(id) ON DELETE CASCADE
);
CREATE INDEX card_limit_members_group ON card_limit_members(group_id);
CREATE TRIGGER card_limit_member_type BEFORE INSERT ON card_limit_members
WHEN NOT EXISTS(SELECT 1 FROM accounts WHERE id=NEW.account_id AND type='credit_card')
BEGIN SELECT RAISE(ABORT,'Shared limits require credit card accounts'); END;
