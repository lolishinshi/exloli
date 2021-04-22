-- This file should undo anything in `up.sql`

DROP INDEX IF EXISTS images.image_index;
DROP TABLE IF EXISTS images;

CREATE TABLE IF NOT EXISTS images (
  gallery_id INTEGER NOT NULL,
  number INTEGER NOT NULL,
  url TEXT NOT NULL,
  PRIMARY KEY (gallery_id, number)
);
CREATE UNIQUE INDEX IF NOT EXISTS image_index ON images (gallery_id, number);
