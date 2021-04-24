use crate::bot::utils::*;
use crate::database::Gallery;
use crate::*;
use std::convert::TryInto;
use std::str::FromStr;
use teloxide::prelude::UpdateWithCx;
use teloxide::types::Message;

pub enum CommandError {
    /// 命令解析错误
    WrongCommand(&'static str),
    /// 不是自己的命令
    NotACommand,
}

#[derive(PartialEq, Debug)]
pub enum RuaCommand {
    // 上传指定画廊
    Upload(Vec<String>),
    // 查询指定画廊
    Query(Vec<String>),
    // Ping bot
    Ping,
    // 用该命令回复一条画廊以将其删除
    Delete,
    // 这是真的删除，彻底删除
    RealDelete,
    // 按评分高低查询一段时间内的本子，格式 /best 最少几天前 最多几天前 多少本
    Best([i64; 2]),
    // 用该命令回复一条画廊以上传其完整版本
    Full(Vec<Gallery>),
    // 更新 tag
    UpdateTag(Vec<Gallery>),
    // 查询画廊信息
    Info(Gallery),
}

impl RuaCommand {
    /// 将消息解析为命令
    pub fn parse(message: &UpdateWithCx<Bot, Message>, bot_id: &str) -> Result<Self, CommandError> {
        use CommandError::*;

        let text = message.update.text().unwrap_or("");

        if !text.starts_with('/') {
            return Err(NotACommand);
        }

        // TODO: split_once
        let (cmd, args) = match text.find(|c| c == ' ' || c == '\n') {
            Some(pos) => (&text[1..pos], text[pos + 1..].trim()),
            _ => (&text[1..], ""),
        };
        let (cmd, bot) = match cmd.find('@') {
            Some(pos) => (&cmd[..pos], &cmd[pos + 1..]),
            None => (cmd, ""),
        };

        if !bot.is_empty() && bot != bot_id {
            return Err(NotACommand);
        }

        info!("收到命令：/{} {}", cmd, args);

        let is_admin = check_is_channel_admin(message);

        match (cmd, is_admin) {
            ("ping", _) => Ok(Self::Ping),
            ("full", true) => {
                let mut arg = get_galleries(args);
                if let Some(g) = message.update.reply_to_gallery() {
                    arg.push(g);
                }
                match arg.is_empty() {
                    false => Ok(Self::Full(arg)),
                    true => Err(WrongCommand("用法：请指定需要上传的画廊")),
                }
            }
            ("uptag", true) => {
                let mut arg = get_galleries(args);
                if let Some(g) = message.update.reply_to_gallery() {
                    arg.push(g);
                }
                match arg.is_empty() {
                    false => Ok(Self::UpdateTag(arg)),
                    true => Err(WrongCommand("用法：请指定需要更新的画廊")),
                }
            }
            ("delete", true) => {
                if message.update.reply_to_gallery().is_none() {
                    return Err(WrongCommand("用法：请回复一个需要删除的画廊"));
                }
                Ok(Self::Delete)
            }
            ("real_delete", true) => {
                if message.update.reply_to_gallery().is_none() {
                    return Err(WrongCommand("用法：请回复一个需要彻底删除的画廊"));
                }
                Ok(Self::RealDelete)
            }
            ("upload", true) => {
                let urls = get_exhentai_urls(message.update.text().unwrap_or_default());
                if urls.is_empty() {
                    Err(WrongCommand("用法：/upload 画廊地址..."))
                } else {
                    Ok(Self::Upload(urls))
                }
            }
            ("info", _) => {
                if let Some(g) = message.update.reply_to_gallery() {
                    return Ok(Self::Info(g));
                }
                let mut gallery = get_galleries(args);
                if gallery.is_empty() {
                    return Err(WrongCommand("用法：请回复一个需要查询的画廊"));
                }
                return Ok(Self::Info(gallery.swap_remove(0)));
            }
            ("best", _) => match parse_command_best(args) {
                Some(mut v) => {
                    v[0] = v[0].min(3650);
                    v[1] = v[1].min(3650);
                    Ok(RuaCommand::Best(v))
                }
                _ => Err(WrongCommand("用法：/best 起始时间 终止时间")),
            },
            ("query", _) => {
                let urls = get_exhentai_urls(message.update.text().unwrap_or_default());
                if urls.is_empty() {
                    Err(WrongCommand("用法：/query 画廊地址..."))
                } else {
                    Ok(Self::Query(urls))
                }
            }
            _ => {
                if bot == bot_id {
                    Err(WrongCommand(""))
                } else {
                    Err(NotACommand)
                }
            }
        }
    }
}

/// 将字符串解析为三个数字
fn parse_command_best(input: &str) -> Option<[i64; 2]> {
    let v = input
        .split_ascii_whitespace()
        .map(i64::from_str)
        .collect::<Result<Vec<_>, _>>()
        .ok();
    if let Some(v) = v.and_then(|v| TryInto::<[i64; 2]>::try_into(v).ok()) {
        return Some(v);
    }
    None
}

/// 提取字符串中的 e 站地址
fn get_exhentai_urls(s: &str) -> Vec<String> {
    EXHENTAI_URL
        .captures_iter(s)
        .filter_map(|c| c.get(0).map(|m| m.as_str().to_owned()))
        .collect::<Vec<_>>()
}

fn get_galleries(s: &str) -> Vec<Gallery> {
    s.split_ascii_whitespace()
        .filter_map(|url| {
            url.split('/')
                .last()
                .and_then(|s| s.parse::<i32>().ok())
                .and_then(|id| DB.query_gallery_by_message_id(id).ok())
        })
        .collect()
}
