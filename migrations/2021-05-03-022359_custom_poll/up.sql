CREATE TABLE IF NOT EXISTS user_vote
(
    user_id   BIGINT  NOT NULL,
    poll_id   INTEGER NOT NULL,
    option    INTEGER NOT NULL,
    PRIMARY KEY (user_id, poll_id)
);

CREATE INDEX IF NOT EXISTS user_vote_index ON user_vote (user_id, poll_id);
CREATE INDEX IF NOT EXISTS poll_index ON user_vote (poll_id);
