-- Your SQL goes here

DROP INDEX IF EXISTS images.image_index;
DROP TABLE IF EXISTS images;

CREATE TABLE IF NOT EXISTS images (
  fileindex INTEGER PRIMARY KEY NOT NULL,
  url TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS image_index ON images (fileindex);
