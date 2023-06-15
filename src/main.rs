#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate anyhow;

use crate::config::Config;
use crate::database::DataBase;
use crate::exloli::ExLoli;

use anyhow::Error;
use futures::executor::block_on;
use once_cell::sync::Lazy;
use teloxide::prelude::*;
use tokio::time::sleep;

use std::env;
use std::str::FromStr;
use std::time;

mod bot;
mod config;
mod database;
mod ehentai;
mod exhentai;
mod exloli;
mod models;
mod schema;
mod trans;
mod utils;
mod xpath;

static CONFIG: Lazy<Config> = Lazy::new(|| {
    let config_file = std::env::var("EXLOLI_CONFIG");
    let config_file = config_file.as_deref().unwrap_or("config.toml");
    Config::new(config_file).expect("配置文件解析失败")
});
static BOT: Lazy<Bot> = Lazy::new(|| teloxide::Bot::new(&CONFIG.telegram.token));
static DB: Lazy<DataBase> = Lazy::new(|| DataBase::init().expect("数据库初始化失败"));
static EXLOLI: Lazy<ExLoli> = Lazy::new(|| block_on(ExLoli::new()).expect("登录失败"));

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp_secs()
        .write_style(env_logger::WriteStyle::Auto)
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

fn init_args() -> getopts::Matches {
    let args = env::args().collect::<Vec<_>>();
    let mut opts = getopts::Options::new();
    opts.optflag("", "debug", "调试模式，不自动爬本");
    opts.optflag("h", "help", "打印帮助");
    let matches = match opts.parse(&args[1..]) {
        Ok(v) => v,
        Err(e) => panic!("{}", e),
    };
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options]", args[0]);
        print!("{}", opts.usage(&brief));
        std::process::exit(0);
    }
    matches
}

async fn run() -> Result<(), Error> {
    let matches = init_args();

    env::var("DATABASE_URL").expect("请设置 DATABASE_URL");

    let debug_mode = matches.opt_present("debug");

    tokio::spawn(async move {
        sleep(time::Duration::from_secs(10)).await;
        bot::start_bot(BOT.clone()).await
    });

    loop {
        if !debug_mode {
            info!("定时更新开始");
            let result = EXLOLI.scan_and_upload().await;
            if let Err(e) = result {
                error!("定时更新出错：{}", e);
            } else {
                info!("定时更新完成");
            }
        }
        info!("休眠中，预计 {} 分钟后开始工作", CONFIG.interval / 60);
        sleep(time::Duration::from_secs(CONFIG.interval)).await;
    }
}
