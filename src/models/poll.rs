use crate::database::DB;
use chrono::{NaiveDate, NaiveDateTime};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::Error;

#[derive(sqlx::FromRow, Debug)]
pub struct Poll {
    /// 投票 ID，通常使用第一次被 Publish 时的 ID
    pub id: String,
    /// 每个选项的投票数量，为 JSON 格式的数组，长度为 5
    pub votes: String,
    /// 当前投票的分数，为 0~1 的小数
    pub score: f32,
}

#[derive(sqlx::FromRow, Debug)]
pub struct Vote {
    /// 用户 ID
    pub user_id: i32,
    /// 投票 ID
    pub poll_id: i32,
    /// 投票选项
    pub option: i32,
    /// 投票时间
    pub vote_time: NaiveDateTime,
}

impl Poll {
    pub async fn upsert(&self) -> sqlx::Result<SqliteQueryResult> {
        sqlx::query("REPLACE INTO poll (id, votes, score) VALUES (?, ?, ?)")
            .bind(&self.id)
            .bind(&self.votes)
            .bind(&self.score)
            .execute(&*DB)
            .await
    }

    pub async fn fetch_by_id(id: String) -> sqlx::Result<Option<Self>> {
        sqlx::query_as("SELECT * FROM poll WHERE id = ?")
            .bind(id)
            .fetch_optional(&*DB)
            .await
    }
}

impl Vote {
    pub async fn upsert(&self) -> sqlx::Result<SqliteQueryResult> {
        sqlx::query("REPLACE INTO vote (user_id, poll_id, option, vote_time) VALUES (?, ?, ?, ?)")
            .bind(&self.user_id)
            .bind(&self.poll_id)
            .bind(&self.option)
            .bind(&self.vote_time)
            .execute(&*DB)
            .await
    }
}
