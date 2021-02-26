-- Your SQL goes here

CREATE INDEX IF NOT EXISTS title_index ON gallery (title);
CREATE UNIQUE INDEX IF NOT EXISTS gallery_id_index ON gallery (gallery_id);
CREATE INDEX IF NOT EXISTS poll_id_index ON gallery (poll_id);
CREATE UNIQUE INDEX IF NOT EXISTS image_index ON images (gallery_id, number);
