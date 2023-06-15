-- Add up migration script here
CREATE TABLE IF NOT EXISTS "gallery" (
    message_id INTEGER PRIMARY KEY NOT NULL,
    gallery_id INTEGER NOT NULL,
    token TEXT NOT NULL,
    title TEXT NOT NULL,
    tags TEXT NOT NULL,
    telegraph TEXT NOT NULL,
    upload_images INT2 NOT NULL,
    publish_date DATE NOT NULL,
    poll_id TEXT NOT NULL,
    score FLOAT NOT NULL,
    votes TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS image_hash (
    hash TEXT NOT NULL PRIMARY KEY,
    url TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS user_vote (
    user_id   BIGINT  NOT NULL,
    poll_id   INTEGER NOT NULL,
    option    INTEGER NOT NULL, vote_time DATETIME NOT NULL DEFAULT "2021-05-14 00:00:00.000000000",
    PRIMARY KEY (user_id, poll_id)
);
CREATE INDEX IF NOT EXISTS gallery_id_index ON gallery (gallery_id);
CREATE INDEX IF NOT EXISTS poll_id_index ON gallery (poll_id);
CREATE INDEX IF NOT EXISTS poll_index ON user_vote (poll_id);
