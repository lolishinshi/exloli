use crate::bot::utils::*;
use crate::database::Gallery;
use crate::exhentai::EXHENTAI;
use crate::*;
use futures::TryFutureExt;
use std::convert::TryInto;
use std::fmt::{self, Debug, Formatter};
use std::str::FromStr;
use teloxide::types::Message;
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, PartialEq, Debug)]
#[command(rename = "snake_case", parse_with = "split")]
pub enum PublicCommand {
    #[command(description = "根据 URL 查询指定画廊")]
    Query(String),
    #[command(description = "ping")]
    Ping,
    #[command(description = "查询从 $1 ~ $2 天之间的本子")]
    Best(i64, i64),
    #[command(description = "上传所回复画廊的完整版本")]
    Full(Option<String>),
    #[command(description = "更新所回复画廊 tag")]
    UpdateTag(Option<String>),
}

#[derive(BotCommands, PartialEq, Debug)]
#[command(rename = "snake_case", parse_with = "split")]
pub enum PrivilegedCommand {
    #[command(description = "根据 URL 上传指定画廊")]
    Upload(String),
    #[command(description = "从频道中删除所回复的画廊")]
    Delete,
    #[command(description = "从频道和数据库中删除所回复的画廊")]
    RealDelete,
}
