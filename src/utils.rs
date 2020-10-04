use crate::trans::TRANS;
use crate::CONFIG;
use std::collections::HashMap;
use teloxide::types::{
    Chat, ChatId, ChatKind, ChatPublic, Message, PublicChatChannel, PublicChatKind,
};
use teloxide::utils::command::BotCommand;

/// 将图片地址格式化为 html
pub fn img_urls_to_html(img_urls: &[String]) -> String {
    img_urls
        .iter()
        .map(|s| format!(r#"<img src="{}">"#, s))
        .collect::<Vec<_>>()
        .join("")
}

/// 将 tag 转换为可以直接发送至 tg 的文本格式
pub fn tags_to_string(tags: &HashMap<String, Vec<String>>) -> String {
    let trans = vec![
        (" ", "_"),
        ("_|_", " #"),
        ("-", "_"),
        ("/", "_"),
        ("·", "_"),
    ];
    tags.iter()
        .map(|(k, v)| {
            let v = v
                .iter()
                .map(|s| {
                    let mut s = TRANS.trans(k, s).to_owned();
                    for (from, to) in trans.iter() {
                        s = s.replace(from, to);
                    }
                    format!("#{}", s)
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("<code>{:>5}</code>: {}", TRANS.trans("rows", k), v)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub trait MessageExt {
    fn is_from_admin(&self) -> bool;
    fn is_from_channel(&self) -> bool;
    fn get_command<T: BotCommand>(&self) -> Option<T>;
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
    fn is_from_admin(&self) -> bool {
        if let Some(user) = self.from() {
            user.username.as_ref() == Some(&CONFIG.telegram.owner)
                || user.username == Some("GroupAnonymousBot".into())
        } else {
            false
        }
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

    fn get_command<T: BotCommand>(&self) -> Option<T> {
        if let Some(text) = self.text() {
            if text.starts_with('/') {
                return T::parse(&text, &CONFIG.telegram.bot_id).ok();
            }
        }
        None
    }
}
