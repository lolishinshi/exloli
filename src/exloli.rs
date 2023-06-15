use crate::database::Gallery;
use crate::ehentai::EHentaiClient;
use crate::exhentai::*;
use crate::utils::*;
use crate::{BOT, CONFIG, DB};
use anyhow::Result;
use chrono::{Datelike, Duration, Timelike, Utc};
use futures::TryFutureExt;
use telegraph_rs::{html_to_node, Page, Telegraph};
use teloxide::prelude::*;
use teloxide::types::{MessageId, ParseMode};
use v_htmlescape::escape;

pub struct ExLoli {
    telegraph: Telegraph,
    ehentai: EHentaiClient,
}

impl ExLoli {
    pub async fn new() -> Result<Self> {
        let telegraph = CONFIG.init_telegraph().await?;
        Ok(ExLoli { telegraph })
    }

    /// 根据配置文件自动扫描并上传本子
    pub async fn scan_and_upload(&self) -> Result<()> {
        // 筛选最新本子
        let page_limit = CONFIG.exhentai.search_pages;
        let galleries = EXHENTAI.search_n_pages(page_limit).await?;

        // 从后往前爬, 保持顺序
        for gallery in galleries.into_iter().rev() {
            info!("检测中：{}", gallery.url);
            match DB.query_gallery_by_url(&gallery.url) {
                Ok(g) => {
                    self.update_gallery_tag(g, gallery)
                        .await
                        .log_on_error()
                        .await;
                }
                _ => {
                    self.upload_gallery(gallery).await.log_on_error().await;
                }
            }
        }
        Ok(())
    }

    /// 更新画廊信息
    async fn update_gallery_tag<'a>(
        &'a self,
        g: Gallery,
        gallery: BasicGalleryInfo<'a>,
    ) -> Result<()> {
        let now = Utc::now();
        let duration = Utc::today().naive_utc() - g.publish_date;
        // 已删除画廊不更新
        if (g.score - -1.0).abs() < f32::EPSILON
            // 7 天前的本子，如果是同一 weekday 发的则更新
            || (duration.num_days() > 7 && !(now.weekday() == g.publish_date.weekday() && now.hour() % 8 == 0))
            // 两天前的本子，每 4 小时更新一次
            || (duration.num_days() > 2 && !(now.hour() % 4 == 0))
        {
            debug!("跳过更新：{}", g.publish_date);
            return Ok(());
        }

        // 检测是否需要更新 tag
        // TODO: 将 tags 塞到 BasicInfo 里
        let info = gallery.into_full_info().await?;
        let new_tags = serde_json::to_string(&info.tags)?;
        if new_tags != g.tags {
            info!("tag 有更新，同步中...");
            info!("画廊名称: {}", info.title);
            info!("画廊地址: {}", info.url);
            self.update_tag(&g, Some(&info)).await?;
        }
        Ok(())
    }

    /// 上传指定 URL 的画廊
    pub async fn upload_gallery_by_url(&self, url: &str) -> Result<()> {
        let gallery = match url.split_once('#') {
            Some((url, cover_index)) => {
                let mut gallery = EXHENTAI.get_gallery_by_url(url).await?;
                gallery.limit = false;
                gallery.cover_index = cover_index.parse()?;
                gallery
            }
            _ => {
                let mut gallery = EXHENTAI.get_gallery_by_url(url).await?;
                gallery.limit = false;
                gallery
            }
        };
        self.upload_gallery(gallery).await
    }

    /// 将画廊上传到 telegram
    async fn upload_gallery<'a>(&'a self, basic_info: BasicGalleryInfo<'a>) -> Result<()> {
        info!("上传中，画廊名称: {}", basic_info.title);

        let mut gallery = basic_info.clone().into_full_info().await?;

        // 判断是否上传过历史版本
        let old_gallery = Self::get_history_upload(&gallery).await;
        match &old_gallery {
            Ok(g) => {
                // 上传量已经达到限制的，不做更新
                if g.upload_images as usize == CONFIG.exhentai.max_img_cnt && gallery.limit {
                    return Err(anyhow::anyhow!("NoNeedToUpdate"));
                }
                // outdate 天以内上传过的，不重复发，在原消息的基础上更新
                // 没有图片增删的，也不重复发送
                let outdate = CONFIG.exhentai.outdate.unwrap_or(7);
                let not_outdated =
                    g.publish_date + Duration::days(outdate) > Utc::today().naive_utc();
                let not_bigupdate = gallery.img_pages.len() == g.upload_images as usize;

                // FIXME: 当前判断方法可能会误判，而且修改最大图片数量以后会失效
                // 如果曾经更新过完整版，则继续上传完整版
                if g.upload_images as usize > CONFIG.exhentai.max_img_cnt {
                    gallery.limit = false;
                }

                // 如果没有过期或者没有图片修改，则直接更新历史消息
                if not_outdated || not_bigupdate {
                    info!("找到历史上传：{}", g.message_id);
                    return self.update_gallery(g, Some(gallery), false).await;
                } else {
                    info!("历史上传已过期：{}", g.message_id);
                }
            }
            Err(e) => warn!("没有找到历史上传：{}", e),
        }

        let mut img_urls = gallery.upload_images_to_telegraph().await?;
        img_urls.swap(0, basic_info.cover_index);

        // 上传到 telegraph
        let title = gallery.title();
        let content = Self::get_article_string(
            &img_urls,
            gallery.img_pages.len(),
            old_gallery.as_ref().ok().map(|g| g.upload_images as usize),
        );
        let page = self.publish_to_telegraph(title, &content).await?;
        info!("文章地址: {}", page.url);

        // 不需要原地更新的旧本子，发布新消息
        let message = self.publish_to_telegram(&gallery, &page.url).await?;

        // 生成 poll_id，仅当旧画廊使用的新格式 poll_id 的情况下才会继承
        let poll_id = old_gallery
            .and_then(|g| g.poll_id.parse::<i32>().map_err(anyhow::Error::new))
            .unwrap_or(message.id.0);

        DB.insert_gallery(message.id.0, &gallery, page.url)?;
        DB.update_poll_id(message.id.0, &poll_id.to_string())
    }

    /// 原地更新画廊，若 gallery 为 None 则原地更新为原画廊的完整版
    pub async fn update_gallery<'a>(
        &self,
        ogallery: &Gallery,
        gallery: Option<FullGalleryInfo<'a>>,
        republish: bool,
    ) -> Result<()> {
        info!("更新画廊：{}", ogallery.get_url());
        if republish {
            let resp = reqwest::get(&ogallery.telegraph).await?;
            if resp.status().as_u16() != 404 {
                return Ok(());
            }
        }

        let gallery = match gallery {
            Some(v) => v,
            None => {
                let mut gallery: FullGalleryInfo = EXHENTAI
                    .get_gallery_by_url(ogallery.get_url())
                    .and_then(|g| g.into_full_info())
                    .await?;
                gallery.limit = false;
                gallery
            }
        };

        let img_urls = gallery.upload_images_to_telegraph().await?;

        let title = gallery.title();
        let content = Self::get_article_string(
            &img_urls,
            gallery.img_pages.len(),
            (ogallery.upload_images != 0).then_some(ogallery.upload_images as usize),
        );

        let page = if republish {
            self.publish_to_telegraph(title, &content).await?
        } else {
            self.edit_telegraph(extract_telegraph_path(&ogallery.telegraph), title, &content)
                .await?
        };

        let url = format!("{}?_={}", page.url, get_timestamp());
        self.update_message(ogallery.message_id, &gallery, &url, img_urls.len())
            .await
    }

    /// 更新 tag，可选手动传入 new_gallery 避免重复请求
    pub async fn update_tag<'a>(
        &self,
        old_gallery: &Gallery,
        new_gallery: Option<&FullGalleryInfo<'a>>,
    ) -> Result<()> {
        let url = old_gallery.get_url();
        let mut _g = None;
        let new_gallery = match new_gallery {
            Some(v) => v,
            None => {
                // fuck lifetime
                _g = Some(
                    EXHENTAI
                        .get_gallery_by_url(&url)
                        .and_then(|g| g.into_full_info())
                        .await?,
                );
                _g.as_ref().unwrap()
            }
        };

        // 更新 telegraph
        let path = extract_telegraph_path(&old_gallery.telegraph);
        let old_page = Telegraph::get_page(path, true).await?;
        let new_page = self
            .telegraph
            .edit_page(
                &old_page.path,
                new_gallery.title_jp.as_ref().unwrap_or(&new_gallery.title),
                &serde_json::to_string(&old_page.content)?,
                false,
            )
            .await?;

        let upload_images = old_gallery.upload_images as usize;
        let message_id = old_gallery.message_id;
        self.update_message(message_id, new_gallery, &new_page.url, upload_images)
            .await
    }

    /// 更新旧消息并同时更新数据库
    async fn update_message<'a>(
        &self,
        message_id: i32,
        gallery: &FullGalleryInfo<'a>,
        article: &str,
        upload_images: usize,
    ) -> Result<()> {
        info!("更新 Telegram 频道消息");
        let text = Self::get_message_string(gallery, article);
        BOT.edit_message_text(
            CONFIG.telegram.channel_id.clone(),
            MessageId(message_id),
            &text,
        )
        .parse_mode(ParseMode::Html)
        .await?;
        DB.update_gallery(message_id, gallery, article, upload_images)
    }

    /// 将画廊内容上传至 telegraph
    async fn publish_to_telegraph<'a>(&self, title: &str, content: &str) -> Result<Page> {
        info!("上传到 Telegraph");
        let text = html_to_node(content);
        trace!("{}", text);
        Ok(self.telegraph.create_page(title, &text, false).await?)
    }

    /// 修改已有的 telegraph 文章
    async fn edit_telegraph<'a>(&self, path: &str, title: &str, content: &str) -> Result<Page> {
        info!("更新 Telegraph: {}", path);
        let text = html_to_node(content);
        trace!("{}", text);
        Ok(self.telegraph.edit_page(path, title, &text, false).await?)
    }

    /// 将画廊内容上传至 telegraph
    async fn publish_to_telegram<'a>(
        &self,
        gallery: &FullGalleryInfo<'a>,
        article: &str,
    ) -> Result<Message> {
        info!("发布到 Telegram 频道");
        let text = Self::get_message_string(gallery, article);
        Ok(BOT
            .send_message(CONFIG.telegram.channel_id.clone(), &text)
            .parse_mode(ParseMode::Html)
            .await?)
    }

    /// 生成用于发送消息的字符串
    fn get_message_string<'a>(gallery: &FullGalleryInfo<'a>, article: &str) -> String {
        let mut tags = tags_to_string(&gallery.tags);
        tags.push_str(&format!(
            "\n<code>  预览</code>: <a href=\"{}\">{}</a>",
            article,
            escape(&gallery.title)
        ));
        tags.push_str(&format!("\n<code>原始地址</code>: {} ", gallery.url));
        tags
    }

    /// 生成 telegraph 文章内容
    fn get_article_string(
        image_urls: &[String],
        total_image: usize,
        last_uploaded: Option<usize>,
    ) -> String {
        let mut content = img_urls_to_html(image_urls);
        if last_uploaded.is_some() || image_urls.len() != total_image {
            content.push_str("<p>");
            content.push_str(&format!("已上传 {}/{}", image_urls.len(), total_image,));
            if let Some(v) = last_uploaded {
                content.push_str(&format!("，上次上传到 {}", v));
            }
            if image_urls.len() != total_image {
                content.push_str("，完整版请前往 E 站观看");
            }
            content.push_str("</p>");
        }
        content
    }
}

impl ExLoli {
    /// 获取画廊的历史上传
    pub async fn get_history_upload<'a>(gallery: &FullGalleryInfo<'a>) -> Result<Gallery> {
        let mut gallery_url = Some(gallery.url.clone());
        while let Some(url) = &gallery_url {
            match DB.query_gallery_by_url(url) {
                Ok(v) => return Ok(v),
                _ => {
                    let gallery = EXHENTAI.get_gallery_by_url(url).await?;
                    let parent = gallery.into_full_info().await?;
                    gallery_url = parent.parent;
                }
            }
        }
        Err(anyhow!("Not Found"))
    }
}
