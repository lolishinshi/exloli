use chrono::{NaiveDate, NaiveDateTime};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Error, SqlitePool};

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
