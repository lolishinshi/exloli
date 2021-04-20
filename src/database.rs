use crate::exhentai::*;
use crate::schema::*;
use crate::utils::{get_id_from_gallery, get_id_from_image};
use anyhow::Result;
use chrono::prelude::*;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::Sqlite;
use std::env;

embed_migrations!("migrations");

#[derive(Queryable, Insertable, PartialEq, Debug)]
#[table_name = "gallery"]
pub struct Gallery {
    pub gallery_id: i32,
    pub token: String,
    pub title: String,
    pub tags: String,
    pub telegraph: String,
    pub upload_images: i16,
    pub publish_date: NaiveDate,
    pub message_id: i32,
    pub poll_id: String,
    pub score: f32,
    pub votes: String,
}

#[derive(Queryable, Insertable)]
#[table_name = "images"]
pub struct Image {
    pub gallery_id: i32,
    pub number: i32,
    pub url: String,
}

pub struct DataBase {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl DataBase {
    pub fn init() -> Self {
        info!("数据库建立连接中……");
        let url = env::var("DATABASE_URL").expect("请设置 DATABASE_URL");
        let manager = ConnectionManager::new(url);
        let pool = Pool::builder()
            .max_size(16)
            .build(manager)
            .expect("连接池建立失败");
        Self { pool }
    }

    pub fn init_database(&self) -> Result<()> {
        embedded_migrations::run(&self.pool.get()?)?;
        embedded_migrations::run_with_output(&self.pool.get()?, &mut std::io::stdout())?;
        Ok(())
    }

    pub fn insert_image(&self, image_url: &str, uploaded_url: &str) -> Result<()> {
        let (id, number) = get_id_from_image(image_url);
        let img = Image {
            gallery_id: id,
            number,
            url: uploaded_url.to_owned(),
        };
        diesel::insert_or_ignore_into(images::table)
            .values(&img)
            .execute(&self.pool.get()?)?;
        Ok(())
    }

    pub fn query_image_by_url(&self, image_url: &str) -> Result<Image> {
        let (id, number) = get_id_from_image(image_url);
        Ok(images::table
            .filter(images::gallery_id.eq(id).and(images::number.eq(number)))
            .get_result::<Image>(&self.pool.get()?)?)
    }

    pub fn insert_gallery(
        &self,
        info: &FullGalleryInfo,
        telegraph: String,
        message_id: i32,
    ) -> Result<()> {
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
            .execute(&self.pool.get()?)?;
        Ok(())
    }

    // TODO: 根据 grep.app 上的代码优化一下自己的代码
    /// 更新旧画廊信息
    pub fn update_gallery(
        &self,
        old_gallery: &Gallery,
        info: &FullGalleryInfo,
        telegraph: &str,
        message_id: i32,
    ) -> Result<()> {
        debug!("更新画廊数据");
        let (gallery_id, token) = get_id_from_gallery(&info.url);
        diesel::update(gallery::table)
            .filter(gallery::gallery_id.eq(old_gallery.gallery_id))
            .set((
                gallery::gallery_id.eq(gallery_id),
                gallery::title.eq(&info.title),
                gallery::token.eq(token),
                gallery::message_id.eq(message_id),
                gallery::telegraph.eq(telegraph),
                gallery::tags.eq(serde_json::to_string(&info.tags)?),
                gallery::upload_images.eq(info.get_image_lists().len() as i16),
            ))
            .execute(&self.pool.get()?)?;
        // 如果这次更新发布了新消息，那么需要同时更改发布日期
        if old_gallery.message_id != message_id {
            diesel::update(gallery::table)
                .filter(gallery::gallery_id.eq(old_gallery.gallery_id))
                .set(gallery::publish_date.eq(Utc::today().naive_utc()))
                .execute(&self.pool.get()?)?;
        }
        Ok(())
    }

    /// 根据消息 id 删除画廊，并不会实际删除，否则又会在定时更新时被上传
    pub fn delete_gallery_by_message_id(&self, message_id: i32) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::message_id.eq(message_id))
            .set(gallery::score.eq(-1.0))
            .execute(&self.pool.get()?)?;
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
        let ordering: Box<dyn BoxableExpression<gallery::table, Sqlite, SqlType = ()>> =
            if offset > 0 {
                Box::new(gallery::score.desc())
            } else {
                offset = -offset;
                Box::new(gallery::score.asc())
            };
        Ok(gallery::table
            .filter(
                gallery::publish_date
                    .ge(to)
                    .and(gallery::publish_date.le(from))
                    .and(gallery::score.gt(0.0)),
            )
            .order_by(ordering)
            .offset(offset - 1)
            .limit(20)
            .load::<Gallery>(&self.pool.get()?)?)
    }

    pub fn update_poll_id(&self, message_id: i32, poll_id: &str) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::message_id.eq(message_id))
            .set(gallery::poll_id.eq(poll_id))
            .execute(&self.pool.get()?)?;
        Ok(())
    }

    pub fn update_score(&self, poll_id: &str, score: f32, votes: &str) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::poll_id.eq(poll_id))
            .set((gallery::score.eq(score), gallery::votes.eq(votes)))
            .execute(&self.pool.get()?)?;
        Ok(())
    }

    pub fn query_gallery_by_url(&self, url: &str) -> Result<Gallery> {
        let (id, _) = get_id_from_gallery(url);
        Ok(gallery::table
            .filter(gallery::gallery_id.eq(id))
            .get_result::<Gallery>(&self.pool.get()?)?)
    }

    /// 查询最近一次发布的符合标题的画廊
    pub fn query_gallery_by_title(&self, title: &str) -> Result<Gallery> {
        Ok(gallery::table
            .filter(gallery::title.eq(title))
            .order_by(gallery::gallery_id.desc())
            .limit(1)
            .get_result::<Gallery>(&self.pool.get()?)?)
    }

    pub fn query_gallery_by_message_id(&self, message_id: i32) -> Result<Gallery> {
        Ok(gallery::table
            .filter(gallery::message_id.eq(message_id))
            .get_result::<Gallery>(&self.pool.get()?)?)
    }
}

impl Gallery {
    pub fn get_url(&self) -> String {
        format!("https://exhentai.org/g/{}/{}/", self.gallery_id, self.token)
    }
}
