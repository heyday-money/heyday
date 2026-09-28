-- Preserve existing IDs, names, archive states and custom logo preferences.
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('wallet-line-pay','LINE Pay','line pay','LINE Pay',NULL,NULL,'line-pay.svg') ON CONFLICT DO NOTHING;
UPDATE institutions SET short_name=COALESCE(short_name,'LINE Pay'),logo=COALESCE(logo,'line-pay.svg') WHERE name_key='line pay';
INSERT INTO institutions(id,name,name_key,short_name,bank_code,swift_code,logo) VALUES('wallet-grab-pay','GrabPay','grabpay','GrabPay',NULL,NULL,'grab-pay.svg') ON CONFLICT DO NOTHING;
UPDATE institutions SET short_name=COALESCE(short_name,'GrabPay'),logo=COALESCE(logo,'grab-pay.svg') WHERE name_key='grabpay';
