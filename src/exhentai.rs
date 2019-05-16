use crate::xpath::parse_html;
use chrono::prelude::*;
use failure::{format_err, Error};
use lazy_static::lazy_static;
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, ClientBuilder, StatusCode,
};

lazy_static! {
    static ref HEADERS: HeaderMap = {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, deflate, br"),
        );
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,en-US;q=0.7,en;q=0.3"),
        );
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
        headers.insert(header::DNT, HeaderValue::from_static("1"));
        headers.insert(header::HOST, HeaderValue::from_static("exhentai.org"));
        headers.insert(
            header::REFERER,
            HeaderValue::from_static("https://exhentai.org/"),
        );
        headers.insert(
            header::UPGRADE_INSECURE_REQUESTS,
            HeaderValue::from_static("1"),
        );
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (X11; Linux x86_64; rv:66.0) Gecko/20100101 Firefox/66.0",
            ),
        );
        headers
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
    /// 图片 URL
    pub img_urls: Vec<String>,
}

#[derive(Debug)]
pub struct ExHentai {
    client: Client,
}

impl ExHentai {
    /// 登录 E-Hentai (能够访问 ExHentai 的前置条件
    /// FIXME: 需要 search 一次后才能够正常访问 gallery 页面
    pub fn new(username: &str, password: &str) -> Result<Self, Error> {
        let client = ClientBuilder::new().cookie_store(true).build()?;

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
        let mut response = self
            .client
            .get("https://exhentai.org")
            .query(&[("f_search", keyword), ("page", &page.to_string())])
            .send()?;
        let html = parse_html(response.text()?)?;

        let gallery_list = html
            .xpath(r#"//table[@class="itg gltc"]/tr[position() > 1]"#)?
            .into_element()
            .unwrap();

        let mut ret = vec![];
        for gallery in gallery_list {
            let title = gallery
                .xpath(r#".//td[@class="gl3c glname"]/a/div/text()"#)?
                .into_text()
                .unwrap()
                .swap_remove(0);
            let url = gallery
                .xpath(r#".//td[@class="gl3c glname"]/a/@href"#)?
                .into_text()
                .unwrap()
                .swap_remove(0);
            let post_time = Local
                .datetime_from_str(
                    &gallery
                        .xpath(r#".//td[@class="gl2c"]//div[contains(@id, "posted")]/text()"#)?
                        .into_text()
                        .unwrap()[0],
                    "%Y-%m-%d %H:%M",
                )
                .expect("解析时间失败");
            ret.push(Gallery {
                title,
                url,
                post_time,
                rating: String::new(),
                fav_cnt: String::new(),
                img_urls: vec![],
            })
        }

        Ok(ret)
    }

    pub fn get_gallery(&self, url: &str) -> Result<(String, String, Vec<String>), Error> {
        let mut response = self.client.get(url).send()?;
        let mut html = parse_html(response.text()?)?;

        let rating = html
            .xpath(r#"//td[@id="rating_label"]/text()"#)?
            .into_text()
            .unwrap()
            .swap_remove(0)
            .split(' ')
            .skip(1)
            .next()
            .unwrap()
            .to_owned();
        let fav_cnt = html
            .xpath(r#"//td[@id="favcount"]/text()"#)?
            .into_text()
            .unwrap()
            .swap_remove(0);
        let mut img_pages = html
            .xpath(r#"//div[@class="gdtl"]/a/@href"#)?
            .into_text()
            .unwrap();

        while let Some(mut next_page) = html
            .xpath(r#"//table[@class="ptt"]//td[last()]/a/@href"#)?
            .into_text()
        {
            let mut response = self.client.get(&next_page.swap_remove(0)).send()?;
            html = parse_html(response.text()?)?;
            img_pages.extend(
                html.xpath(r#"//div[@class="gdtl"]/a/@href"#)?
                    .into_text()
                    .unwrap(),
            )
        }
        Ok((rating, fav_cnt, img_pages))
    }

    /// 根据图片页面的 URL 获取图片的真实地址
    pub fn get_image_url(&self, url: &str) -> Result<String, Error> {
        let mut response = self.client.get(url).send()?;
        let html = parse_html(response.text()?)?;
        Ok(html
            .xpath(r#"//img[@id="img"]/@src"#)?
            .into_text()
            .unwrap()
            .swap_remove(0))
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
