use anyhow::Error;
use serde::Deserialize;
use std::{fs::File, io::Read, path::Path};
use teloxide::types::ChatId;

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
    pub username: String,
    pub password: String,
    pub cookie: Option<String>,
    pub search_url: String,
    pub search_params: Vec<(String, String)>,
    pub max_pages: i32,
    pub max_img_cnt: usize,
    pub outdate: Option<i64>,
    pub proxy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Telegraph {
    pub access_token: String,
    pub author_name: String,
    pub author_url: String,
    pub proxy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Telegram {
    pub channel_id: ChatId,
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
        Ok(telegraph_rs::Telegraph::new(&telegraph.author_name)
            .author_url(&telegraph.author_url)
            .access_token(&telegraph.access_token)
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
