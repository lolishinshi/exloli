use super::utils::*;
use crate::bot::command::*;
use crate::database::Gallery;
use crate::utils::get_message_url;
use crate::*;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use teloxide::types::*;
use tokio_stream::wrappers::UnboundedReceiverStream;

type Update = UpdateWithCx<Bot, Message>;

async fn send_pool(message: &Update) -> Result<()> {
    info!("频道消息更新，发送投票");
    let options = vec![
        "我瞎了".into(),
        "不咋样".into(),
        "还行吧".into(),
        "不错哦".into(),
        "太棒了".into(),
    ];
    let poll = send!(BOT
        .send_poll(
            message.update.chat.id,
            "看完以后发表一下感想吧！",
            options,
            PollType::Regular
        )
        .reply_to_message_id(message.update.id))?;
    let poll_id = poll.poll().unwrap().id.to_owned();
    let message_id = *message.update.forward_from_message_id().unwrap();
    debug!("投票：{} {}", message_id, poll_id);
    DB.update_poll_id(message_id, &poll_id)
}

/// 响应 /upload 命令，根据 url 上传指定画廊
async fn upload_gallery(message: &Update, urls: &[String]) -> Result<Message> {
    info!("执行：/upload {:?}", urls);
    let mut text = "收到命令，上传中……".to_owned();
    let mut reply_message = send!(message.reply_to(&text))?;
    for (idx, url) in urls.iter().enumerate() {
        match EXLOLI.upload_gallery_by_url(&url).await {
            Ok(_) => text.push_str(&format!("\n第 {} 本 - 上传成功", idx + 1)),
            Err(e) => text.push_str(&format!("\n第 {} 本 - 上传失败：{}", idx + 1, e)),
        }
        reply_message =
            send!(BOT.edit_message_text(reply_message.chat.id, reply_message.id, &text))?;
    }
    text.push_str("\n上传完毕！");
    Ok(send!(BOT.edit_message_text(
        reply_message.chat.id,
        reply_message.id,
        text
    ))?)
}

async fn delete_gallery(message: &Update) -> Result<Message> {
    info!("执行：/delete");
    let to_del = message.update.reply_to_message().context("找不到回复")?;
    let channel = to_del.forward_from_chat().context("获取来源对话失败")?;
    let mes_id = to_del
        .forward_from_message_id()
        .context("获取转发来源失败")?;
    send!(BOT.delete_message(to_del.chat.id, to_del.id))?;
    send!(BOT.delete_message(channel.id, *mes_id))?;
    DB.delete_gallery_by_message_id(*mes_id)?;
    let gallery = DB.query_gallery_by_message_id(*mes_id)?;
    Ok(send!(BOT.send_message(
        message.chat_id(),
        format!("画廊 {} 已删除", gallery.get_url())
    ))?)
}

async fn full_gallery(message: &Update, galleries: &[Gallery]) -> Result<Message> {
    info!("执行：/full");
    let mut text = "收到命令，上传完整版本中...".to_owned();
    let mut reply_message = send!(message.reply_to(&text))?;
    for (idx, gallery) in galleries.iter().enumerate() {
        match EXLOLI.update_gallery(gallery, None).await {
            Ok(_) => text.push_str(&format!("\n第 {} 本，上传成功", idx + 1)),
            Err(e) => text.push_str(&format!("\n第 {} 本，上传失败：{}", idx + 1, e)),
        }
        reply_message =
            send!(BOT.edit_message_text(reply_message.chat.id, reply_message.id, &text))?;
    }
    text.push_str("\n上传完毕！");
    Ok(send!(BOT.edit_message_text(
        reply_message.chat.id,
        reply_message.id,
        text
    ))?)
}

async fn update_tag(message: &Update, galleries: &[Gallery]) -> Result<Message> {
    info!("执行：/update_tag");
    let mut text = "收到命令，更新 tag 中...".to_owned();
    let mut reply_message = send!(message.reply_to(&text))?;
    for (idx, gallery) in galleries.iter().enumerate() {
        match EXLOLI.update_tag(&gallery, None).await {
            Ok(_) => text.push_str(&format!("\n第 {} 本，更新成功", idx + 1)),
            Err(e) => text.push_str(&format!("\n第 {} 本，更新失败：{}", idx + 1, e)),
        }
        reply_message =
            send!(BOT.edit_message_text(reply_message.chat.id, reply_message.id, &text))?;
    }
    text.push_str("\n更新完毕！");
    Ok(send!(BOT.edit_message_text(
        reply_message.chat.id,
        reply_message.id,
        text
    ))?)
}

async fn query_best(message: &Update, from: i64, to: i64, cnt: i64) -> Result<Message> {
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
                    g.score * 100.0,
                    g.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    Ok(send!(message.reply_to(text).parse_mode(ParseMode::Html))?)
}

/// 查询画廊，若失败则返回失败消息，成功则直接发送
async fn query_gallery(message: &Update, urls: &[String]) -> Result<Message> {
    let text = urls
        .iter()
        .map(|url| {
            DB.query_gallery_by_url(url)
                .map(|g| get_message_url(g.message_id))
                .unwrap_or_else(|_| "未找到！".to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(send!(message.reply_to(text))?)
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
    let mut to_delete = vec![message.update.id];
    let cmd = RuaCommand::parse(&message, &CONFIG.telegram.bot_id);
    match &cmd {
        Err(CommandError::WrongCommand(help)) => {
            info!("错误的命令：{}", help);
            if !help.is_empty() {
                to_delete.push(send!(message.reply_to(*help))?.id);
            } else {
                send!(message.delete_message())?;
            }
        }
        Ok(Ping) => {
            to_delete.push(send!(message.reply_to("pong"))?.id);
        }
        Ok(Full(g)) => {
            to_delete.push(full_gallery(&message, g).await?.id);
        }
        Ok(Delete) => {
            to_delete.push(delete_gallery(&message).await?.id);
        }
        Ok(Upload(urls)) => {
            to_delete.push(upload_gallery(&message, urls).await?.id);
        }
        Ok(UpdateTag(g)) => {
            to_delete.push(update_tag(&message, g).await?.id);
        }
        Ok(Query(urls)) => {
            query_gallery(&message, urls).await?;
        }
        Ok(Best([from, to, cnt])) => {
            query_best(&message, *from, *to, *cnt).await?;
        }
        // 收到无效命令则立即返回
        Err(CommandError::NotACommand) => return Ok(()),
    }

    // 对 query 和 best 命令的调用保留
    if matches!(cmd, Ok(Query(_)) | Ok(Best(_))) {
        to_delete.clear();
    }
    // 没有直接回复画廊的 upload full update_tag 则保留
    if matches!(cmd, Ok(Upload(_)) | Ok(Full(_)) | Ok(UpdateTag(_)))
        && message.update.reply_to_gallery().is_none()
    {
        to_delete.clear();
    }

    // 定时删除群组内的 BOT 消息
    if !to_delete.is_empty() && message.update.is_from_my_group() {
        let chat_id = message.chat_id();
        tokio::spawn(async move {
            sleep(time::Duration::from_secs(60)).await;
            for id in to_delete {
                send!(BOT.delete_message(chat_id, id)).log_on_error().await;
            }
        });
    }
    Ok(())
}

async fn poll_handler(poll: UpdateWithCx<Bot, Poll>) -> Result<()> {
    let options = poll.update.options;
    let votes = options.iter().map(|s| s.voter_count).collect::<Vec<_>>();
    let score = wilson_score(&votes);
    let votes = serde_json::to_string(&votes)?;
    debug!("投票状态变动：{} -> {}", poll.update.id, score);
    DB.update_score(&poll.update.id, score, &votes)
}

async fn inline_handler(query: UpdateWithCx<Bot, InlineQuery>) -> Result<()> {
    let text = query.update.query.trim();
    info!("行内查询：{}", text);
    let mut answer = vec![];
    if EXHENTAI_URL.is_match(text) {
        if let Ok(v) = DB.query_gallery_by_url(&query.update.query) {
            let url = get_message_url(v.message_id);
            answer.push(InlineQueryResult::Article(inline_article(v.title, url)));
        }
    }
    if answer.is_empty() {
        answer.push(InlineQueryResult::Article(inline_article(
            "未找到",
            "没有找到",
        )));
    }
    send!(BOT.answer_inline_query(query.update.id, answer))?;
    Ok(())
}

pub async fn start_bot() {
    info!("BOT 启动");
    Dispatcher::new(BOT.clone())
        .messages_handler(|rx: DispatcherHandlerRx<Bot, Message>| {
            UnboundedReceiverStream::new(rx).for_each_concurrent(8, |message| async {
                message_handler(message).await.log_on_error().await;
            })
        })
        .polls_handler(|rx: DispatcherHandlerRx<Bot, Poll>| {
            UnboundedReceiverStream::new(rx).for_each_concurrent(8, |message| async {
                poll_handler(message).await.log_on_error().await;
            })
        })
        .inline_queries_handler(|rx: DispatcherHandlerRx<Bot, InlineQuery>| {
            UnboundedReceiverStream::new(rx).for_each_concurrent(8, |message| async {
                inline_handler(message).await.log_on_error().await;
            })
        })
        .dispatch()
        .await;
}
