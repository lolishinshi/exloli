use failure::Error;
use serde::Deserialize;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub exhentai: ExHentai,
    pub telegraph: Telegraph,
    pub telegram: Telegram,
}

#[derive(Debug, Deserialize)]
pub struct ExHentai {
    pub username: String,
    pub password: String,
    pub keyword: String,
}

#[derive(Debug, Deserialize)]
pub struct Telegraph {
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
