ALTER TABLE user_vote RENAME to _user_vote;
CREATE TABLE user_vote
(
    user_id   BIGINT  NOT NULL,
    poll_id   INTEGER NOT NULL,
    option    INTEGER NOT NULL,
    PRIMARY KEY (user_id, poll_id)
);
CREATE INDEX IF NOT EXISTS user_vote_index ON user_vote (user_id, poll_id);
CREATE INDEX IF NOT EXISTS poll_index ON user_vote (poll_id);

INSERT INTO user_vote (user_id, poll_id, option)
SELECT user_id, poll_id, option FROM _user_vote;

DROP TABLE _user_vote;