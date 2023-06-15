use crate::bot::utils::*;
use crate::database::Gallery;
use crate::exhentai::EXHENTAI;
use crate::*;
use futures::TryFutureExt;
use std::convert::TryInto;
use std::fmt::{self, Debug, Formatter};
use std::str::FromStr;
use teloxide::types::Message;

pub enum CommandError {
    /// 命令解析错误
    WrongCommand(&'static str),
    /// 不是自己的命令
    NotACommand,
}

#[derive(PartialEq)]
pub enum InputGallery {
    ExHentaiUrl(String),
    Gallery(Gallery),
}

impl InputGallery {
    /// 转换为 `Gallery`，会自动请求历史画廊
    pub async fn to_gallery(&self) -> anyhow::Result<Gallery> {
        match &self {
            Self::Gallery(g) => Ok(g.clone()),
            Self::ExHentaiUrl(s) => match DB.query_gallery_by_url(s) {
                Err(_) => {
                    let gallery = EXHENTAI
                        .get_gallery_by_url(s)
                        .and_then(|g| g.into_full_info())
                        .await?;
                    Ok(ExLoli::get_history_upload(&gallery).await?)
                }
                v => v,
            },
        }
    }
}

impl Debug for InputGallery {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExHentaiUrl(s) => f.debug_tuple("Url").field(&s).finish(),
            Self::Gallery(g) => f.debug_tuple("Message").field(&g.message_id).finish(),
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum RuaCommand {
    // 上传指定画廊
    Upload(Vec<String>),
    // 查询指定画廊
    Query(Vec<InputGallery>),
    // Ping bot
    Ping,
    // 用该命令回复一条画廊以将其删除
    Delete,
    // 这是真的删除，彻底删除
    RealDelete,
    // 按评分高低查询一段时间内的本子，格式 /best 最少几天前 最多几天前 多少本
    Best([i64; 2]),
    // 用该命令回复一条画廊以上传其完整版本
    Full(Vec<InputGallery>),
    // 用该命令回复一条画廊以重新上传
    ReUpload(Vec<InputGallery>),
    // 更新 tag
    UpdateTag(Vec<InputGallery>),
}

impl RuaCommand {
    /// 将消息解析为命令
    pub async fn parse(bot: Bot, message: &Message, bot_id: &str) -> Result<Self, CommandError> {
        use CommandError::*;

        let text = message.text().unwrap_or("");

        if !text.starts_with('/') {
            return Err(NotACommand);
        }

        let (cmd, args) = text
            .split_once(|c| c == ' ' || c == '\n')
            .unwrap_or((text, ""));
        let (cmd, bot_name) = cmd.split_once('@').unwrap_or((cmd, ""));
        // remove leading `/`
        let cmd = &cmd[1..];

        if !bot_name.is_empty() && bot_name != bot_id {
            return Err(NotACommand);
        }

        debug!("收到命令：/{} {}", cmd, args);

        let (is_admin, trusted) = check_is_channel_admin(bot, message).await;

        match (cmd, is_admin, trusted) {
            ("ping", _, _) => Ok(Self::Ping),
            ("full", _, true) => {
                let arg = get_input_gallery(message, args);
                match arg.is_empty() {
                    false => Ok(Self::Full(arg)),
                    true => Err(WrongCommand("用法：/full [回复|画廊地址|消息地址]...")),
                }
            }
            ("uptag", _, true) => {
                let arg = get_input_gallery(message, args);
                match arg.is_empty() {
                    false => Ok(Self::UpdateTag(arg)),
                    true => Err(WrongCommand("用法：/uptag [回复|画廊地址|消息地址]...")),
                }
            }
            ("reupload", _, _) => {
                let arg = get_input_gallery(message, args);
                match arg.is_empty() {
                    false => Ok(Self::ReUpload(arg)),
                    true => Err(WrongCommand("用法：/reupload [回复|消息地址]...")),
                }
            }
            ("delete", true, _) => {
                if message.reply_to_gallery().is_none() {
                    return Err(WrongCommand("用法：请回复一个需要删除的画廊"));
                }
                Ok(Self::Delete)
            }
            ("real_delete", true, _) => {
                if message.reply_to_gallery().is_none() {
                    return Err(WrongCommand("用法：请回复一个需要彻底删除的画廊"));
                }
                Ok(Self::RealDelete)
            }
            ("upload", _, true) => {
                let urls = get_exhentai_urls(message.text().unwrap_or_default());
                if urls.is_empty() {
                    Err(WrongCommand("用法：/upload 画廊地址..."))
                } else {
                    Ok(Self::Upload(urls))
                }
            }
            ("best", _, _) => match parse_command_best(args) {
                Some(mut v) => {
                    v[0] = v[0].min(3650);
                    v[1] = v[1].min(3650);
                    Ok(RuaCommand::Best(v))
                }
                _ => Err(WrongCommand("用法：/best 起始时间 终止时间")),
            },
            ("query", _, _) => {
                let arg = get_input_gallery(message, args);
                match arg.is_empty() {
                    false => Ok(Self::Query(arg)),
                    true => Err(WrongCommand("用法：/query [回复|画廊地址|消息地址]...")),
                }
            }
            _ => {
                if bot_name == bot_id {
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

fn get_input_gallery(message: &Message, s: &str) -> Vec<InputGallery> {
    let i1 = MESSAGE_URL.captures_iter(s).filter_map(|c| {
        c.get(1)
            .and_then(|s| s.as_str().parse::<i32>().ok())
            .and_then(|n| DB.query_gallery(n).ok())
            .map(InputGallery::Gallery)
    });
    let i2 = EXHENTAI_URL.captures_iter(s).filter_map(|c| {
        c.get(0)
            .map(|s| InputGallery::ExHentaiUrl(s.as_str().to_owned()))
    });
    let mut ret = i1.chain(i2).collect::<Vec<_>>();
    if let (true, Some(g)) = (ret.is_empty(), message.reply_to_gallery()) {
        ret.push(InputGallery::Gallery(g));
    }
    ret
}
