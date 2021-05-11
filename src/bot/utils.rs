use crate::database::Gallery;
use crate::{BOT, CONFIG, DB};
use cached::proc_macro::cached;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::hash::Hash;
use std::time::{Duration, Instant};
use teloxide::prelude::*;
use teloxide::types::*;
use tokio::task::block_in_place;
use uuid::Uuid;

pub static EXHENTAI_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"https://e.hentai\.org/g/\d+/[0-9a-f]+/?").unwrap());
pub static MESSAGE_URL: Lazy<Regex> = Lazy::new(|| {
    let channel_id = &CONFIG.telegram.channel_id;
    Regex::new(
        &format!(r"https://t.me/{}/(\d+)", channel_id)
            .replace("/-100", "/")
            .replace("@", ""),
    )
    .unwrap()
});

pub type Update<T> = UpdateWithCx<AutoSend<Bot>, T>;

pub trait MessageExt {
    fn is_from_my_group(&self) -> bool;
    fn from_username(&self) -> Option<&String>;
    fn reply_to_user(&self) -> Option<&User>;
    fn reply_to_gallery(&self) -> Option<Gallery>;
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
            .and_then(|mess_id| DB.query_gallery(*mess_id).ok())
    }
}

/// 获取管理员列表，提供 1 个小时的缓存
#[cached(time = 3600)]
async fn get_admins() -> Vec<User> {
    let mut admins = BOT
        .get_chat_administrators(CONFIG.telegram.channel_id.clone())
        .await
        .unwrap_or_default();
    admins.extend(
        BOT.get_chat_administrators(CONFIG.telegram.group_id.clone())
            .await
            .unwrap_or_default(),
    );
    admins.into_iter().map(|member| member.user).collect()
}

// 检测是否是指定频道的管理员
pub fn check_is_channel_admin(message: &Update<Message>) -> bool {
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

pub fn inline_article<S1, S2>(title: S1, content: S2) -> InlineQueryResultArticle
where
    S1: Into<String>,
    S2: Into<String>,
{
    let content = content.into();
    let uuid = Uuid::new_v3(
        &Uuid::from_bytes(b"EXLOLIINLINEQURY".to_owned()),
        content.as_bytes(),
    );
    InlineQueryResultArticle::new(
        uuid.to_string(),
        title.into(),
        InputMessageContent::Text(InputMessageContentText::new(content)),
    )
}

pub fn poll_keyboard(poll_id: i32, votes: &[i32; 5]) -> InlineKeyboardMarkup {
    let sum = votes.iter().sum::<i32>();
    let votes: Box<dyn Iterator<Item = f32>> = if sum == 0 {
        Box::new([0.].iter().cloned().cycle())
    } else {
        Box::new(votes.iter().map(|&i| i as f32 / sum as f32 * 100.))
    };

    let options = ["我瞎了", "不咋样", "还行吧", "不错哦", "太棒了"]
        .iter()
        .zip(votes)
        .enumerate()
        .map(|(idx, (name, vote))| {
            vec![InlineKeyboardButton::new(
                format!("{:.0}% {}", vote, name),
                InlineKeyboardButtonKind::CallbackData(format!("vote {} {}", poll_id, idx + 1)),
            )]
        })
        .collect::<Vec<_>>();

    InlineKeyboardMarkup::new(options)
}

pub fn query_best_keyboard(from: i64, to: i64, offset: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![["<<", "<", ">", ">>"]
        .iter()
        .map(|&s| {
            InlineKeyboardButton::new(
                s,
                InlineKeyboardButtonKind::CallbackData(format!("{} {} {} {}", s, from, to, offset)),
            )
        })
        .collect::<Vec<_>>()])
}

/// 威尔逊得分
/// 基于：https://www.jianshu.com/p/4d2b45918958
pub fn wilson_score(votes: &[i32]) -> f32 {
    let base = [0., 0.25, 0.5, 0.75, 1.];
    let votes = votes.to_owned();
    let count = votes.iter().sum::<i32>() as f32;
    if count == 0. {
        return 0.;
    }
    let mean = Iterator::zip(votes.iter(), base.iter())
        .map(|(&a, &b)| a as f32 * b)
        .sum::<f32>()
        / count;
    let var = Iterator::zip(votes.iter(), base.iter())
        .map(|(&a, &b)| (mean - b).powi(2) * a as f32)
        .sum::<f32>()
        / count;
    // 80% 置信度
    let z = 1.281f32;

    (mean + z.powi(2) / (2. * count) - ((z / (2. * count)) * (4. * count * var + z.powi(2)).sqrt()))
        / (1. + z.powi(2) / count)
}

/// 一个用于限制请求频率的数据结构
#[derive(Debug)]
pub struct RateLimiter<T: Hash + Eq> {
    interval: Duration,
    limit: usize,
    data: DashMap<T, VecDeque<Instant>>,
}

impl<T: Hash + Eq> RateLimiter<T> {
    pub fn new(interval: Duration, limit: usize) -> Self {
        assert_ne!(limit, 0);
        Self {
            interval,
            limit,
            data: Default::default(),
        }
    }

    /// 插入数据，正常情况下返回 None，如果达到了限制则返回需要等待的时间
    pub fn insert(&self, key: T) -> Option<Duration> {
        let mut entry = self.data.entry(key).or_insert_with(VecDeque::new);
        let entry = entry.value_mut();
        // 插入时，先去掉已经过期的元素
        while let Some(first) = entry.front() {
            if first.elapsed() > self.interval {
                entry.pop_front();
            } else {
                break;
            }
        }
        if entry.len() == self.limit {
            return entry.front().cloned().map(|d| self.interval - d.elapsed());
        }
        entry.push_back(Instant::now());
        None
    }
}
