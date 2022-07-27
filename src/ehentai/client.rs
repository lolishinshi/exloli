use anyhow::Result;
use reqwest::header::*;
use reqwest::{Client, Response};
use std::time::Duration;
use crate::ehentai::types::Gallery;
use crate::xpath::parse_html;

macro_rules! send {
    ($e:expr) => {
        $e.send().await.and_then(Response::error_for_status)
    };
}

const DEFAULT_HEADERS: [(HeaderName, &'static str); 9] = [
    (
        ACCEPT,
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    ),
    (ACCEPT_ENCODING, "gzip, deflate, br"),
    (ACCEPT_LANGUAGE, "zh-CN,en-US;q=0.7,en;q=0.3"),
    (CACHE_CONTROL, "max-age=0"),
    (CONNECTION, "keep-alive"),
    // TODO: 支持 e-hentai
    (HOST, "exhentai.org"),
    (REFERER, "https://exhentai.org"),
    (UPGRADE_INSECURE_REQUESTS, "1"),
    (
        USER_AGENT,
        "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:67.0) Gecko/20100101 Firefox/67.0",
    ),
];

pub struct EHentaiClient {
    client: Client,
}

impl EHentaiClient {
    pub async fn new(cookie: String) -> Result<Self> {
        let mut headers = DEFAULT_HEADERS
            .iter()
            .map(|(k, v)| (k.clone(), v.parse().unwrap()))
            .collect::<HeaderMap>();
        headers.insert(COOKIE, cookie.parse().unwrap());

        let client = Client::builder()
            .cookie_store(true)
            .default_headers(headers)
            .timeout(Duration::from_secs(15))
            .build()?;

        let _response = send!(client.get("https://exhentai.org/uconfig.php"))?;
        let _response = send!(client.get("https://exhentai.org/mytags"))?;

        Ok(Self { client })
    }

    /// 使用指定参数查询符合要求的画廊列表
    pub async fn search(&self, params: &[(&str, &str)], page: i32) -> Result<Vec<(String, String)>> {
        let resp = send!(self
            .client
            .get("https://exhentai.org")
            .query(params)
            .query(&[("page", &page.to_string())]))?;
        let text = resp.text().await?;
        let html = parse_html(text)?;

        let gl_list = html.xpath_elem(r#"//table[@class="itg gltc"]/tr[position() > 1]"#)?;

        let mut ret = vec![];
        for gl in gl_list {
            let title = gl
                .xpath_text(r#".//td[@class="gl3c glname"]/a/div/text()"#)?
                .swap_remove(0);

            let url = gl
                .xpath_text(r#".//td[@class="gl3c glname"]/a/@href"#)?
                .swap_remove(0);

            ret.push((title, url))
        }

        Ok(ret)
    }

    /// 根据画廊 URL 获取画廊的完整信息
    pub async fn gallery(&self, url: &str) -> Result<Gallery> {
        let resp = send!(self.client.get(url))?;
        let mut html = parse_html(resp.text().await?)?;

        // 标题
        let title = html.xpath_text(r#"//h1[@id="gn"]/text()"#)?.swap_remove(0);
        let title_jp = html
            .xpath_text(r#"//h1[@id="gj"]/text()"#)
            .map(|mut n| n.swap_remove(0))
            .ok();

        // 父画廊
        let parent = html
            .xpath_text(r#"//tr[contains(./td[1]/text(), "Parent:")]/td[2]/a/@href"#)
            .ok()
            .map(|mut v| v.swap_remove(0));

        // 标签
        let mut tags = vec![];
        for ele in html
            .xpath_elem(r#"//div[@id="taglist"]//tr"#)
            .unwrap_or_default()
        {
            let tag_set_name = ele.xpath_text(r#"./td[1]/text()"#)?[0]
                .trim_matches(':')
                .to_owned();
            let tag = ele.xpath_text(r#"./td[2]/div/a/text()"#)?;
            tags.push((tag_set_name, tag));
        }

        // 图片列表
        let mut images = html.xpath_text(r#"//div[@id="gdt"]//a/@href"#)?;
        while let Ok(next_page) = html.xpath_text(r#"//table[@class="ptt"]//td[last()]/a/@href"#) {
            let resp = send!(self.client.get(&next_page[0]))?;
            html = parse_html(resp.text().await?)?;
            images.extend(html.xpath_text(r#"//div[@id="gdt"]//a/@href"#)?);
        }

        Ok(Gallery {
            title,
            title_jp,
            url: url.to_string(),
            parent,
            tags,
            images
        })
    }

    /// 根据图片页面的 URL 解析出真实的图片地址
    pub async fn image(&self, url: &str) -> Result<String> {
        let resp = send!(self.client.get(url))?;
        let url = parse_html(resp.text().await?)?
            .xpath_text(r#"//img[@id="img"]/@src"#)?
            .swap_remove(0);
        Ok(url)
    }
}
