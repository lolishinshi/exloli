use crate::database::Gallery;
use crate::{BOT, CONFIG, DB};
use cached::proc_macro::cached;
use teloxide::prelude::*;
use teloxide::types::*;
use tokio::task::block_in_place;

#[macro_export]
macro_rules! send {
    ($e:expr) => {
        $e.send().await
    };
}

pub trait MessageExt {
    fn is_from_my_group(&self) -> bool;
    fn from_username(&self) -> Option<&String>;
    fn reply_to_user(&self) -> Option<&User>;
    fn reply_to_gallery(&self) -> Option<Gallery>;
    fn to_chat_or_inline_message(&self) -> ChatOrInlineMessage;
}

impl MessageExt for Message {
    // 判断消息来源是否是指定群组
    fn is_from_my_group(&self) -> bool {
        match CONFIG.telegram.group_id {
            ChatId::Id(id) => self.chat.id == id,
            _ => todo!(),
        }
    }

    fn from_username(&self) -> Option<&String> {
        if let Some(User { username, .. }) = self.from() {
            return username.as_ref();
        }
        None
    }

    fn reply_to_user(&self) -> Option<&User> {
        if let Some(reply) = self.reply_to_message() {
            return reply.from();
        }
        None
    }

    fn reply_to_gallery(&self) -> Option<Gallery> {
        self.reply_to_message()
            .and_then(|message| message.forward_from_message_id())
            .and_then(|mess_id| DB.query_gallery_by_message_id(*mess_id).ok())
    }

    fn to_chat_or_inline_message(&self) -> ChatOrInlineMessage {
        ChatOrInlineMessage::Chat {
            chat_id: ChatId::Id(self.chat.id),
            message_id: self.id,
        }
    }
}

/// 获取管理员列表，提供 1 个小时的缓存
#[cached(time = 3600)]
async fn get_admins() -> Vec<User> {
    let mut admins =
        send!(BOT.get_chat_administrators(CONFIG.telegram.channel_id.clone())).unwrap_or_default();
    admins.extend(
        send!(BOT.get_chat_administrators(CONFIG.telegram.group_id.clone())).unwrap_or_default(),
    );
    admins.into_iter().map(|member| member.user).collect()
}

// 检测是否是指定频道的管理员
pub fn check_is_channel_admin(message: &UpdateWithCx<Message>) -> bool {
    // 先检测是否为匿名管理员
    let from_user = message.update.from();
    if from_user
        .map(|u| u.username == Some("GroupAnonymousBot".into()))
        .unwrap_or(false)
        && message.update.is_from_my_group()
    {
        return true;
    }
    let admins = block_in_place(|| futures::executor::block_on(get_admins()));
    message
        .update
        .from()
        .map(|user| admins.iter().map(|admin| admin == user).any(|x| x))
        .unwrap_or(false)
}
