#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use crate::config::Config;
use crate::database::DataBase;
use crate::exloli::ExLoli;

use anyhow::Error;
use lazy_static::lazy_static;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use tokio::time::delay_for;

use std::env;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time;

mod bot;
mod config;
mod database;
mod exhentai;
mod exloli;
mod schema;
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
    pub static ref DB: DataBase = DataBase::init();
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
    env::set_var("DATABASE_URL", &CONFIG.database_url);

    if let Err(e) = run().await {
        error!("{}", e);
    }
}

async fn run() -> Result<(), Error> {
    let args = env::args().collect::<Vec<_>>();
    let mut opts = getopts::Options::new();
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
    }

    let db_path = env::var("DATABASE_URL").expect("请设置 DATABASE_URL");
    if !Path::new(&db_path).exists() {
        info!("初始化数据库");
        DB.init_database()?;
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
