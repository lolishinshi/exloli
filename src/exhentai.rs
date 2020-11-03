use crate::xpath::parse_html;
use crate::{CONFIG, DB};
use anyhow::{anyhow, Context, Result};
use futures::executor::block_on;
use futures::prelude::*;
use lazy_static::lazy_static;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{redirect::Policy, Client, Proxy, Response};
use telegraph_rs::Telegraph;
use tempfile::NamedTempFile;
use tokio::time::delay_for;

use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::block_in_place;

macro_rules! set_header {
    ($($k:ident => $v:expr), *) => {{
        let mut headers = HeaderMap::new();
        $(headers.insert(header::$k, HeaderValue::from_static($v));)*
        headers
    }};
}

lazy_static! {
    static ref HEADERS: HeaderMap = set_header! {
        ACCEPT => "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ACCEPT_ENCODING => "gzip, deflate, br",
        ACCEPT_LANGUAGE => "zh-CN,en-US;q=0.7,en;q=0.3",
        CACHE_CONTROL => "max-age=0",
        DNT => "1",
        HOST => "exhentai.org",
        REFERER => "https://exhentai.org/",
        UPGRADE_INSECURE_REQUESTS => "1",
        USER_AGENT => "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:67.0) Gecko/20100101 Firefox/67.0"
    };
}

// TODO： 通过调整搜索页面展示的信息将 tag 移到这里来
/// 基本画廊信息
#[derive(Debug, Clone)]
pub struct BasicGalleryInfo<'a> {
    client: &'a Client,
    /// 画廊标题
    pub title: String,
    /// 画廊地址
    pub url: String,
    /// 是否限制图片数量
    pub limit: bool,
}

impl<'a> BasicGalleryInfo<'a> {
    /// 获取画廊的完整信息
    pub async fn into_full_info(self) -> Result<FullGalleryInfo<'a>> {
        debug!("获取画廊信息: {}", self.url);
        let response = self.client.get(&self.url).send().await?;
        debug!("状态码: {}", response.status());
        let mut html = parse_html(response.text().await?)?;

        // 标签
        let mut tags = vec![];
        for ele in html.xpath_elem(r#"//div[@id="taglist"]//tr"#)? {
            let tag_set_name = ele.xpath_text(r#"./td[1]/text()"#)?[0]
                .trim_matches(':')
                .to_owned();
            let tag = ele.xpath_text(r#"./td[2]/div/a/text()"#)?;
            tags.push((tag_set_name, tag));
        }
        debug!("tags: {:?}", tags);

        // 评分
        let rating = html.xpath_text(r#"//td[@id="rating_label"]/text()"#)?[0]
            .split(' ')
            .nth(1)
            .context("找不到评分")?
            .to_owned();
        debug!("评分: {}", rating);

        // 收藏
        let fav_cnt = html.xpath_text(r#"//td[@id="favcount"]/text()"#)?[0]
            .split(' ')
            .next()
            .context("找不到收藏数")?
            .to_owned();
        debug!("收藏数: {}", fav_cnt);

        // 图片页面
        let mut img_pages = html.xpath_text(r#"//div[@class="gdtl"]/a/@href"#)?;

        // 继续翻页 (如果有
        while let Ok(next_page) = html.xpath_text(r#"//table[@class="ptt"]//td[last()]/a/@href"#) {
            debug!("下一页: {:?}", next_page);
            // TODO: 干掉此处的 block_on
            let text = block_in_place(|| {
                block_on(async { self.client.get(&next_page[0]).send().await?.text().await })
            })?;
            html = parse_html(text)?;
            img_pages.extend(html.xpath_text(r#"//div[@class="gdtl"]/a/@href"#)?);
        }

        Ok(FullGalleryInfo {
            client: self.client,
            title: self.title.clone(),
            url: self.url.clone(),
            limit: self.limit,
            rating,
            fav_cnt,
            img_pages,
            tags,
        })
    }
}

/// 画廊信息
#[derive(Debug)]
pub struct FullGalleryInfo<'a> {
    client: &'a Client,
    /// 画廊标题
    pub title: String,
    /// 画廊地址
    pub url: String,
    /// 评分
    pub rating: String,
    /// 收藏次数
    pub fav_cnt: String,
    /// 标签
    pub tags: Vec<(String, Vec<String>)>,
    /// 图片 URL
    pub img_pages: Vec<String>,
    /// 是否限制图片数量
    pub limit: bool,
}

impl<'a> FullGalleryInfo<'a> {
    /// 返回调整数量后的图片页面链接
    pub fn get_image_lists(&self) -> &[String] {
        if !self.limit {
            return &self.img_pages;
        }
        let limit = CONFIG.exhentai.max_img_cnt;
        let img_cnt = self.img_pages.len().min(limit);
        info!("保留图片数量: {}", img_cnt);
        &self.img_pages[..img_cnt]
    }

    /// 将画廊里的图片上传至 telegraph，返回上传后的图片链接
    pub async fn upload_images_to_telegraph(&self) -> Result<Vec<String>> {
        let img_pages = self.get_image_lists();
        let img_cnt = img_pages.len();
        let idx = Arc::new(AtomicU32::new(0));

        let update_progress = || {
            let now = idx.load(Ordering::SeqCst);
            idx.store(now + 1, Ordering::SeqCst);
            info!("第 {} / {} 张图片", now + 1, img_cnt);
        };

        let mut f = vec![];
        for url in img_pages.iter() {
            f.push(async move {
                update_progress();
                // TODO: 此处不应返回 None，上传失败时应该整体重来
                for _ in 0..5i32 {
                    let result = self.upload_image(url).await;
                    match result {
                        Ok(v) => {
                            DB.insert_image(url, &v).expect("插入图片失败");
                            return Some(v);
                        }
                        Err(e) => {
                            error!("获取图片地址失败：{}", e);
                            delay_for(Duration::from_secs(10)).await;
                        }
                    }
                }
                None
            });
        }

        let ret = futures::stream::iter(f)
            .buffered(CONFIG.threads_num)
            .filter_map(|x| async move { x })
            .collect::<Vec<_>>()
            .await;

        Ok(ret)
    }

    /// 上传指定的图片并返回上传后的地址
    pub async fn upload_image(&self, url: &str) -> Result<String> {
        debug!("获取图片真实地址中：{}", url);
        if let Ok(image) = DB.query_image_by_url(url) {
            trace!("找到缓存!");
            return Ok(image.url);
        }
        let response = self.client.get(url).send().await?;
        trace!("状态码: {}", response.status());

        let url = parse_html(response.text().await?)?
            .xpath_text(r#"//img[@id="img"]/@src"#)?
            .swap_remove(0);

        debug!("下载图片中：{}", &url);
        // TODO: 是否有必要创建新的 client？
        let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
        let bytes = client.get(&url).send().and_then(Response::bytes).await?;
        let mut tmp = NamedTempFile::new()?;
        tmp.write_all(bytes.as_ref())?;
        let file = tmp.path().to_owned();

        debug!("上传图片中...");
        let mut result = Telegraph::upload(&[file])
            .await
            .map_err(|e| anyhow!("上传 telegraph 失败：{}", e))?;
        Ok(result.swap_remove(0).src)
    }
}

#[derive(Debug)]
pub struct ExHentai {
    client: Client,
}

impl ExHentai {
    /// 登录 E-Hentai (能够访问 ExHentai 的前置条件
    pub async fn new() -> Result<Self> {
        // 此处手动设置重定向, 因为 reqwest 的默认重定向处理策略会把相同 URL 直接判定为无限循环
        // 然而其实 COOKIE 变了, 所以不会无限循环
        let custom = Policy::custom(|attempt| {
            if attempt.previous().len() > 3 {
                attempt.error("too many redirects")
            } else {
                attempt.follow()
            }
        });

        let mut client = Client::builder()
            .redirect(custom)
            .cookie_store(true)
            .default_headers(HEADERS.clone());
        if let Some(proxy) = &CONFIG.exhentai.proxy {
            client = client.proxy(Proxy::all(proxy)?)
        }
        let client = client.build()?;

        info!("登录表站...");
        // 登录表站, 获得 cookie
        let _response = client
            .post("https://forums.e-hentai.org/index.php")
            .query(&[("act", "Login"), ("CODE", "01")])
            .form(&[
                ("CookieDate", "1"),
                ("b", "d"),
                ("bt", "1-6"),
                ("UserName", &CONFIG.exhentai.username),
                ("PassWord", &CONFIG.exhentai.password),
                ("ipb_login_submit", "Login!"),
            ])
            .send()
            .await?;

        info!("登录里站...");
        // 访问里站, 取得必要的 cookie
        let _response = client.get("https://exhentai.org").send().await?;
        // 获得过滤设置相关的 cookie ?
        let _response = client
            .get("https://exhentai.org/uconfig.php")
            .send()
            .await?;
        info!("登录成功!");

        Ok(Self { client })
    }

    /// 直接通过 cookie 登录
    pub async fn from_cookie() -> Result<Self> {
        let mut headers = HEADERS.clone();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(CONFIG.exhentai.cookie.as_ref().unwrap())?,
        );

        let mut client = Client::builder()
            .cookie_store(true)
            .default_headers(headers);
        if let Some(proxy) = &CONFIG.exhentai.proxy {
            client = client.proxy(Proxy::all(proxy)?)
        }
        let client = client.build()?;

        let _response = client
            .get("https://exhentai.org/uconfig.php")
            .send()
            .await?;
        info!("登录成功!");

        Ok(Self { client })
    }

    /// 搜索指定关键字
    pub async fn search<'a>(&'a self, page: i32) -> Result<Vec<BasicGalleryInfo<'a>>> {
        debug!("搜索第 {} 页", page);
        let response = self
            .client
            .get(&CONFIG.exhentai.search_url)
            .query(&CONFIG.exhentai.search_params)
            .query(&[("page", &page.to_string())])
            .send()
            .await?;
        debug!("状态码: {}", response.status());
        let text = response.text().await?;
        debug!("返回: {}", &text[..100.min(text.len())]);
        let html = parse_html(text)?;

        let gallery_list = html.xpath_elem(r#"//table[@class="itg gltc"]/tr[position() > 1]"#)?;
        debug!("数量: {}", gallery_list.len());

        let mut ret = vec![];
        for gallery in gallery_list {
            let title = gallery
                .xpath_text(r#".//td[@class="gl3c glname"]/a/div/text()"#)?
                .swap_remove(0);
            debug!("标题: {}", title);

            let url = gallery
                .xpath_text(r#".//td[@class="gl3c glname"]/a/@href"#)?
                .swap_remove(0);
            debug!("地址: {}", url);

            ret.push(BasicGalleryInfo {
                client: &self.client,
                title,
                url,
                limit: true,
            })
        }

        Ok(ret)
    }

    pub async fn search_n_pages<'a>(&'a self, n: i32) -> Result<Vec<BasicGalleryInfo<'a>>> {
        info!("搜索前 {} 页本子", n);
        let mut result = vec![];
        for page in 0..n {
            match self.search(page).await {
                Ok(v) => result.extend(v),
                Err(e) => error!("{}", e),
            }
        }
        info!("找到 {} 本", result.len());
        Ok(result)
    }

    pub async fn get_gallery_by_url<'a>(&'a self, url: &str) -> Result<BasicGalleryInfo<'a>> {
        info!("获取本子信息: {}", url);
        let response = self.client.get(url).send().await?;
        let html = parse_html(response.text().await?)?;
        let title = html.xpath_text(r#"//h1[@id="gn"]/text()"#)?.swap_remove(0);
        Ok(BasicGalleryInfo {
            client: &self.client,
            title,
            url: url.to_owned(),
            limit: true,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_login() {}
}
