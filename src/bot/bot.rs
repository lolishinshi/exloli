use super::utils::*;
use crate::bot::command::*;
use crate::utils::get_message_url;
use crate::*;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use teloxide::types::*;

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

/// 响应 /upload 命令，根据 url 上传指定画廊
async fn upload_gallery(message: &Update, url: &str) -> Result<Message> {
    info!("执行：/upload {}", url);
    let reply_message = send!(message.reply_to("收到命令，上传中……"))?.to_chat_or_inline_message();
    let mut text = "上传完毕".to_owned();
    if let Err(e) = EXLOLI.upload_gallery_by_url(&url).await {
        error!("上传出错：{}", e);
        text = format!("上传失败：{}", e);
    }
    Ok(send!(message.bot.edit_message_text(reply_message, text))?)
}

async fn delete_gallery(message: &Update) -> Result<Message> {
    info!("执行：/delete");
    let to_del = message.update.reply_to_message().context("找不到回复")?;
    let channel = to_del.forward_from_chat().context("获取来源对话失败")?;
    let mes_id = to_del
        .forward_from_message_id()
        .context("获取转发来源失败")?;
    send!(message.bot.delete_message(to_del.chat.id, to_del.id))?;
    send!(message.bot.delete_message(channel.id, *mes_id))?;
    DB.delete_gallery_by_message_id(*mes_id)?;
    let gallery = DB.query_gallery_by_message_id(*mes_id)?;
    Ok(send!(BOT.send_message(
        message.chat_id(),
        format!("画廊 {} 已删除", gallery.get_url())
    ))?)
}

async fn full_gallery(message: &Update) -> Result<Message> {
    info!("执行：/full");
    let reply_message =
        send!(message.reply_to("收到命令，将更新该画廊的完整版本……"))?.to_chat_or_inline_message();

    let gallery = message.update.reply_to_gallery().context("找不到回复")?;

    let mut text = "更新完毕".to_owned();
    if let Err(e) = EXLOLI.upload_gallery_by_url(&gallery.get_url()).await {
        error!("上传出错：{}", e);
        text = format!("更新失败：{}", e);
    }
    Ok(send!(message.bot.edit_message_text(reply_message, text))?)
}

async fn query_best(message: &Update, from: i64, to: i64, cnt: i64) -> Result<()> {
    info!("执行：/best {} {} {}", from, to, cnt);
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
                    r#"<a href="{}">{:.2} - {}</a>"#,
                    get_message_url(g.message_id),
                    g.score,
                    g.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    send!(message.reply_to(text))?;
    Ok(())
}

async fn query_gallery(message: &Update, url: &str) -> Result<Option<Message>> {
    match DB.query_gallery_by_url(url) {
        Ok(g) => {
            send!(message.reply_to(get_message_url(g.message_id)))?;
            Ok(None)
        }
        _ => Ok(Some(send!(message.reply_to("未找到！"))?)),
    }
}

/// 判断是否是新本子的发布信息
fn is_new_gallery(message: &Message) -> bool {
    // 判断是否是由官方 bot 转发的
    let user = match message.from() {
        Some(v) => v,
        _ => return false,
    };
    if user.id != 777000 {
        return false;
    }
    // 判断是否是新本子的发布信息
    message
        .text()
        .map(|s| s.contains("原始地址"))
        .unwrap_or(false)
}

async fn message_handler(message: Update) -> Result<()> {
    use RuaCommand::*;

    trace!("{:#?}", message.update);

    // 如果是新本子上传的消息，则回复投票
    if is_new_gallery(&message.update) && message.update.is_from_my_group() {
        send_pool(&message).await.log_on_error().await;
    }

    // 其他命令
    let mut to_delete = vec![];
    match RuaCommand::parse(&message, &CONFIG.telegram.bot_id) {
        Err(CommandError::WrongCommand(help)) => {
            info!("错误的命令：{}", help);
            if !help.is_empty() {
                to_delete.push(send!(message.reply_to(help))?.id);
                to_delete.push(message.update.id);
            } else {
                send!(message.delete_message())?;
            }
        }
        Ok(Ping) => {
            to_delete.push(send!(message.reply_to("pong"))?.id);
            to_delete.push(message.update.id);
        }
        Ok(Full) => {
            to_delete.push(full_gallery(&message).await?.id);
            to_delete.push(message.update.id);
        }
        Ok(Delete) => {
            to_delete.push(delete_gallery(&message).await?.id);
            to_delete.push(message.update.id);
        }
        Ok(Upload(url)) => {
            to_delete.push(upload_gallery(&message, &url).await?.id);
            to_delete.push(message.update.id);
        }
        Ok(Query(url)) => {
            if let Some(m) = query_gallery(&message, &url).await? {
                to_delete.push(m.id);
            }
        }
        Ok(Best([from, to, cnt])) => query_best(&message, from, to, cnt).await?,
        _ => {}
    }

    // 群组内的消息
    if !to_delete.is_empty() && message.update.is_from_my_group() {
        let chat_id = message.chat_id();
        tokio::spawn(async move {
            delay_for(time::Duration::from_secs(60)).await;
            for id in to_delete {
                send!(BOT.delete_message(chat_id, id)).log_on_error().await;
            }
        });
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

pub async fn start_bot() {
    info!("BOT 启动");
    Dispatcher::new(BOT.clone())
        .messages_handler(|rx: DispatcherHandlerRx<Message>| {
            rx.for_each_concurrent(8, |message| async {
                message_handler(message).await.log_on_error().await;
            })
        })
        .polls_handler(|rx: DispatcherHandlerRx<Poll>| {
            rx.for_each_concurrent(8, |message| async {
                poll_handler(message).await.log_on_error().await;
            })
        })
        .dispatch()
        .await;
}
