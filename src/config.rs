use anyhow::Error;
use reqwest::{Client, Proxy};
use serde::Deserialize;
use std::time::Duration;
use std::{fs::File, io::Read, path::Path};
use teloxide::types::{ChatId, Recipient};
use url::Url;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_level: String,
    pub threads_num: usize,
    pub interval: u64,
    pub database_url: String,
    pub exhentai: ExHentai,
    pub telegraph: Telegraph,
    pub telegram: Telegram,
}

#[derive(Debug, Deserialize)]
pub struct ExHentai {
    pub cookie: Option<String>,
    pub search_params: Vec<(String, String)>,
    pub search_pages: i32,
    pub outdate: i64,
}

#[derive(Debug, Deserialize)]
pub struct Telegraph {
    pub access_token: String,
    pub author_name: String,
    pub author_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Telegram {
    pub channel_id: Recipient,
    pub bot_id: String,
    pub token: String,
    pub group_id: ChatId,
}

impl Config {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let mut file = File::open(path)?;
        let mut str = String::new();
        file.read_to_string(&mut str)?;
        Ok(toml::from_str(&str)?)
    }

    pub async fn init_telegraph(&self) -> Result<telegraph_rs::Telegraph, Error> {
        let telegraph = &self.telegraph;
        let mut client_builder = Client::builder().timeout(Duration::from_secs(30));
        if let Some(proxy) = &self.telegraph.proxy {
            client_builder = client_builder.proxy(Proxy::all(proxy)?);
        }
        let client = client_builder.build()?;
        Ok(telegraph_rs::Telegraph::new(&telegraph.author_name)
            .author_url(&telegraph.author_url)
            .access_token(&telegraph.access_token)
            .client(client)
            .create()
            .await?)
    }

    pub async fn init_exhentai(&self) -> Result<crate::exhentai::ExHentai, Error> {
        if self.exhentai.cookie.is_some() {
            crate::exhentai::ExHentai::from_cookie().await
        } else {
            crate::exhentai::ExHentai::new().await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    #[test]
    fn test() {
        let config = Config::new("config.toml");
        println!("{:?}", config);
    }
}
