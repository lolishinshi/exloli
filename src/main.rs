use crate::{
    config::Config,
    exhentai::{ExHentai, Gallery},
    telegram::Bot,
    telegraph::{publish_article, upload_by_url},
};
use chrono::{prelude::*, Duration};
use failure::Error;
use std::{
    fs,
    io::{Read, Write},
    path,
};

mod config;
mod exhentai;
mod telegram;
mod telegraph;
mod xpath;

fn run() -> Result<(), Error> {
    let config = Config::new("config.toml")?;
    println!("登录中...");
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
        println!("画廊名称: {}", gallery.title);
        println!("画廊地址: {}", gallery.url);

        let mut gallery = gallery;
        let (rating, fav_cnt, img_pages) = exhentai.get_gallery(&gallery.url)?;
        gallery.rating.push_str(&rating);
        gallery.fav_cnt.push_str(&fav_cnt);

        for (idx, url) in img_pages.iter().enumerate() {
            println!("{} / {} 张图片", idx + 1, img_pages.len());
            let img_url = exhentai.get_image_url(url)?;
            gallery
                .img_urls
                .push(upload_by_url(&img_url)?[0].src.to_owned());
        }

        let content = gallery
            .img_urls
            .iter()
            .map(|s| format!(r#"{{ "tag":"img", "attrs":{{ "src": "{}" }} }}"#, s))
            .collect::<Vec<_>>()
            .join(",");

        let article_url = publish_article(
            &config.telegraph.access_token,
            &gallery.title,
            &config.telegraph.author_name,
            &config.telegraph.author_url,
            &format!("[{}]", content),
        )?;
        println!("文章地址: {}", article_url);
        bot.send_message(
            &config.telegram.channel_id,
            &format!("评分: {}\n收藏数: {}\n{}", gallery.rating, gallery.fav_cnt, article_url),
        )?;

        fs::File::create("./LAST_TIME")?.write_all(gallery.post_time.to_rfc3339().as_bytes())?;
    }

    Ok(())
}

fn main() {
    match run() {
        Ok(()) => (),
        Err(e) => eprintln!("{}", e),
    }
}
