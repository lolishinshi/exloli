#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;

use crate::{
    config::Config,
    exhentai::{ExHentai, Gallery},
    telegram::Bot,
    telegraph::{publish_article, upload_by_url},
};
use chrono::{prelude::*, Duration};
use failure::Error;
use rayon::prelude::*;
use std::{
    fs,
    io::{Read, Write},
    path,
    sync::{
        atomic::{AtomicU32, Ordering::SeqCst},
        Arc,
    },
};

mod config;
mod exhentai;
mod telegram;
mod telegraph;
mod xpath;

fn run() -> Result<(), Error> {
    let config = Config::new("config.toml")?;
    info!("登录中...");
    let bot = Bot::new(&config.telegram.token);
    let exhentai = ExHentai::new(&config.exhentai.username, &config.exhentai.password)?;

    let mut page = -1;
    let galleries = std::iter::from_fn(|| {
        page += 1;
        exhentai.search(&config.exhentai.keyword, page).ok()
    });

    let last_time = if path::Path::new("./LAST_TIME").exists() {
        let mut s = String::new();
        fs::File::open("./LAST_TIME")?.read_to_string(&mut s)?;
        s.parse::<DateTime<Local>>()?
    } else {
        // 默认从一天前开始
        Local::now() - Duration::days(1)
    };

    let galleries = galleries
        .flatten()
        .into_iter()
        // FIXME: 由于时间只精确到分钟, 此处存在极小的忽略掉本子的可能性
        .take_while(|gallery| gallery.post_time > last_time)
        .collect::<Vec<Gallery>>();

    for gallery in galleries.into_iter().rev() {
        info!("画廊名称: {}", gallery.title);
        info!("画廊地址: {}", gallery.url);

        let mut gallery = gallery;
        let (rating, fav_cnt, img_pages) = exhentai.get_gallery(&gallery.url)?;
        gallery.rating.push_str(&rating);
        gallery.fav_cnt.push_str(&fav_cnt);

        // 多线程爬取图片并上传至 telegraph
        let i = Arc::new(AtomicU32::new(0));
        let img_urls = img_pages
            .par_iter()
            .map(|url| {
                let now = i.load(SeqCst);
                info!("第 {} / {} 张图片", now + 1, img_pages.len());
                i.store(now + 1, SeqCst);
                exhentai
                    .get_image_url(url)
                    .and_then(|img_url| upload_by_url(&img_url))
                    .map(|result| result[0].src.to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        gallery.img_urls.extend(img_urls);

        let content = gallery
            .img_urls
            .iter()
            .map(|s| format!(r#"{{ "tag":"img", "attrs":{{ "src": "{}" }} }}"#, s))
            .collect::<Vec<_>>()
            .join(",");
        info!("发布文章");
        let article_url = publish_article(
            &config.telegraph.access_token,
            &gallery.title,
            &config.telegraph.author_name,
            &config.telegraph.author_url,
            &format!("[{}]", content),
        )?;
        info!("文章地址: {}", article_url);
        bot.send_message(
            &config.telegram.channel_id,
            &format!(
                "评分: {}\n收藏数: {}\n地址: <code>{}</code>\n<a href=\"{}\">{}</a>",
                gallery.rating, gallery.fav_cnt, gallery.url, article_url, gallery.title
            ),
        )?;

        fs::File::create("./LAST_TIME")?.write_all(gallery.post_time.to_rfc3339().as_bytes())?;
    }

    Ok(())
}

fn main() {
    env_logger::init();

    match run() {
        Ok(()) => (),
        Err(e) => eprintln!("{}", e),
    }
}
