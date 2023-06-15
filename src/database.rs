use crate::exhentai::*;
use crate::schema::*;
use crate::utils::*;
use anyhow::{Context, Result};
use chrono::prelude::*;
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::Float;
use diesel::sqlite::Sqlite;
use futures::executor::block_on;
use once_cell::sync::Lazy;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::env;

//embed_migrations!("migrations");

#[derive(Queryable, Insertable, PartialEq, Debug, Clone)]
#[table_name = "gallery"]
pub struct Gallery {
    pub message_id: i32,
    pub gallery_id: i32,
    pub token: String,
    pub title: String,
    pub tags: String,
    pub telegraph: String,
    pub upload_images: i16,
    pub publish_date: NaiveDate,
    pub poll_id: String,
    pub score: f32,
    pub votes: String,
}

#[derive(Queryable, Insertable)]
#[table_name = "images"]
pub struct Image {
    pub fileindex: i32,
    pub url: String,
}

#[derive(Queryable, Insertable)]
#[table_name = "image_hash"]
pub struct ImageHash {
    pub hash: String,
    pub url: String,
}

pub struct DataBase {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl DataBase {
    pub fn init() -> Result<Self> {
        info!("数据库初始化中……");
        let url = env::var("DATABASE_URL").expect("请设置 DATABASE_URL");
        let manager = ConnectionManager::new(url);
        let pool = Pool::builder()
            .max_size(16)
            .build(manager)
            .expect("连接池建立失败");
        //embedded_migrations::run_with_output(&pool.get()?, &mut std::io::stdout())?;
        Ok(Self { pool })
    }

    pub fn insert_image(&self, image_url: &str, uploaded_url: &str) -> Result<()> {
        let hash = get_hash_from_image(image_url).context("图片哈希提取失败")?;
        let img = ImageHash {
            hash: hash.to_owned(),
            url: uploaded_url.to_owned(),
        };
        diesel::insert_or_ignore_into(image_hash::table)
            .values(&img)
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    pub fn query_image_by_hash(&self, image_url: &str) -> Result<String> {
        let hash = get_hash_from_image(image_url).context("无法提取图片 hash")?;
        Ok(image_hash::table
            .filter(image_hash::hash.eq(hash))
            .get_result::<ImageHash>(&mut self.pool.get()?)?
            .url)
    }

    pub fn query_image_by_fileindex(&self, image_url: &str) -> Result<String> {
        let fileindex = get_id_from_image(image_url).context("无法提取图片 fileindex")?;
        Ok(images::table
            .filter(images::fileindex.eq(fileindex))
            .get_result::<Image>(&mut self.pool.get()?)?
            .url)
    }

    pub fn insert_gallery(
        &self,
        message_id: i32,
        info: &FullGalleryInfo,
        telegraph: String,
    ) -> Result<()> {
        debug!("添加新画廊");
        let (gallery_id, token) = get_id_from_gallery(&info.url);
        let gallery = Gallery {
            title: info.title.to_owned(),
            tags: serde_json::to_string(&info.tags)?,
            publish_date: Utc::today().naive_utc(),
            score: 0.0,
            votes: "[]".to_string(),
            upload_images: info.get_image_lists().len() as i16,
            poll_id: "".to_string(),
            telegraph,
            gallery_id,
            token,
            message_id,
        };
        diesel::insert_into(gallery::table)
            .values(&gallery)
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    // TODO: 根据 grep.app 上的代码优化一下自己的代码
    /// 更新旧画廊信息
    pub fn update_gallery(
        &self,
        message_id: i32,
        info: &FullGalleryInfo,
        telegraph: &str,
        upload_images: usize,
    ) -> Result<()> {
        debug!("更新画廊数据");
        let (gallery_id, token) = get_id_from_gallery(&info.url);
        diesel::update(gallery::table)
            .filter(gallery::message_id.eq(message_id))
            .set((
                gallery::gallery_id.eq(gallery_id),
                gallery::title.eq(&info.title),
                gallery::token.eq(token),
                gallery::telegraph.eq(telegraph),
                gallery::tags.eq(serde_json::to_string(&info.tags)?),
                gallery::upload_images.eq(upload_images as i16),
            ))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    /// 根据消息 id 删除画廊，并不会实际删除，否则又会在定时更新时被上传
    pub fn delete_gallery(&self, message_id: i32) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::message_id.eq(message_id))
            .set(gallery::score.eq(-1.0))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    /// 根据消息 id 删除画廊，这是真的删除
    pub fn real_delete_gallery(&self, message_id: i32) -> Result<()> {
        diesel::delete(gallery::table)
            .filter(gallery::message_id.eq(message_id))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    /// 查询自指定日期以来分数大于指定分数的 20 本本子
    /// offset 为 1 表示正序，-1 表示逆序
    pub fn query_best(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        mut offset: i64,
    ) -> Result<Vec<Gallery>> {
        todo!();
        // let ordering: Box<dyn BoxableExpression<gallery::table, Sqlite, SqlType = ()>> =
        //     if offset > 0 {
        //         Box::new(gallery::score.desc())
        //     } else {
        //         offset = -offset;
        //         Box::new(gallery::score.asc())
        //     };
        // Ok(gallery::table
        //     .filter(
        //         gallery::publish_date
        //             .ge(to)
        //             .and(gallery::publish_date.le(from))
        //             .and(gallery::score.ne(-1.0))
        //             .and(gallery::poll_id.ne("")),
        //     )
        //     .order_by((ordering, gallery::publish_date.desc()))
        //     .group_by(gallery::poll_id)
        //     .offset(offset - 1)
        //     .limit(20)
        //     .load::<Gallery>(&mut self.pool.get()?)?)
    }

    pub fn get_rank(&self, score: f32) -> Result<f32> {
        Ok(gallery::table
            .filter(gallery::poll_id.ne("").and(gallery::score.ge(0.0)))
            .select(sql::<Float>(&format!(
                "sum(IIF(score >= {}, 1., 0.)) / count(*)",
                score
            )))
            .get_result::<f32>(&mut self.pool.get()?)?)
    }

    pub fn update_poll_id(&self, message_id: i32, poll_id: &str) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::message_id.eq(message_id))
            .set(gallery::poll_id.eq(poll_id))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    pub fn query_poll_id(&self, message_id: i32) -> Result<String> {
        Ok(gallery::table
            .filter(gallery::message_id.eq(message_id))
            .select(gallery::poll_id)
            .get_result::<String>(&mut self.pool.get()?)?)
    }

    pub fn insert_vote(&self, user_id: u64, poll_id: i32, option: i32) -> Result<()> {
        diesel::replace_into(user_vote::table)
            .values(&vec![(
                user_vote::user_id.eq(user_id as i64),
                user_vote::poll_id.eq(poll_id),
                user_vote::option.eq(option),
                user_vote::vote_time.eq(Utc::now().naive_utc()),
            )])
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    pub fn query_vote(&self, poll_id: i32) -> Result<[i32; 5]> {
        let mut ret = [0; 5];
        let options = user_vote::table
            .select(user_vote::option)
            .filter(user_vote::poll_id.eq(poll_id))
            .load::<i32>(&mut self.pool.get()?)?;
        for i in options {
            ret[i as usize - 1] += 1
        }
        Ok(ret)
    }

    pub fn update_score<S: AsRef<str>>(&self, poll_id: &str, score: f32, votes: S) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::poll_id.eq(poll_id))
            .set((gallery::score.eq(score), gallery::votes.eq(votes.as_ref())))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    pub fn query_gallery_by_url(&self, url: &str) -> Result<Gallery> {
        let (id, _) = get_id_from_gallery(url);
        Ok(gallery::table
            .filter(gallery::gallery_id.eq(id))
            .order_by(gallery::publish_date.desc())
            .limit(1)
            .get_result::<Gallery>(&mut self.pool.get()?)?)
    }

    pub fn query_gallery(&self, message_id: i32) -> Result<Gallery> {
        Ok(gallery::table
            .filter(gallery::message_id.eq(message_id))
            .get_result::<Gallery>(&mut self.pool.get()?)?)
    }
}

impl Gallery {
    pub fn get_url(&self) -> String {
        format!("https://{}/g/{}/{}/", *HOST, self.gallery_id, self.token)
    }
}

pub async fn get_connection_pool(url: &str) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(false)
        .filename(url)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("数据库连接失败");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("数据库迁移失败");
    pool
}

pub static DB: Lazy<SqlitePool> = Lazy::new(|| {
    let url = env::var("DATABASE_URL").expect("数据库连接字符串未设置");
    block_on(get_connection_pool(&url))
});
