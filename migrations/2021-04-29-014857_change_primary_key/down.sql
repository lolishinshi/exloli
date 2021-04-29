-- This file should undo anything in `up.sql`

CREATE TABLE IF NOT EXISTS gallery_tmp (
    gallery_id INTEGER PRIMARY KEY NOT NULL,
    token TEXT NOT NULL,
    title TEXT NOT NULL,
    tags TEXT NOT NULL,
    telegraph TEXT NOT NULL,
    upload_images INT2 NOT NULL,
    publish_date DATE NOT NULL,
    message_id INTEGER NOT NULL,
    poll_id TEXT NOT NULL,
    score FLOAT NOT NULL,
    votes TEXT NOT NULL
);

INSERT INTO
    gallery_tmp
SELECT
    gallery_id, token, title, tags, telegraph, upload_images, publish_date, message_id, poll_id, score, votes
FROM gallery;

DROP TABLE gallery;
ALTER TABLE gallery_tmp RENAME TO gallery;

CREATE UNIQUE INDEX IF NOT EXISTS gallery_id_index ON gallery (gallery_id);
CREATE INDEX IF NOT EXISTS poll_id_index ON gallery (poll_id);
