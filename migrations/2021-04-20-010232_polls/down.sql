-- This file should undo anything in `up.sql`

CREATE TABLE gallery_tmp (
     gallery_id INTEGER PRIMARY KEY NOT NULL,
     token TEXT NOT NULL,
     title TEXT NOT NULL,
     tags TEXT NOT NULL,
     telegraph TEXT NOT NULL,
     upload_images INT2 NOT NULL,
     publish_date DATE NOT NULL,
     message_id INTEGER NOT NULL,
     poll_id TEXT NOT NULL,
     score FLOAT NOT NULL
);

INSERT INTO gallery_tmp SELECT
   gallery_id, token, title, tags, telegraph, upload_images, publish_date, message_id, poll_id, score
FROM gallery;

DROP TABLE gallery;

ALTER TABLE gallery_tmp RENAME TO gallery;

UPDATE gallery SET score = score * 5;
