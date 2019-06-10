use crate::xpath::parse_html;
use chrono::prelude::*;
use failure::Error;
use lazy_static::lazy_static;
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, ClientBuilder, RedirectPolicy,
};
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
#[derive(Debug)]
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
    pub fn get_full_info(&self) -> Result<FullGalleryInfo, Error> {
        debug!("获取画廊信息: {}", self.url);
        let mut response = self.client.get(&self.url).send()?;
        debug!("状态码: {}", response.status());
        let mut html = parse_html(response.text()?)?;

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
            let mut response = self.client.get(&next_page.swap_remove(0)).send()?;
            html = parse_html(response.text()?)?;
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
    pub fn get_image_url(&self, url: &str) -> Result<String, Error> {
        debug!("获取图片真实地址");
        let mut response = self.client.get(url).send()?;
        debug!("状态码: {}", response.status());
        let html = parse_html(response.text()?)?;
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
    pub fn new(username: &str, password: &str, search_watched: bool) -> Result<Self, Error> {
        let client = ClientBuilder::new()
            .redirect(RedirectPolicy::none())
            .cookie_store(true)
            .build()?;
        info!("登录表站...");
        // 登录表站, 获得 cookie
        let _response = client
            .post("https://forums.e-hentai.org/index.php")
            .headers(HEADERS.clone())
            .query(&[("act", "Login"), ("CODE", "01")])
            .form(&[
                ("CookieDate", "1"),
                ("b", "d"),
                ("bt", "1-6"),
                ("UserName", username),
                ("PassWord", password),
                ("ipb_login_submit", "Login!"),
            ])
            .send()?;

        info!("登录里站...");
        // 访问里站, 取得必要的 cookie
        // 此处手动处理重定向, 因为 reqwest 的重定向处理似乎有问题
        let mut response = client.get("https://exhentai.org").send()?;
        for _ in 0..3 {
            let next_url = response.headers().get(header::LOCATION).unwrap().to_str()?;
            response = client.get(next_url).send()?;
        }
        // 获得过滤设置相关的 cookie ?
        let _response = client.get("https://exhentai.org/uconfig.php").send()?;

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
    pub fn search(&self, keyword: &str, page: i32) -> Result<Vec<BasicGalleryInfo>, Error> {
        debug!("搜索 {} - {}", keyword, page);
        let mut response = self
            .client
            .get(&self.search_page)
            .query(&[("f_search", keyword), ("page", &page.to_string())])
            .send()?;
        debug!("状态码: {}", response.status());
        let text = response.text()?;
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

    pub fn search_galleries_after(
        &self,
        keyword: &str,
        time: DateTime<Local>,
    ) -> Result<Vec<BasicGalleryInfo>, Error> {
        info!("搜索 {:?} 之前的本子", time);
        // generator 还未稳定, 用 from_fn + flatten 凑合一下
        let mut page = -1;
        let result = std::iter::from_fn(|| {
            page += 1;
            self.search(keyword, page).ok()
        })
        .flatten()
        // FIXME: 由于时间只精确到分钟, 此处存在极小的忽略掉本子的可能性
        .take_while(|gallery| gallery.post_time > time)
        .collect::<Vec<BasicGalleryInfo>>();

        info!("找到 {} 本", result.len());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::{config::Config, exhentai::ExHentai};

    #[test]
    fn test_login() {
        color_backtrace::install();

        let config = Config::new("./config.toml").unwrap();
        let exhentai = ExHentai::new(
            &config.exhentai.username,
            &config.exhentai.password,
            config.exhentai.search_watched,
        )
        .unwrap();

        for i in exhentai
            .search("female:lolicon language:Chinese", 0)
            .unwrap()
        {
            println!("{:?}", i);
        }

        let x = exhentai.get_gallery("https://exhentai.org/g/1415107/2bd0489932/");
        println!("{:?}", x);
    }
}
