-- Broaden review tracking without modifying the already-applied billing migration.
DROP TRIGGER card_statement_transaction_deleted;
DROP TRIGGER card_statement_backdated_transaction;
CREATE TRIGGER card_statement_transaction_deleted BEFORE DELETE ON transactions BEGIN
 UPDATE card_statements SET needs_review=1 WHERE (account_id=OLD.account_id OR account_id=OLD.destination_account_id) AND end_date>=OLD.date;
END;
CREATE TRIGGER card_statement_backdated_transaction AFTER INSERT ON transactions BEGIN
 UPDATE card_statements SET needs_review=1 WHERE (account_id=NEW.account_id OR account_id=NEW.destination_account_id)
 AND end_date>=NEW.date;
END;

CREATE TRIGGER card_statement_installment_edited AFTER UPDATE ON installments BEGIN
 UPDATE card_statements SET needs_review=1 WHERE id IN (SELECT statement_id FROM card_statement_installments WHERE installment_id=OLD.id);
END;
CREATE TRIGGER card_statement_installment_removed BEFORE DELETE ON installments BEGIN
 UPDATE card_statements SET needs_review=1 WHERE id IN (SELECT statement_id FROM card_statement_installments WHERE installment_id=OLD.id);
END;
