use crate::exloli::ExLoli;
use crate::utils::MessageExt;
use crate::*;
use anyhow::Result;
use std::sync::Arc;
use teloxide::types::{ChatId, ChatOrInlineMessage};
use teloxide::utils::command::BotCommand;

macro_rules! send {
    ($e:expr) => {
        $e.send().await?
    };
}

macro_rules! unwrap {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => return Ok(()),
        }
    };
}

macro_rules! check_permission {
    ($e:expr) => {
        if !($e.is_from_admin() && $e.is_from_group()) {
            info!("权限检查失败");
            return ResponseResult::<()>::Ok(());
        }
    };
}

#[derive(BotCommand, PartialEq, Debug)]
#[command(rename = "lowercase", parse_with = "split")]
enum RuaCommand {
    Upload(String),
    Ban,
    Ping,
    Delete,
}

async fn kick_user(message: &UpdateWithCx<Message>) -> ResponseResult<()> {
    check_permission!(&message.update);
    match message.update.reply_to_message() {
        Some(reply) => {
            if let Some(user) = reply.from() {
                info!("踢出用户：{:?}", user);
                send!(message.bot.kick_chat_member(message.chat_id(), user.id));
                send!(message.reply_to(format!("已封禁用户 {}", user.id)));
            }
        }
        None => {
            send!(message.reply_to("请回复需要被操作的用户"));
        }
    }
    send!(message.delete_message());
    Ok(())
}

async fn send_pool(message: &UpdateWithCx<Message>) -> ResponseResult<Message> {
    info!("频道消息更新，发送投票");
    let options = vec![
        "★".into(),
        "★★".into(),
        "★★★".into(),
        "★★★★".into(),
        "★★★★★".into(),
    ];
    Ok(send!(message
        .bot
        .send_poll(message.update.chat.id, "你如何评价这本本子", options)
        .reply_to_message_id(message.update.id)))
}

pub async fn upload_gallery(
    message: &UpdateWithCx<Message>,
    url: &str,
    exloli: &ExLoli,
) -> ResponseResult<()> {
    check_permission!(&message.update);
    let mes = send!(message.reply_to("收到命令，上传中……"));
    if let Err(e) = exloli.upload_gallery_by_url(&url).await {
        if &*e.to_string() == "AlreadyUpload" {
            send!(message.reply_to("已上传"));
        }
        error!("上传出错：{}", e);
    }
    let to_send = ChatOrInlineMessage::Chat {
        chat_id: ChatId::Id(mes.chat.id),
        message_id: mes.id,
    };
    send!(message.bot.edit_message_text(to_send, "上传完毕"));
    Ok(())
}

async fn delete_gallery(message: &UpdateWithCx<Message>) -> ResponseResult<()> {
    check_permission!(&message.update);
    let to_del = match message.update.reply_to_message() {
        Some(v) => v,
        None => {
            send!(message.reply_to("请回复需要删除的画廊"));
            return Ok(());
        }
    };
    let channel = unwrap!(to_del.forward_from_chat());
    let mes_id = unwrap!(to_del.forward_from_message_id());
    send!(message.bot.delete_message(to_del.chat.id, to_del.id));
    send!(message.bot.delete_message(channel.id, *mes_id));
    Ok(())
}

pub async fn start_bot(exloli: Arc<ExLoli>) {
    info!("BOT 启动");
    teloxide::repl(BOT.clone(), move |message| {
        let exloli = exloli.clone();
        async move {
            debug!("{:#?}", message.update);
            if message.update.is_from_channel()
                && message
                    .update
                    .text()
                    .map(|s| s.contains("原始地址"))
                    .unwrap_or(false)
            {
                send_pool(&message).await?;
            }
            if let Some(command) = message.update.get_command() {
                info!("收到命令：{:?}", command);
                match command {
                    RuaCommand::Upload(url) => upload_gallery(&message, &url, &exloli).await?,
                    RuaCommand::Ban => kick_user(&message).await?,
                    RuaCommand::Delete => delete_gallery(&message).await?,
                    RuaCommand::Ping => {
                        send!(message.reply_to("pong"));
                    }
                }
            }
            ResponseResult::<()>::Ok(())
        }
    })
    .await;
}
