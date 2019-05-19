use crate::xpath::parse_html;
use chrono::prelude::*;
use failure::{format_err, Error};
use lazy_static::lazy_static;
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, ClientBuilder, StatusCode,
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

/// 画廊信息
#[derive(Debug)]
pub struct Gallery {
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
    pub img_urls: Vec<String>,
}

#[derive(Debug)]
pub struct ExHentai {
    client: Client,
}

impl ExHentai {
    /// 登录 E-Hentai (能够访问 ExHentai 的前置条件
    pub fn new(username: &str, password: &str) -> Result<Self, Error> {
        let client = ClientBuilder::new().cookie_store(true).build()?;
        info!("登录表站...");
        // 登录表站, 获得 cookie
        let response = client
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
        if response.status() != StatusCode::OK {
            return Err(format_err!(
                "failed to login: status code {}",
                response.status()
            ));
        }

        info!("登录里站...");
        // 访问里站, 取得必要的 cookie
        let response = client
            .get("https://exhentai.org")
            .query(&[("f_search", "lolicon")])
            .send()?;
        if response.status() != StatusCode::OK {
            return Err(format_err!(
                "failed to login: status code {}",
                response.status()
            ));
        }

        Ok(Self { client })
    }

    /// 搜索指定关键字
    pub fn search(&self, keyword: &str, page: i32) -> Result<Vec<Gallery>, Error> {
        debug!("搜索 {} - {}", keyword, page);
        let mut response = self
            .client
            .get("https://exhentai.org")
            .query(&[("f_search", keyword), ("page", &page.to_string())])
            .send()?;
        debug!("状态码: {}", response.status());
        let html = parse_html(response.text()?)?;

        let gallery_list = html.xpath_elem(r#"//table[@class="itg gltc"]/tr[position() > 1]"#)?;

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
            ret.push(Gallery {
                title,
                url,
                post_time,
                rating: String::new(),
                fav_cnt: String::new(),
                tags: HashMap::new(),
                img_urls: vec![],
            })
        }

        Ok(ret)
    }

    pub fn get_gallery(
        &self,
        url: &str,
    ) -> Result<(String, String, Vec<String>, HashMap<String, Vec<String>>), Error> {
        debug!("获取画廊信息: {}", url);
        let mut response = self.client.get(url).send()?;
        debug!("状态码: {}", response.status());
        let mut html = parse_html(response.text()?)?;

        let mut tags = HashMap::new();

        // get tags
        for ele in html.xpath_elem(r#"//div[@id="taglist"]//tr"#)? {
            let tag_set_name = ele.xpath_text(r#"./td[1]/text()"#)?[0]
                .trim_matches(':')
                .to_owned();
            let tag = ele.xpath_text(r#"./td[2]/div/a/text()"#)?;
            tags.insert(tag_set_name, tag);
        }
        debug!("tags: {:?}", tags);
        let rating = html.xpath_text(r#"//td[@id="rating_label"]/text()"#)?[0]
            .split(' ')
            .nth(1)
            .unwrap()
            .to_owned();
        debug!("评分: {}", rating);
        let fav_cnt = html.xpath_text(r#"//td[@id="favcount"]/text()"#)?[0]
            .split(' ')
            .next()
            .unwrap()
            .to_owned();
        debug!("收藏数: {}", fav_cnt);
        let mut img_pages = html.xpath_text(r#"//div[@class="gdtl"]/a/@href"#)?;

        while let Ok(mut next_page) =
            html.xpath_text(r#"//table[@class="ptt"]//td[last()]/a/@href"#)
        {
            debug!("下一页: {:?}", next_page);
            let mut response = self.client.get(&next_page.swap_remove(0)).send()?;
            html = parse_html(response.text()?)?;
            img_pages.extend(html.xpath_text(r#"//div[@class="gdtl"]/a/@href"#)?)
        }
        Ok((rating, fav_cnt, img_pages, tags))
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

#[cfg(test)]
mod tests {
    use crate::{config::Config, exhentai::ExHentai};

    #[test]
    fn test_login() {
        color_backtrace::install();

        let config = Config::new("./config.toml").unwrap();
        let exhentai = ExHentai::new(&config.exhentai.username, &config.exhentai.password).unwrap();

        // 必须先查询 ?
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
