use crate::database::DB;
use chrono::{NaiveDate, NaiveDateTime};
use sqlx::sqlite::SqliteQueryResult;
use sqlx::Error;

#[derive(sqlx::FromRow, Debug)]
pub struct Image {
    /// 画廊 ID
    pub gallery_id: i32,
    /// 页面编号
    pub page: i32,
    /// 图片 hash
    pub hash: String,
    /// 相对 https://telegra.ph 的图片 URL
    pub url: String,
}

impl Image {
    pub async fn upsert(&self) -> sqlx::Result<SqliteQueryResult> {
        sqlx::query("REPLACE INTO image (gallery_id, page, hash, url) VALUES (?, ?, ?, ?)")
            .bind(&self.gallery_id)
            .bind(&self.page)
            .bind(&self.hash)
            .bind(&self.url)
            .execute(&*DB)
            .await
    }

    pub async fn fetch_by_hash(hash: &str) -> sqlx::Result<Option<Image>> {
        sqlx::query_as::<_, Image>("SELECT * FROM image WHERE hash = ?")
            .bind(hash)
            .fetch_optional(&*DB)
            .await
    }

    pub async fn fetch_random_by_gallery_id(gallery_id: i32) -> sqlx::Result<Option<Image>> {
        sqlx::query_as::<_, Image>(
            "SELECT * FROM image WHERE gallery_id = ? ORDER BY RANDOM() LIMIT 1",
        )
        .bind(gallery_id)
        .fetch_optional(&*DB)
        .await
    }
}
