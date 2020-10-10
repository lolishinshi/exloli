use crate::exhentai::*;
use crate::schema::*;
use crate::utils::{get_id_from_gallery, get_id_from_image};
use anyhow::Result;
use chrono::prelude::*;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::result::Error as DieselError;
use std::env;

#[derive(Queryable, Insertable)]
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
}

#[derive(Queryable, Insertable)]
#[table_name = "images"]
pub struct Image {
    pub gallery_id: i32,
    pub number: i32,
    pub url: String,
}

#[derive(Queryable, Insertable)]
#[table_name = "users"]
pub struct User {
    pub user_id: i32,
    pub warn: i16,
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
            .max_size(8)
            .build(manager)
            .expect("连接池建立失败");
        Self { pool }
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

    pub fn reset_image_by_url(&self) {
        todo!()
    }

    pub fn insert_gallery<S: Into<String>>(
        &self,
        info: &FullGalleryInfo,
        telegraph: S,
        message_id: i32,
    ) -> Result<()> {
        let (id, token) = get_id_from_gallery(&info.url);
        let gallery = Gallery {
            gallery_id: id,
            token: token.to_owned(),
            title: info.title.to_owned(),
            tags: serde_json::to_string(&info.tags)?,
            publish_date: Utc::today().naive_utc(),
            score: 0.0,
            message_id,
            upload_images: info.get_image_lists().len() as i16,
            poll_id: "".to_string(),
            telegraph: telegraph.into(),
        };
        diesel::insert_or_ignore_into(gallery::table)
            .values(&gallery)
            .execute(&self.pool.get()?)?;
        Ok(())
    }

    pub fn update_poll_id(&self, message_id: i32, poll_id: &str) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::message_id.eq(message_id))
            .set(gallery::poll_id.eq(poll_id))
            .execute(&self.pool.get()?)?;
        Ok(())
    }

    pub fn update_score(&self, poll_id: &str, score: f32) -> Result<()> {
        diesel::update(gallery::table)
            .filter(gallery::poll_id.eq(poll_id))
            .set(gallery::score.eq(score))
            .execute(&self.pool.get()?)?;
        Ok(())
    }

    pub fn query_gallery_by_url(&self, url: &str) -> Result<Gallery> {
        let (id, _) = get_id_from_gallery(url);
        Ok(gallery::table
            .filter(gallery::gallery_id.eq(id))
            .get_result::<Gallery>(&self.pool.get()?)?)
    }

    pub fn query_gallery_by_title(&self, title: &str) -> Result<Gallery> {
        Ok(gallery::table
            .filter(gallery::title.eq(title))
            .get_result::<Gallery>(&self.pool.get()?)?)
    }

    pub fn add_warn(&self, user_id: i32) -> Result<i16> {
        let warn = users::table
            .select(users::warn)
            .filter(users::user_id.eq(user_id))
            .first::<i16>(&self.pool.get()?);
        match warn {
            Err(DieselError::NotFound) => {
                let t = User { user_id, warn: 1 };
                diesel::insert_into(users::table)
                    .values(&t)
                    .execute(&self.pool.get()?)?;
                Ok(1)
            }
            Ok(v) => {
                diesel::update(users::table)
                    .filter(users::user_id.eq(user_id))
                    .set(users::warn.eq(v + 1))
                    .execute(&self.pool.get()?)?;
                Ok(v + 1)
            }
            Err(e) => Err(e)?,
        }
    }
}
