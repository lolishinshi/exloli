use failure::Error;
use serde::Deserialize;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_level: Option<String>,
    pub threads_num: Option<String>,
    pub exhentai: ExHentai,
    pub telegraph: Telegraph,
    pub telegram: Telegram,
}

#[derive(Debug, Deserialize)]
pub struct ExHentai {
    pub username: String,
    pub password: String,
    pub keyword: String,
    pub search_watched: bool,
    pub max_img_cnt: usize,
    pub local_cache: bool,
    pub cache_path: String,
}

#[derive(Debug, Deserialize)]
pub struct Telegraph {
    pub upload: bool,
    pub access_token: String,
    pub author_name: String,
    pub author_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Telegram {
    pub channel_id: String,
    pub token: String,
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
        let exhentai = &self.exhentai;
        Ok(crate::exhentai::ExHentai::new(
            &exhentai.username,
            &exhentai.password,
            exhentai.search_watched,
        )
        .await?)
    }

    pub fn init_telegram(&self) -> crate::telegram::Bot {
        crate::telegram::Bot::new(&self.telegram.token)
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
