use crate::exloli::ExLoli;
use crate::utils::MessageExt;
use crate::*;
use anyhow::Result;
use std::sync::Arc;
use teloxide::utils::command::BotCommand;

#[derive(BotCommand, PartialEq, Debug)]
#[command(rename = "lowercase", parse_with = "split")]
enum RuaCommand {
    Upload(String),
    Ban,
}

async fn kick_user(message: &UpdateWithCx<Message>) -> ResponseResult<()> {
    match message.update.reply_to_message() {
        Some(reply) => {
            if let Some(user) = reply.from() {
                info!("踢出用户：{:?}", user);
                message
                    .bot
                    .kick_chat_member(message.chat_id(), user.id)
                    .send()
                    .await?;
            }
            Ok(())
        }
        None => Ok(()),
    }
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
    message
        .bot
        .send_poll(message.update.chat.id, "你如何评价这本本子", options)
        .reply_to_message_id(message.update.id)
        .send()
        .await
}

pub async fn start_bot(exloli: Arc<ExLoli>) {
    info!("BOT 启动");
    teloxide::repl(BOT.clone(), move |message| {
        let exloli = exloli.clone();
        async move {
            debug!("{:#?}", message.update);
            if message.update.is_from_channel() {
                send_pool(&message).await?;
            }
            if let Some(command) = message.update.get_command() {
                if message.update.is_from_admin() {
                    return ResponseResult::<()>::Ok(());
                }
                info!("收到命令：{:?}", command);
                match command {
                    RuaCommand::Upload(url) => {
                        message.reply_to("收到命令，上传中……").send().await?;
                        if let Err(e) = exloli.upload_gallery_by_url(&url).await {
                            error!("上传出错：{}", e);
                        }
                    }
                    RuaCommand::Ban => kick_user(&message).await?,
                }
            }
            ResponseResult::<()>::Ok(())
        }
    })
    .await;
}
