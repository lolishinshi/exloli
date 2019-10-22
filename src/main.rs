#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

use crate::{
    config::Config,
    exhentai::{BasicGalleryInfo, ExHentai},
    telegram::Bot,
};
use chrono::{prelude::*, Duration};
use failure::{format_err, Error};
use futures::prelude::*;
use reqwest::Client;
use std::{
    collections::HashMap,
    env,
    fs::{create_dir_all, File},
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering::SeqCst},
        Arc,
    },
    time,
};
use telegraph_rs::{html_to_node, Telegraph, UploadResult};
use tempfile::NamedTempFile;
use tokio::timer::delay_for;
use v_htmlescape::escape;

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
pub async fn upload_by_url(url: &str, path: &str) -> Result<UploadResult, Error> {
    let client = Client::new();
    // 下载图片
    debug!("下载图片: {}", url);

    let mut tmp = NamedTempFile::new()?;

    let file = if Path::new(path).exists() {
        Path::new(path).to_owned()
    } else {
        let response = client.get(url).send().await?;
        let bytes = response.bytes().await?;

        if CONFIG.exhentai.local_cache {
            let mut file = File::create(path)?;
            file.write_all(bytes.as_ref())?;
            Path::new(path).to_owned()
        } else {
            tmp.write_all(bytes.as_ref())?;
            tmp.path().to_owned()
        }
    };

    let result = if CONFIG.telegraph.upload {
        debug!("上传图片: {:?}", file);
        Telegraph::upload(&[file]).await?.swap_remove(0)
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
async fn get_img_urls<'a>(gallery: &BasicGalleryInfo<'a>, img_pages: &[String]) -> Vec<String> {
    let img_cnt = img_pages.len();
    let idx = Arc::new(AtomicU32::new(0));

    if CONFIG.exhentai.local_cache {
        let path = format!("{}/{}", &CONFIG.exhentai.cache_path, &gallery.title);
        create_dir_all(path).unwrap();
    }

    let f = img_pages
        .iter()
        .enumerate()
        .map(|(i, url)| {
            let gallery = gallery.clone();
            let idx = idx.clone();
            async move {
                let now = idx.load(SeqCst);
                info!("第 {} / {} 张图片", now + 1, img_cnt);
                idx.store(now + 1, SeqCst);
                // 最多重试五次
                for _ in 0..5i32 {
                    let path = format!("{}/{}/{}", &CONFIG.exhentai.cache_path, &gallery.title, i);
                    let img_url = gallery
                        .get_image_url(url)
                        .and_then(|img_url| async move { upload_by_url(&img_url, &path).await })
                        .await
                        .map(|result| result.src);
                    match img_url {
                        Ok(v) => return Some(v),
                        Err(e) => {
                            error!("获取图片地址失败: {}", e);
                            delay_for(time::Duration::from_secs(10));
                        }
                    }
                }
                None
            }
        })
        .collect::<Vec<_>>();

    futures::stream::iter(f)
        .buffered(CONFIG.threads_num)
        .filter_map(|x| async move { x })
        .collect::<Vec<_>>()
        .await
}

struct ExLoli {
    config: Config,
    bot: Bot,
    exhentai: ExHentai,
    telegraph: Telegraph,
}

impl ExLoli {
    async fn new() -> Result<Self, Error> {
        let config =
            Config::new("config.toml").map_err(|e| format_err!("配置文件解析失败:\n{}", e))?;
        let bot = config.init_telegram();
        let exhentai = config.init_exhentai().await?;
        let telegraph = config.init_telegraph().await?;
        Ok(ExLoli {
            config,
            bot,
            exhentai,
            telegraph,
        })
    }

    async fn scan_and_upload(&self) -> Result<(), Error> {
        // 筛选最新本子
        let last_time = load_last_time()?;
        let galleries = self
            .exhentai
            .search_galleries_after(&self.config.exhentai.keyword, last_time)
            .await?;

        // 从后往前爬, 防止半路失败导致进度记录错误
        for gallery in galleries.into_iter().rev() {
            self.upload_gallery_to_telegram(&gallery).await?;
            std::fs::File::create("./LAST_TIME")?
                .write_all(gallery.post_time.to_rfc3339().as_bytes())?;
        }

        Ok(())
    }

    async fn upload_gallery_by_url(&self, url: &str) -> Result<(), Error> {
        let gallery = self.exhentai.get_gallery_by_url(url).await?;
        self.upload_gallery_to_telegram(&gallery).await
    }

    async fn upload_gallery_to_telegram<'a>(
        &'a self,
        gallery: &BasicGalleryInfo<'a>,
    ) -> Result<(), Error> {
        info!("画廊名称: {}", gallery.title);
        info!("画廊地址: {}", gallery.url);

        let gallery_info = gallery.get_full_info().await?;

        let actual_img_cnt = gallery_info.img_pages.len();
        let max_img_cnt = self.config.exhentai.max_img_cnt;
        let max_length = std::cmp::min(actual_img_cnt, max_img_cnt);

        let img_pages = &gallery_info.img_pages[..max_length];
        info!("保留图片数量: {}", max_length);

        let img_urls = get_img_urls(gallery, img_pages).await;

        if !self.config.telegraph.upload {
            return Ok(());
        }

        info!("发表文章");
        let mut content = img_urls_to_html(&img_urls);
        if actual_img_cnt > max_img_cnt {
            content.push_str(r#"<p>图片数量过多, 只显示部分. 完整版请前往 E 站观看.</p>"#);
        }
        let page = self
            .telegraph
            .create_page(&gallery_info.title, &html_to_node(&content), false)
            .await?;
        info!("文章地址: {}", page.url);

        let tags = tags_to_string(&gallery_info.tags);
        let text = format!(
            "{}\n<a href=\"{}\">{}</a>",
            tags,
            page.url,
            escape(&gallery_info.title)
        );
        self.bot
            .send_message(&self.config.telegram.channel_id, &text, &gallery_info.url)
            .await
    }
}

#[tokio::main]
async fn main() {
    let exloli = ExLoli::new().await.unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });

    let args = env::args().collect::<Vec<_>>();
    env::set_var("RUST_LOG", format!("exloli={}", exloli.config.log_level));
    env_logger::init();

    for _ in 0..3i32 {
        let result = if args.len() == 3 && args[1] == "upload" {
            exloli.upload_gallery_by_url(&args[2]).await
        } else {
            exloli.scan_and_upload().await
        };

        match result {
            Ok(()) => {
                info!("任务完成!");
                return;
            }
            Err(e) => {
                error!("任务出错: {}", e);
                delay_for(time::Duration::from_secs(60));
            }
        }
    }
}
