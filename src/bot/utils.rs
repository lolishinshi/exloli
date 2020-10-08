use crate::CONFIG;
use teloxide::types::*;
use teloxide::utils::command::BotCommand;

#[macro_export]
macro_rules! send {
    ($e:expr) => {
        $e.send().await
    };
}

#[macro_export]
macro_rules! unwrap {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => return Ok(()),
        }
    };
}

pub trait MessageExt {
    fn is_from_root(&self) -> bool;
    fn is_from_owner(&self) -> bool;
    fn is_from_channel(&self) -> bool;
    fn is_from_group(&self) -> bool;
    fn get_command<T: BotCommand>(&self) -> Option<T>;
    fn from_username(&self) -> Option<&String>;
    fn reply_to_user(&self) -> Option<&User>;
}

fn get_channel_name(chat: &Chat) -> Option<&str> {
    match &chat.kind {
        ChatKind::Public(ChatPublic {
            kind:
                PublicChatKind::Channel(PublicChatChannel {
                    username: Some(text),
                    ..
                }),
            ..
        }) => Some(text),
        _ => None,
    }
}

impl MessageExt for Message {
    fn is_from_root(&self) -> bool {
        if let Some(user) = self.from() {
            user.username.as_ref() == Some(&CONFIG.telegram.owner)
                || user.username == Some("GroupAnonymousBot".into())
        } else {
            false
        }
    }

    fn is_from_owner(&self) -> bool {
        self.from()
            .map(|u| u.username.as_ref() == Some(&CONFIG.telegram.owner))
            .unwrap_or(false)
    }

    fn is_from_channel(&self) -> bool {
        let user = match self.from() {
            Some(v) => v,
            None => return false,
        };
        let chat = match self.forward_from_chat() {
            Some(v) => v,
            None => return false,
        };

        match &CONFIG.telegram.channel_id {
            ChatId::Id(id) => user.id == 777000 && chat.id == *id,
            ChatId::ChannelUsername(name) => {
                if let Some(text) = get_channel_name(chat) {
                    &name[1..] == text
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn is_from_group(&self) -> bool {
        match CONFIG.telegram.group_id {
            ChatId::Id(id) => self.chat.id == id,
            _ => todo!(),
        }
    }

    fn get_command<T: BotCommand>(&self) -> Option<T> {
        if let Some(text) = self.text() {
            if text.starts_with('/') {
                return T::parse(&text, &CONFIG.telegram.bot_id).ok();
            }
        }
        None
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
}
