#![type_length_limit = "9049659"]

use crate::config::Config;
use crate::exloli::ExLoli;

use anyhow::Error;
use lazy_static::lazy_static;
use log::{debug, error, info};
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use tokio::time::delay_for;

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use std::time;

mod bot;
mod config;
mod exhentai;
mod exloli;
mod trans;
mod xpath;

lazy_static! {
    static ref CONFIG: Config = Config::new("config.toml").unwrap_or_else(|e| {
        eprintln!("配置文件解析失败:\n{}", e);
        std::process::exit(1);
    });
    pub static ref BOT: Bot = teloxide::BotBuilder::new()
        .token(&CONFIG.telegram.token)
        .parse_mode(ParseMode::HTML)
        .build();
    pub static ref DB: sled::Db = sled::open("./db").expect("无法打开数据库");
}

fn dump_db() -> Result<(), Error> {
    let mut map = HashMap::new();
    for i in DB.iter() {
        let (k, v) = i?;
        let k = String::from_utf8(k.to_vec()).unwrap_or_default();
        let v = String::from_utf8(v.to_vec()).unwrap_or_default();
        map.insert(k, v);
    }
    let string = serde_json::to_string_pretty(&map)?;
    println!("{}", string);
    Ok(())
}

fn load_db(file: &str) -> Result<(), Error> {
    let file = File::open(file)?;
    let map: HashMap<String, String> = serde_json::from_reader(file)?;
    for (k, v) in map.iter() {
        DB.insert(k.as_bytes(), v.as_bytes())?;
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_builder()
        .write_style(pretty_env_logger::env_logger::WriteStyle::Auto)
        .filter(Some("teloxide"), log::LevelFilter::Error)
        .filter(
            Some("exloli"),
            log::LevelFilter::from_str(&CONFIG.log_level).expect("LOG 等级设置错误"),
        )
        .init();

    let args = env::args().collect::<Vec<_>>();
    if args.len() == 1 {
        loop {
            if let Err(e) = run().await {
                error!("{}", e);
            }
            delay_for(time::Duration::from_secs(60)).await;
        }
    }

    let result = match (args.len(), args.get(1).map(String::as_str)) {
        (2, Some("dump")) => dump_db(),
        (3, Some("load")) => load_db(&args[2]),
        _ => Ok(()),
    };
    if let Err(e) = result {
        error!("{}", e);
    }
}

async fn run() -> Result<(), Error> {
    let exloli = Arc::new(ExLoli::new().await?);

    {
        let exloli = exloli.clone();
        tokio::spawn(async move { crate::bot::start_bot(exloli).await });
    }

    loop {
        let result = exloli.scan_and_upload().await;
        if let Err(e) = result {
            error!("定时更新出错：{}", e);
        } else {
            info!("定时更新完成");
        }
        info!("休眠中，预计 {} 分钟后开始工作", CONFIG.interval / 60);
        delay_for(time::Duration::from_secs(CONFIG.interval)).await;
    }
}
