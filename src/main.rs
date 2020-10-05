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
mod utils;
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

    if let Err(e) = run().await {
        error!("{}", e);
    }
}

async fn run() -> Result<(), Error> {
    let args = env::args().collect::<Vec<_>>();
    let mut opts = getopts::Options::new();
    opts.optflag("", "dump", "导出数据库");
    opts.optopt("", "load", "导入数据库", "PATH");
    opts.optflag("", "debug", "调试模式");
    opts.optflag("h", "help", "print this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(v) => v,
        Err(e) => panic!("{}", e),
    };

    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options]", args[0]);
        print!("{}", opts.usage(&brief));
        return Ok(());
    } else if matches.opt_present("dump") {
        info!("导出数据库");
        return dump_db();
    } else if let Some(name) = matches.opt_str("load") {
        info!("导入数据库");
        return load_db(&name);
    }

    let debug = matches.opt_present("debug");
    let exloli = Arc::new(ExLoli::new().await?);

    {
        let exloli = exloli.clone();
        tokio::spawn(async move { crate::bot::start_bot(exloli).await });
    }

    loop {
        if !debug {
            let result = exloli.scan_and_upload().await;
            if let Err(e) = result {
                error!("定时更新出错：{}", e);
            } else {
                info!("定时更新完成");
            }
        }
        info!("休眠中，预计 {} 分钟后开始工作", CONFIG.interval / 60);
        delay_for(time::Duration::from_secs(CONFIG.interval)).await;
    }
}
