use super::utils::*;
use crate::exloli::ExLoli;
use crate::*;
use anyhow::Result;
use chrono::{Duration, Utc};
use std::sync::Arc;
use teloxide::types::*;
use teloxide::utils::command::BotCommand;

macro_rules! check_is_owner {
    ($e:expr) => {
        if !check_is_channel_admin($e).await? {
            info!("权限检查失败");
            send!($e.delete_message())?;
            return Ok(());
        }
    };
}

// 检测是否是指定频道的管理员
async fn check_is_channel_admin(message: &UpdateWithCx<Message>) -> Result<bool> {
    // 先检测是否为匿名管理员
    let from_user = message.update.from();
    if from_user
        .map(|u| u.username == Some("GroupAnonymousBot".into()))
        .unwrap_or(false)
        && message.update.is_from_my_group()
    {
        return Ok(true);
    }
    // TODO: 缓存管理员名单
    let channel_admins = send!(BOT.get_chat_administrators(CONFIG.telegram.channel_id.clone()))?;
    Ok(message
        .update
        .from()
        .map(|user| {
            channel_admins
                .iter()
                .map(|admin| &admin.user == user)
                .any(|x| x)
        })
        .unwrap_or(false))
}

#[derive(BotCommand, PartialEq, Debug)]
#[command(rename = "lowercase", parse_with = "split")]
enum RuaCommand {
    Upload(String),
    Ping,
    Delete,
    // 最少几天前 最多几天前 多少本
    Best(u16, u16, u8),
    Full,
}

type Update = UpdateWithCx<Message>;

async fn send_pool(message: &Update) -> Result<()> {
    info!("频道消息更新，发送投票");
    let options = vec![
        "我瞎了".into(),
        "不咋样".into(),
        "还行吧".into(),
        "不错哦".into(),
        "太棒了".into(),
    ];
    let poll = send!(message
        .bot
        .send_poll(message.update.chat.id, "看完以后发表一下感想吧！", options)
        .reply_to_message_id(message.update.id))?;
    let poll_id = poll.poll().unwrap().id.to_owned();
    let message_id = *message.update.forward_from_message_id().unwrap();
    debug!("投票：{} {}", message_id, poll_id);
    DB.update_poll_id(message_id, &poll_id)
}

pub async fn upload_gallery(message: &Update, url: &str, exloli: &ExLoli) -> Result<()> {
    check_is_owner!(&message);
    let reply_message = send!(message.reply_to("收到命令，上传中……"))?.to_chat_or_inline_message();
    let mut text = "上传完毕".to_owned();
    if let Err(e) = exloli.upload_gallery_by_url(&url).await {
        error!("上传出错：{}", e);
        text = format!("上传失败：{}", e);
    }
    send!(message.bot.edit_message_text(reply_message, text))?;
    Ok(())
}

async fn delete_gallery(message: &Update) -> Result<()> {
    check_is_owner!(&message);
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
    DB.delete_gallery_by_message_id(*mes_id)?;
    Ok(())
}

async fn update_gallery_to_full(message: &Update, exloli: &ExLoli) -> Result<()> {
    check_is_owner!(&message);
    let reply_message =
        send!(message.reply_to("收到命令，将更新该画廊的完整版本……"))?.to_chat_or_inline_message();

    let url = match message.update.reply_to_gallery() {
        Some(v) => v.get_url(),
        None => {
            send!(message.reply_to("请回复需要更新的画廊"))?;
            return Ok(());
        }
    };

    let mut text = "更新完毕".to_owned();
    if let Err(e) = exloli.upload_gallery_by_url(&url).await {
        error!("上传出错：{}", e);
        text = format!("更新失败：{}", e);
    }
    send!(message.bot.edit_message_text(reply_message, text))?;
    Ok(())
}

async fn query_best(message: &Update, from: i64, to: i64, cnt: i64) -> Result<()> {
    if cnt > 20 {
        send!(message.reply_to("最多展示前 20 本"))?;
        return Ok(());
    }
    let (from_d, to_d) = (
        Utc::today().naive_utc() - Duration::days(from),
        Utc::today().naive_utc() - Duration::days(to),
    );
    let galleries = DB.query_best(from_d, to_d, cnt)?;
    let mut text = format!("最近 {} - {} 天评分最高的 {} 本本子：\n", from, to, cnt);
    text.push_str(
        &galleries
            .iter()
            .map(|g| {
                format!(
                    r#"<a href="https://t.me/{}/{}">{:.2} - {}</a>"#,
                    CONFIG.telegram.channel_id, g.message_id, g.score, g.title
                )
                .replace("/-100", "/")
                .replace("@", "")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    send!(message.reply_to(text))?;
    Ok(())
}

/// 判断是否是新本子的发布信息
fn is_new_gallery(message: &Message) -> bool {
    // 判断是否是由官方 bot 转发的
    let user = unwrap!(message.from(), false);
    if user.id != 777000 {
        return false;
    }
    // 判断是否是新本子的发布信息
    message
        .text()
        .map(|s| s.contains("原始地址"))
        .unwrap_or(false)
}

async fn message_handler(exloli: Arc<ExLoli>, message: Update) -> Result<()> {
    use RuaCommand::*;

    trace!("{:#?}", message.update);

    // 如果是新本子上传的消息，则回复投票
    if is_new_gallery(&message.update) && message.update.is_from_my_group() {
        send_pool(&message).await.log_on_error().await;
    }

    // 其他命令
    if let Some(command) = message.update.get_command() {
        info!("收到命令：{:?}", command);
        match command {
            Upload(url) => upload_gallery(&message, &url, &exloli).await?,
            Delete => delete_gallery(&message).await?,
            Ping => {
                send!(message.reply_to("pong"))?;
            }
            Best(from, to, cnt) => query_best(&message, from as i64, to as i64, cnt as i64).await?,
            Full => update_gallery_to_full(&message, &exloli).await?,
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
    debug!("投票状态变动：{} -> {}", poll.update.id, score);
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
