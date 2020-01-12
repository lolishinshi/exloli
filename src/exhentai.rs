use crate::xpath::parse_html;
use chrono::prelude::*;
use failure::Error;
use lazy_static::lazy_static;
use log::{debug, error, info};
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Client, redirect::Policy};
use std::collections::HashMap;

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
        USER_AGENT => "Mozilla/5.0 (X11; Linux x86_64; rv:66.0) Gecko/20100101 Firefox/66.0"
    };
}

/// 基本画廊信息
#[derive(Debug, Clone)]
pub struct BasicGalleryInfo<'a> {
    client: &'a Client,
    /// 画廊标题
    pub title: String,
    /// 画廊地址
    pub url: String,
    /// 发布时间,
    pub post_time: DateTime<Local>,
}

impl<'a> BasicGalleryInfo<'a> {
    /// 获取画廊的完整信息
    pub async fn get_full_info(&self) -> Result<FullGalleryInfo, Error> {
        debug!("获取画廊信息: {}", self.url);
        let response = self.client.get(&self.url).send().await?;
        debug!("状态码: {}", response.status());
        let mut html = parse_html(response.text().await?)?;

        // 标签
        let mut tags = HashMap::new();
        for ele in html.xpath_elem(r#"//div[@id="taglist"]//tr"#)? {
            let tag_set_name = ele.xpath_text(r#"./td[1]/text()"#)?[0]
                .trim_matches(':')
                .to_owned();
            let tag = ele.xpath_text(r#"./td[2]/div/a/text()"#)?;
            tags.insert(tag_set_name, tag);
        }
        debug!("tags: {:?}", tags);

        // 评分
        let rating = html.xpath_text(r#"//td[@id="rating_label"]/text()"#)?[0]
            .split(' ')
            .nth(1)
            .unwrap()
            .to_owned();
        debug!("评分: {}", rating);

        // 收藏
        let fav_cnt = html.xpath_text(r#"//td[@id="favcount"]/text()"#)?[0]
            .split(' ')
            .next()
            .unwrap()
            .to_owned();
        debug!("收藏数: {}", fav_cnt);

        // 图片页面
        let mut img_pages = html.xpath_text(r#"//div[@class="gdtl"]/a/@href"#)?;

        // 继续翻页 (如果有
        while let Ok(mut next_page) =
            html.xpath_text(r#"//table[@class="ptt"]//td[last()]/a/@href"#)
        {
            debug!("下一页: {:?}", next_page);
            let response = self.client.get(&next_page.swap_remove(0)).send().await?;
            html = parse_html(response.text().await?)?;
            img_pages.extend(html.xpath_text(r#"//div[@class="gdtl"]/a/@href"#)?)
        }

        Ok(FullGalleryInfo {
            title: self.title.clone(),
            url: self.url.clone(),
            post_time: self.post_time,
            rating,
            fav_cnt,
            img_pages,
            tags,
        })
    }

    /// 根据图片页面的 URL 获取图片的真实地址
    pub async fn get_image_url(&self, url: &str) -> Result<String, Error> {
        debug!("获取图片真实地址");
        let response = self.client.get(url).send().await?;
        debug!("状态码: {}", response.status());
        let html = parse_html(response.text().await?)?;
        Ok(html.xpath_text(r#"//img[@id="img"]/@src"#)?.swap_remove(0))
    }
}

/// 画廊信息
#[derive(Debug)]
pub struct FullGalleryInfo {
    /// 画廊标题
    pub title: String,
    /// 画廊地址
    pub url: String,
    /// 发布时间,
    pub post_time: DateTime<Local>,
    /// 评分
    pub rating: String,
    /// 收藏次数
    pub fav_cnt: String,
    /// 标签
    pub tags: HashMap<String, Vec<String>>,
    /// 图片 URL
    pub img_pages: Vec<String>,
}

#[derive(Debug)]
pub struct ExHentai {
    client: Client,
    search_page: String,
}

impl ExHentai {
    /// 登录 E-Hentai (能够访问 ExHentai 的前置条件
    pub async fn new(username: &str, password: &str, search_watched: bool) -> Result<Self, Error> {
        // 此处手动设置重定向, 因为 reqwest 的默认重定向处理策略会把相同 URL 直接判定为无限循环
        // 然而其实 COOKIE 变了, 所以不会无限循环
        let custom = Policy::custom(|attempt| {
            if attempt.previous().len() > 3 {
                attempt.error("too many redirects")
            } else {
                attempt.follow()
            }
        });

        let client = Client::builder()
            .redirect(custom)
            .cookie_store(true)
            .default_headers(HEADERS.clone())
            .build()?;
        info!("登录表站...");
        // 登录表站, 获得 cookie
        let _response = client
            .post("https://forums.e-hentai.org/index.php")
            .query(&[("act", "Login"), ("CODE", "01")])
            .form(&[
                ("CookieDate", "1"),
                ("b", "d"),
                ("bt", "1-6"),
                ("UserName", username),
                ("PassWord", password),
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

        Ok(Self {
            client,
            search_page: if search_watched {
                "https://exhentai.org/watched".to_owned()
            } else {
                "https://exhentai.org".to_owned()
            },
        })
    }

    /// 直接通过 cookie 登录
    pub async fn from_cookie(cookie: &str, search_watched: bool) -> Result<Self, Error> {
        let mut headers = HEADERS.clone();
        headers.insert(header::COOKIE, HeaderValue::from_str(cookie)?);

        let client = Client::builder()
            .cookie_store(true)
            .default_headers(headers)
            .build()?;

        let _response = client
            .get("https://exhentai.org/uconfig.php")
            .send()
            .await?;
        info!("登录成功!");

        Ok(Self {
            client,
            search_page: if search_watched {
                "https://exhentai.org/watched".to_owned()
            } else {
                "https://exhentai.org".to_owned()
            },
        })
    }

    /// 搜索指定关键字
    pub async fn search<'a>(
        &'a self,
        keyword: &str,
        page: i32,
    ) -> Result<Vec<BasicGalleryInfo<'a>>, Error> {
        debug!("搜索 {} - {}", keyword, page);
        let response = self
            .client
            .get(&self.search_page)
            .query(&[("f_search", keyword), ("page", &page.to_string())])
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

            let post_time = Local
                .datetime_from_str(
                    &gallery.xpath_text(
                        r#".//td[@class="gl2c"]//div[contains(@id, "posted")]/text()"#,
                    )?[0],
                    "%Y-%m-%d %H:%M",
                )
                .expect("解析时间失败");
            debug!("发布时间: {}", post_time);

            ret.push(BasicGalleryInfo {
                client: &self.client,
                title,
                url,
                post_time,
            })
        }

        Ok(ret)
    }

    pub async fn search_galleries_after<'a>(
        &'a self,
        keyword: &str,
        time: DateTime<Local>,
    ) -> Result<Vec<BasicGalleryInfo<'a>>, Error> {
        info!("搜索 {:?} 之前的本子", time);
        // generator 还未稳定, 用 from_fn + flatten 凑合一下
        let mut result = vec![];
        'l: for page in 0.. {
            match self.search(keyword, page).await {
                Ok(v) => {
                    for gallery in v {
                        // FIXME: 由于时间只精确到分钟, 此处存在极小的忽略掉本子的可能性
                        if gallery.post_time <= time {
                            break 'l;
                        }
                        result.push(gallery);
                    }
                }
                Err(e) => {
                    error!("{}", e);
                    break;
                }
            }
        }

        info!("找到 {} 本", result.len());
        Ok(result)
    }

    pub async fn get_gallery_by_url<'a>(
        &'a self,
        url: &str,
    ) -> Result<BasicGalleryInfo<'a>, Error> {
        info!("获取本子信息: {}", url);
        let response = self.client.get(url).send().await?;
        let html = parse_html(response.text().await?)?;
        let title = html.xpath_text(r#"//h1[@id="gn"]/text()"#)?.swap_remove(0);
        Ok(BasicGalleryInfo {
            client: &self.client,
            title,
            url: url.to_owned(),
            // 不需要时间, 随便填一个吧
            post_time: Local.datetime_from_str("1926-08-17 00:00", "%Y-%m-%d %H:%M")?,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_login() {}
}
