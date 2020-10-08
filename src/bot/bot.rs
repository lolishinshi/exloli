use super::utils::*;
use crate::exloli::ExLoli;
use crate::*;
use anyhow::Result;
use std::num::ParseIntError;
use std::sync::Arc;
use teloxide::types::*;
use teloxide::utils::command::BotCommand;

macro_rules! check_is_root {
    ($e:expr) => {
        if !(($e.update.is_from_root() && $e.update.is_from_group()) || $e.update.is_from_owner()) {
            info!("权限检查失败");
            send!($e.delete_message())?;
            return Ok(());
        }
    };
}

macro_rules! check_is_admin {
    ($e:expr) => {
        if !(($e.update.is_from_root() && $e.update.is_from_group())
            || $e.update.is_from_owner()
            || is_from_admin($e).await?)
        {
            info!("权限检查失败");
            send!($e.delete_message())?;
            return Ok(());
        }
    };
}

#[derive(PartialEq, Debug)]
struct Reason(Option<String>);

impl FromStr for Reason {
    // FIXME: 随便塞了个类型
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Reason(if s.is_empty() {
            None
        } else {
            Some(s.to_owned())
        }))
    }
}

#[derive(BotCommand, PartialEq, Debug)]
#[command(rename = "lowercase", parse_with = "split")]
enum RuaCommand {
    Upload(String),
    Ban(Reason),
    Ping,
    Delete,
    Warn(Reason),
}

async fn is_from_admin(message: &UpdateWithCx<Message>) -> Result<bool> {
    // TODO: 缓存
    let from = message.update.from().unwrap();
    let admins = send!(message.bot.get_chat_administrators(message.chat_id()))?;
    for admin in admins {
        if admin.user.id == from.id {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn ban_user(message: &UpdateWithCx<Message>, reason: Option<String>) -> Result<()> {
    check_is_admin!(&message);
    match message.update.reply_to_user() {
        Some(user) => {
            info!("封禁用户：{:?}", user);
            let mut text = format!("封禁用户： <a href=\"tg://user?id={0}\">{0}</a>", user.id);
            if let Some(reason) = reason {
                text.push_str(&format!("\n原因：{}", reason));
            }
            send!(message.bot.kick_chat_member(message.chat_id(), user.id))?;
            send!(message.reply_to(text))?;
        }
        None => {
            send!(message.reply_to("请回复需要被操作的用户"))?;
        }
    }
    let message_id = message.update.reply_to_message().unwrap().id;
    send!(message.bot.delete_message(message.chat_id(), message_id))?;
    send!(message.delete_message())?;
    Ok(())
}

async fn warn_user(message: &UpdateWithCx<Message>, reason: Option<String>) -> Result<()> {
    check_is_admin!(&message);
    match message.update.reply_to_user() {
        Some(user) => {
            info!("警告用户：{:?}", user);
            let warn = DB.add_warn(user.id)?;
            let mut text = String::new();
            text.push_str(&format!(
                "警告用户：<a href=\"tg://user?id={0}\">{0}</a>\n次数：{1}/3",
                user.id, warn
            ));
            if let Some(reason) = reason {
                text.push_str(&format!("\n原因：{}", reason));
            }
            if warn == 3 {
                text.push_str(&format!("\n警告次数达到上限，已封禁"));
                send!(message.bot.kick_chat_member(message.chat_id(), user.id))?;
            }
            send!(message.reply_to(text))?;
        }
        None => {
            send!(message.reply_to("请回复需要被操作的用户"))?;
        }
    }
    let message_id = message.update.reply_to_message().unwrap().id;
    send!(message.bot.delete_message(message.chat_id(), message_id))?;
    send!(message.delete_message())?;
    Ok(())
}

async fn send_pool(message: &UpdateWithCx<Message>) -> Result<()> {
    info!("频道消息更新，发送投票");
    let options = vec![
        "★".into(),
        "★★".into(),
        "★★★".into(),
        "★★★★".into(),
        "★★★★★".into(),
    ];
    let poll = send!(message
        .bot
        .send_poll(message.update.chat.id, "你如何评价这本本子", options)
        .reply_to_message_id(message.update.id))?;
    let poll_id = poll.poll().unwrap().id.to_owned();
    let message_id = *message.update.forward_from_message_id().unwrap();
    debug!("投票：{} {}", message_id, poll_id);
    DB.update_poll_id(message_id, &poll_id)
}

pub async fn upload_gallery(
    message: &UpdateWithCx<Message>,
    url: &str,
    exloli: &ExLoli,
) -> ResponseResult<()> {
    check_is_root!(&message);
    let mes = send!(message.reply_to("收到命令，上传中……"))?;
    if let Err(e) = exloli.upload_gallery_by_url(&url).await {
        if &*e.to_string() == "AlreadyUpload" {
            send!(message.reply_to("已上传"))?;
        }
        error!("上传出错：{}", e);
    }
    let to_send = ChatOrInlineMessage::Chat {
        chat_id: ChatId::Id(mes.chat.id),
        message_id: mes.id,
    };
    send!(message.bot.edit_message_text(to_send, "上传完毕"))?;
    Ok(())
}

async fn delete_gallery(message: &UpdateWithCx<Message>) -> Result<()> {
    check_is_root!(&message);
    let to_del = match message.update.reply_to_message() {
        Some(v) => v,
        None => {
            send!(message.reply_to("请回复需要删除的画廊"))?;
            return Ok(());
        }
    };
    let channel = unwrap!(to_del.forward_from_chat());
    let mes_id = unwrap!(to_del.forward_from_message_id());
    send!(message.bot.delete_message(to_del.chat.id, to_del.id))?;
    send!(message.bot.delete_message(channel.id, *mes_id))?;
    Ok(())
}

async fn message_handler(exloli: Arc<ExLoli>, message: UpdateWithCx<Message>) -> Result<()> {
    use RuaCommand::*;
    debug!("{:#?}", message.update);
    if message.update.is_from_channel()
        && message
            .update
            .text()
            .map(|s| s.contains("原始地址"))
            .unwrap_or(false)
    {
        send_pool(&message).await.log_on_error().await;
    }
    if let Some(command) = message.update.get_command() {
        info!("收到命令：{:?}", command);
        match command {
            Upload(url) => upload_gallery(&message, &url, &exloli).await?,
            Ban(reason) => ban_user(&message, reason.0).await?,
            Delete => delete_gallery(&message).await?,
            Ping => {
                send!(message.reply_to("pong"))?;
            }
            Warn(reason) => warn_user(&message, reason.0).await?,
        }
    }
    Ok(())
}

async fn poll_handler(poll: UpdateWithCx<Poll>) -> Result<()> {
    let options = poll.update.options;
    let man_cnt = options.iter().map(|s| s.voter_count).sum::<i32>() as f32;
    let score = options
        .iter()
        .enumerate()
        .map(|(i, s)| (i as i32 + 1) * s.voter_count)
        .sum::<i32>() as f32;
    let score = score / man_cnt;
    info!("投票状态变动：{} -> {}", poll.update.id, score);
    DB.update_score(&poll.update.id, score)
}

pub async fn start_bot(exloli: Arc<ExLoli>) {
    info!("BOT 启动");
    Dispatcher::new(BOT.clone())
        .messages_handler(move |rx: DispatcherHandlerRx<Message>| {
            rx.for_each_concurrent(4, move |message| {
                let exloli = exloli.clone();
                async move {
                    message_handler(exloli, message).await.log_on_error().await;
                }
            })
        })
        .polls_handler(|rx: DispatcherHandlerRx<Poll>| {
            rx.for_each_concurrent(4, |message| async move {
                poll_handler(message).await.log_on_error().await;
            })
        })
        .dispatch()
        .await;
}
