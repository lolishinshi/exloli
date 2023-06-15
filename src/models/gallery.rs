use crate::database::DB;
use chrono::{NaiveDate, NaiveDateTime};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::Error;

#[derive(sqlx::FromRow, Debug)]
pub struct Gallery {
    /// 画廊 ID
    pub id: i32,
    /// 画廊 token
    pub token: String,
    /// 画廊标题
    pub title: String,
    /// JSON 格式的画廊标签
    pub tags: String,
    /// 父画廊
    pub parent: Option<i32>,
}

impl Gallery {
    pub async fn upsert(&self) -> Result<SqliteQueryResult, Error> {
        sqlx::query("REPLACE INTO gallery (id, token, title, tags, parent) VALUES (?, ?, ?, ?, ?)")
            .bind(&self.id)
            .bind(&self.token)
            .bind(&self.title)
            .bind(&self.tags)
            .bind(&self.parent)
            .execute(&*DB)
            .await
    }

    pub async fn fetch_by_id(id: i32) -> Result<Option<Self>, Error> {
        sqlx::query_as("SELECT * FROM gallery WHERE id = ?")
            .bind(id)
            .fetch_optional(&*DB)
            .await
    }
}
