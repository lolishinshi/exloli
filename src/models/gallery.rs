use chrono::{NaiveDate, NaiveDateTime};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Error, SqlitePool};

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
    pub async fn upsert(&self, conn: &SqlitePool) -> Result<SqliteQueryResult, Error> {
        sqlx::query(
            "INSERT INTO gallery (id, token, title, tags) VALUES (?, ?, ?, ?) ON CONFLICT (id) DO UPDATE SET token = ?, title = ?, tags = ?",
        )
        .bind(&self.id)
        .bind(&self.token)
        .bind(&self.title)
        .bind(&self.tags)
        .execute(conn)
        .await
    }
}
