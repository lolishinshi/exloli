use crate::exloli::ExLoli;
use crate::*;
use std::sync::Arc;
use teloxide::types::ChatId;
use teloxide::types::*;

fn is_from_channel(message: &UpdateWithCx<Message>) -> bool {
    if let MessageKind::Common(MessageCommon {
        from: Some(user),
        forward_kind: ForwardKind::Channel(ForwardChannel { chat, .. }),
        ..
    }) = &message.update.kind
    {
        return match &CONFIG.telegram.channel_id {
            ChatId::Id(id) => user.id == 777000 && chat.id == *id,
            ChatId::ChannelUsername(name) => {
                if let ChatKind::Public(ChatPublic {
                    kind:
                        PublicChatKind::Channel(PublicChatChannel {
                            username: Some(text),
                            ..
                        }),
                    ..
                }) = &chat.kind
                {
                    &name[1..] == text
                } else {
                    false
                }
            }
            _ => false,
        };
    }
    false
}

fn get_admin_command(message: &UpdateWithCx<Message>) -> Option<String> {
    if let Message {
        chat,
        kind:
            MessageKind::Common(MessageCommon {
                from: Some(user),
                media_kind: MediaKind::Text(text),
                ..
            }),
        ..
    } = &message.update
    {
        if user.username.as_ref() == Some(&CONFIG.telegram.owner) && text.text.starts_with('/') {
            return Some(text.text.to_owned());
        }
        if let ChatId::Id(id) = CONFIG.telegram.group_id {
            if chat.id == id
                && user.username == Some("GroupAnonymousBot".into())
                && text.text.starts_with('/')
            {
                return Some(text.text.to_owned());
            }
        }
    }
    None
}

pub async fn start_bot(exloli: Arc<ExLoli>) {
    info!("BOT 启动");
    teloxide::repl(BOT.clone(), move |message| {
        let exloli = exloli.clone();
        async move {
            debug!("{:#?}", message.update);
            if is_from_channel(&message) {
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
                    .await?;
            }
            if let Some(command) = get_admin_command(&message) {
                info!("收到命令：{}", command);
                let command = command.split_ascii_whitespace().collect::<Vec<_>>();
                if command.len() == 2 && command[0] == "/upload" {
                    message.reply_to("收到命令，上传中……").send().await?;
                    if let Err(e) = exloli.upload_gallery_by_url(command[1]).await {
                        error!("{}", e);
                    }
                }
            }
            ResponseResult::<()>::Ok(())
        }
    })
    .await;
}
