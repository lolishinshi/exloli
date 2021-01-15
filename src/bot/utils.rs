use crate::database::Gallery;
use crate::{CONFIG, DB};
use teloxide::types::*;

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
