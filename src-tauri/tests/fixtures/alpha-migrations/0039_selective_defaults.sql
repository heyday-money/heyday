CREATE TABLE selective_defaults (
    account_id TEXT NOT NULL REFERENCES accounts(id),
    start_month TEXT NOT NULL CHECK(length(start_month)=7 AND start_month GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]' AND substr(start_month,1,4)>='0001' AND substr(start_month,6,2) BETWEEN '01' AND '12'),
    end_month TEXT CHECK(end_month IS NULL OR (length(end_month)=7 AND end_month GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]' AND substr(end_month,1,4)>='0001' AND substr(end_month,6,2) BETWEEN '01' AND '12' AND end_month>=start_month)),
    PRIMARY KEY(account_id,start_month)
);
CREATE UNIQUE INDEX one_open_selective_default ON selective_defaults(account_id) WHERE end_month IS NULL;
