CREATE INDEX IF NOT EXISTS user_vote_index ON user_vote (user_id, poll_id);
CREATE UNIQUE INDEX IF NOT EXISTS image_index ON images (fileindex);
