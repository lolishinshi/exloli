-- Add down migration script here
DROP TABLE IF EXISTS gallery;
DROP TABLE IF EXISTS image_hash;
DROP TABLE IF EXISTS user_vote;
DROP INDEX IF EXISTS gallery_id_index;
DROP INDEX IF EXISTS poll_id_index;
DROP INDEX IF EXISTS poll_index;
