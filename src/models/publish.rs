use crate::database::DB;
use chrono::{NaiveDate, NaiveDateTime};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::{Error, Result, SqlitePool};

#[derive(sqlx::FromRow, Debug)]
pub struct Publish {
    /// 消息 ID
    pub id: i32,
    /// 画廊 ID
    pub gallery_id: i32,
    /// telegraph 文章 URL
    pub telegraph: String,
    /// 总共上传图片数量
    pub upload_images: i32,
    /// 文章发布日期
    pub publish_date: NaiveDate,
}

impl Publish {
    pub async fn upsert(&self) -> Result<SqliteQueryResult> {
        sqlx::query(
            "REPLACE INTO publish (id, gallery_id, telegraph, upload_images, publish_date) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&self.id)
        .bind(&self.gallery_id)
        .bind(&self.telegraph)
        .bind(&self.upload_images)
        .bind(&self.publish_date)
        .execute(&*DB)
        .await
    }
}
