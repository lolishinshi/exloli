-- Your SQL goes here

ALTER TABLE gallery ADD COLUMN votes TEXT NOT NULL default "[]";
UPDATE gallery SET score = score / 5;
