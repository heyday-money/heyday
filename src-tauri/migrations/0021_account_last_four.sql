-- Replace only the last-four constraint; keep accounts and references intact.
ALTER TABLE accounts ADD COLUMN last_four_new TEXT CHECK (
    last_four_new IS NULL OR (length(last_four_new) = 4 AND last_four_new NOT GLOB '*[^0-9]*')
);
UPDATE accounts SET last_four_new = last_four;
ALTER TABLE accounts DROP COLUMN last_four;
ALTER TABLE accounts RENAME COLUMN last_four_new TO last_four;
