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
    ($e:expr, $r:expr) => {
        match $e {
            Some(v) => v,
            None => return $r,
        }
    };
}

pub trait MessageExt {
    fn is_from_owner(&self) -> bool;
    fn is_from_group(&self) -> bool;
    fn get_command<T: BotCommand>(&self) -> Option<T>;
    fn from_username(&self) -> Option<&String>;
    fn reply_to_user(&self) -> Option<&User>;
}

impl MessageExt for Message {
    /// 判断是否是由所有者发出的消息，其中包含了匿名 bot
    fn is_from_owner(&self) -> bool {
        self.from()
            .map(|u| {
                u.username.as_ref() == Some(&CONFIG.telegram.owner)
                    || u.username == Some("GroupAnonymousBot".into())
            })
            .unwrap_or(false)
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
