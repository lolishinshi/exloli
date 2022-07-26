use anyhow::Result;
use reqwest::header::*;
use reqwest::{Client, IntoUrl, RequestBuilder, Response};
use std::time::Duration;

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

    fn get(&self, path: &str) -> RequestBuilder {
        self.client.get(&format!("https://exhentai.org{}", path))
    }

    pub async fn search(&self, params: &[(&str, &str)]) {
        let mut page = 0;
        std::iter::from_fn(move || {
            let res = self
                .get("/")
                .query(params)
                .query(&[("page", &page.to_string())]);
            page += 1;
            Some(res)
        });
        // TODO: stream here?
    }
}
