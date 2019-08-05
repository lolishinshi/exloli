#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

use crate::{config::Config, exhentai::BasicGalleryInfo};
use chrono::{prelude::*, Duration};
use failure::Error;
use rayon::prelude::*;
use reqwest::Client;
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    fs::create_dir_all,
    sync::{
        atomic::{AtomicU32, Ordering::SeqCst},
        Arc,
    },
    thread::sleep,
    time,
};
use telegraph_rs::{html_to_node, Telegraph, UploadResult};
use tempfile::NamedTempFile;
use std::fs::File;
use std::path::Path;

mod config;
mod exhentai;
mod telegram;
mod xpath;

lazy_static! {
    static ref CONFIG: Config = Config::new("config.toml").unwrap_or_else(|e| {
        eprintln!("配置文件解析失败:\n{}", e);
        std::process::exit(1);
    });
}

/// 通过 URL 上传图片至 telegraph
pub fn upload_by_url(url: &str, path: &str) -> Result<UploadResult, Error> {
    let client = Client::new();
    // 下载图片
    debug!("下载图片: {}", url);

    let mut response = client.get(url).send()?;

    let file = if Path::new(path).exists() {
        Path::new(path).to_owned()
    } else {
        if CONFIG.exhentai.local_cache {
            let mut file = File::create(path)?;
            io::copy(&mut response, &mut file)?;
            Path::new(path).to_owned()
        } else {
            let mut file = NamedTempFile::new()?;
            io::copy(&mut response, &mut file)?;
            file.path().to_owned()
        }
    };

    let result = if CONFIG.telegraph.upload {
        debug!("上传图片: {:?}", file);
        let result = Telegraph::upload(&[file])?.swap_remove(0);
        result
    } else {
        UploadResult { src: "".to_owned() }
    };

    Ok(result)
}

/// 将 tag 转换为可以直接发送至 tg 的文本格式
fn tags_to_string(tags: &HashMap<String, Vec<String>>) -> String {
    tags.iter()
        .map(|(k, v)| {
            let v = v
                .iter()
                .map(|s| {
                    let s = s.replace(' ', "_").replace("_|_", " #").replace('-', "_");
                    format!("#{}", s)
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("<code>{:>9}</code>: {}", k, v)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 获取上一本爬取的本子的发布时间
fn load_last_time() -> Result<DateTime<Local>, Error> {
    if std::path::Path::new("./LAST_TIME").exists() {
        let mut s = String::new();
        std::fs::File::open("./LAST_TIME")?.read_to_string(&mut s)?;
        Ok(s.parse::<DateTime<Local>>()?)
    } else {
        // 默认从两天前开始
        Ok(Local::now() - Duration::days(2))
    }
}

/// 将图片地址格式化为 html
fn img_urls_to_html(img_urls: &[String]) -> String {
    img_urls
        .iter()
        .map(|s| format!(r#"<img src="{}">"#, s))
        .collect::<Vec<_>>()
        .join("")
}

/// 从图片页面地址获取图片原始地址
fn get_img_urls(gallery: BasicGalleryInfo, img_pages: &[String]) -> Vec<String> {
    let img_cnt = img_pages.len();
    let idx = Arc::new(AtomicU32::new(0));

    if CONFIG.exhentai.local_cache {
        let path = format!("{}/{}", &CONFIG.exhentai.cache_path, &gallery.title);
        create_dir_all( path).unwrap();
    }

    img_pages
        .par_iter()
        .enumerate()
        .map(|(i, url)| {
            let now = idx.load(SeqCst);
            info!("第 {} / {} 张图片", now + 1, img_cnt);
            idx.store(now + 1, SeqCst);
            for _ in 0..5 {
                let img_url = gallery
                    .get_image_url(url)
                    .and_then(|img_url| {
                        let path = format!("{}/{}/{}", &CONFIG.exhentai.cache_path, &gallery.title, i);
                        upload_by_url(&img_url, &path)
                    })
                    .map(|result| result.src);
                match img_url {
                    Ok(v) => return v,
                    Err(e) => {
                        error!("获取图片地址失败: {}", e);
                        sleep(time::Duration::from_secs(10));
                    }
                }
            }
            std::process::exit(1);
        })
        .collect::<Vec<String>>()
}

fn run(config: &Config) -> Result<(), Error> {
    info!("登录中...");
    let bot = config.init_telegram();
    let exhentai = config.init_exhentai()?;
    let telegraph = config.init_telegraph()?;

    // 筛选最新本子
    let last_time = load_last_time()?;
    let galleries = exhentai.search_galleries_after(&config.exhentai.keyword, last_time)?;

    // 从后往前爬, 防止半路失败导致进度记录错误
    for gallery in galleries.into_iter().rev() {
        info!("画廊名称: {}", gallery.title);
        info!("画廊地址: {}", gallery.url);

        let gallery_info = gallery.get_full_info()?;

        let max_length = gallery_info
            .img_pages
            .len()
            .min(config.exhentai.max_img_cnt);
        info!("保留图片数量: {}", max_length);
        let img_urls = get_img_urls(gallery, &gallery_info.img_pages[..max_length]);

        if CONFIG.telegraph.upload {
            info!("发布文章");
            let mut content = img_urls_to_html(&img_urls);
            if gallery_info.img_pages.len() > config.exhentai.max_img_cnt {
                content.push_str(
                    r#"<p>图片数量过多, 只显示部分. 完整版请前往 E 站观看.</p>"#,
                );
            }
            let page = telegraph.create_page(&gallery_info.title, &html_to_node(&content), false)?;
            info!("文章地址: {}", page.url);

            let tags = tags_to_string(&gallery_info.tags);
            bot.send_message(
                &config.telegram.channel_id,
                &format!(
                    "{}\n<a href=\"{}\">{}</a>",
                    tags, page.url, gallery_info.title
                ),
                &gallery_info.url,
            )?;
        }

        std::fs::File::create("./LAST_TIME")?
            .write_all(gallery_info.post_time.to_rfc3339().as_bytes())?;
    }

    Ok(())
}

fn main() {
    let config = Config::new("config.toml").unwrap_or_else(|e| {
        eprintln!("配置文件解析失败:\n{}", e);
        std::process::exit(1);
    });

    // 设置相关环境变量
    if let Some(log_level) = config.log_level.as_ref() {
        std::env::set_var("RUST_LOG", format!("exloli={}", log_level));
    }
    if let Some(threads_num) = config.threads_num.as_ref() {
        std::env::set_var("RAYON_NUM_THREADS", threads_num);
    }

    env_logger::init();

    for _ in 0..3 {
        match run(&config) {
            Ok(()) => {
                info!("任务完成!");
                return;
            }
            Err(e) => {
                error!("任务出错: {}", e);
                sleep(time::Duration::from_secs(60));
            }
        }
    }
}
