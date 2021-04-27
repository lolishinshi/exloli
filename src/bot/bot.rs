use super::utils::{Update, *};
use crate::bot::command::*;
use crate::database::Gallery;
use crate::utils::get_message_url;
use crate::*;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use std::convert::TryInto;
use teloxide::types::*;
use tokio_stream::wrappers::UnboundedReceiverStream;

// TODO: 新本子继承父画廊的投票？
async fn on_new_gallery(message: &Update<Message>) -> Result<()> {
    info!("频道消息更新，发送投票");
    // 辣鸡 tg 安卓客户端在置顶消息过多时似乎在进群时会卡住
    BOT.unpin_chat_message(message.update.chat.id)
        .message_id(message.update.id)
        .await?;
    let options = vec![
        "我瞎了".into(),
        "不咋样".into(),
        "还行吧".into(),
        "不错哦".into(),
        "太棒了".into(),
    ];
    let poll = BOT
        .send_poll(
            message.update.chat.id,
            "看完以后发表一下感想吧！",
            options,
            PollType::Regular,
        )
        .reply_to_message_id(message.update.id)
        .await?;
    let poll_id = poll.poll().unwrap().id.to_owned();
    let message_id = *message.update.forward_from_message_id().unwrap();
    debug!("投票：{} {}", message_id, poll_id);
    DB.update_poll_id(message_id, &poll_id)
}

/// 响应 /upload 命令，根据 url 上传指定画廊
async fn cmd_upload(message: &Update<Message>, urls: &[String]) -> Result<Message> {
    info!("执行：/upload {:?}", urls);
    let mut text = "收到命令，上传中……".to_owned();
    let mut reply_message = message.reply_to(&text).await?;
    for (idx, url) in urls.iter().enumerate() {
        match EXLOLI.upload_gallery_by_url(&url).await {
            Ok(_) => text.push_str(&format!("\n第 {} 本 - 上传成功", idx + 1)),
            Err(e) => text.push_str(&format!("\n第 {} 本 - 上传失败：{}", idx + 1, e)),
        }
        reply_message = BOT
            .edit_message_text(reply_message.chat.id, reply_message.id, &text)
            .await?;
    }
    text.push_str("\n上传完毕！");
    Ok(BOT
        .edit_message_text(reply_message.chat.id, reply_message.id, text)
        .await?)
}

async fn cmd_delete(message: &Update<Message>, real: bool) -> Result<Message> {
    info!("执行：/delete {}", real);
    let to_del = message.update.reply_to_message().context("找不到回复")?;
    let channel = to_del.forward_from_chat().context("获取来源对话失败")?;
    let mes_id = to_del
        .forward_from_message_id()
        .context("获取转发来源失败")?;
    BOT.delete_message(to_del.chat.id, to_del.id).await?;
    BOT.delete_message(channel.id, *mes_id).await?;
    let gallery = DB.query_gallery_by_message_id(*mes_id)?;
    match real {
        false => DB.delete_gallery_by_message_id(*mes_id)?,
        _ => DB.real_delete_gallery_by_message_id(*mes_id)?,
    }
    let text = format!("画廊 {} 已删除", gallery.get_url());
    Ok(BOT.send_message(message.chat_id(), text).await?)
}

async fn cmd_full(message: &Update<Message>, galleries: &[InputGallery]) -> Result<Message> {
    info!("执行：/full");
    let mut text = "收到命令，更新完整版本中...".to_owned();
    let mut reply_message = message.reply_to(&text).await?;
    for (idx, gallery) in galleries.iter().enumerate() {
        let gallery = match gallery.to_gallery() {
            Ok(v) => v,
            Err(_) => {
                text.push_str(&format!("\n第 {} 本，未上传", idx + 1));
                continue;
            }
        };
        match EXLOLI.update_gallery(&gallery, None).await {
            Ok(_) => text.push_str(&format!("\n第 {} 本，更新成功", idx + 1)),
            Err(e) => text.push_str(&format!("\n第 {} 本，更新失败：{}", idx + 1, e)),
        }
        reply_message = BOT
            .edit_message_text(reply_message.chat.id, reply_message.id, &text)
            .await?;
    }
    text.push_str("\n更新完毕！");
    Ok(BOT
        .edit_message_text(reply_message.chat.id, reply_message.id, text)
        .await?)
}

async fn cmd_update_tag(message: &Update<Message>, galleries: &[InputGallery]) -> Result<Message> {
    info!("执行：/update_tag");
    let mut text = "收到命令，更新 tag 中...".to_owned();
    let mut reply_message = message.reply_to(&text).await?;
    for (idx, gallery) in galleries.iter().enumerate() {
        let gallery = match gallery.to_gallery() {
            Ok(v) => v,
            Err(_) => {
                text.push_str(&format!("\n第 {} 本，未上传", idx + 1));
                continue;
            }
        };
        match EXLOLI.update_tag(&gallery, None).await {
            Ok(_) => text.push_str(&format!("\n第 {} 本，更新成功", idx + 1)),
            Err(e) => text.push_str(&format!("\n第 {} 本，更新失败：{}", idx + 1, e)),
        }
        reply_message = BOT
            .edit_message_text(reply_message.chat.id, reply_message.id, &text)
            .await?;
    }
    text.push_str("\n更新完毕！");
    Ok(BOT
        .edit_message_text(reply_message.chat.id, reply_message.id, text)
        .await?)
}

fn query_best_text(from: i64, to: i64, offset: i64) -> Result<String> {
    let (from_d, to_d) = (
        Utc::today().naive_utc() - Duration::days(from),
        Utc::today().naive_utc() - Duration::days(to),
    );
    let galleries = DB.query_best(from_d, to_d, offset)?;
    let list = galleries
        .iter()
        .map(|g| {
            format!(
                r#"<code>{:.2}</code> - <a href="{}">{}</a>"#,
                g.score * 100.,
                get_message_url(g.message_id),
                g.title
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut text = format!("最近 {} - {} 天的本子排名（{}）：\n", from, to, offset);
    text.push_str(&list);
    Ok(text)
}

fn query_best_keyboard(from: i64, to: i64, offset: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::new(
            "<<",
            InlineKeyboardButtonKind::CallbackData(format!("<< {} {} {}", from, to, offset)),
        ),
        InlineKeyboardButton::new(
            "<",
            InlineKeyboardButtonKind::CallbackData(format!("< {} {} {}", from, to, offset)),
        ),
        InlineKeyboardButton::new(
            ">",
            InlineKeyboardButtonKind::CallbackData(format!("> {} {} {}", from, to, offset)),
        ),
        InlineKeyboardButton::new(
            ">>",
            InlineKeyboardButtonKind::CallbackData(format!(">> {} {} {}", from, to, offset)),
        ),
    ]])
}

async fn cmd_best(message: &Update<Message>, from: i64, to: i64) -> Result<Message> {
    info!("执行：/best {} {}", from, to);
    let text = query_best_text(from, to, 1)?;
    let reply_markup = query_best_keyboard(from, to, 1);
    Ok(message
        .reply_to(text)
        .reply_markup(reply_markup)
        .parse_mode(ParseMode::Html)
        .await?)
}

/// 查询画廊，若失败则返回失败消息，成功则直接发送
async fn cmd_query(message: &Update<Message>, gs: &[InputGallery]) -> Result<Message> {
    let text = match gs.len() {
        1 => gs[0]
            .to_gallery()
            .and_then(|g| cmd_query_rank(&g))
            .unwrap_or_else(|_| "未找到！".to_owned()),
        _ => gs
            .iter()
            .map(|g| {
                g.to_gallery()
                    .map(|g| get_message_url(g.message_id))
                    .unwrap_or_else(|_| "未找到！".to_owned())
            })
            .collect::<Vec<_>>()
            .join("\n"),
    };
    Ok(message.reply_to(text).await?)
}

fn cmd_query_rank(gallery: &Gallery) -> Result<String> {
    let rank = DB.get_rank(gallery.score)?;
    Ok(format!(
        "标题：{}\n消息：{}\n地址：{}\n评分：{:.2}\n位置：{:.2}%\n上传日期：{}",
        gallery.title,
        get_message_url(gallery.message_id),
        gallery.get_url(),
        gallery.score * 100.,
        rank * 100.,
        gallery.publish_date,
    ))
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

async fn message_handler(message: Update<Message>) -> Result<()> {
    use RuaCommand::*;

    trace!("{:#?}", message.update);

    // 如果是新本子上传的消息，则回复投票并取消置顶
    if is_new_gallery(&message.update) && message.update.is_from_my_group() {
        on_new_gallery(&message).await.log_on_error().await;
    }

    // 其他命令
    let mut to_delete = vec![message.update.id];
    let cmd = RuaCommand::parse(&message, &CONFIG.telegram.bot_id);
    match &cmd {
        Err(CommandError::WrongCommand(help)) => {
            info!("错误的命令：{}", help);
            if !help.is_empty() {
                to_delete.push(message.reply_to(*help).await?.id);
            } else {
                message.delete_message().await?;
            }
        }
        Ok(Ping) => {
            to_delete.push(message.reply_to("pong").await?.id);
        }
        Ok(Full(g)) => {
            to_delete.push(cmd_full(&message, g).await?.id);
        }
        Ok(Delete) => {
            to_delete.push(cmd_delete(&message, false).await?.id);
        }
        Ok(RealDelete) => {
            to_delete.push(cmd_delete(&message, true).await?.id);
        }
        Ok(Upload(urls)) => {
            to_delete.push(cmd_upload(&message, urls).await?.id);
        }
        Ok(UpdateTag(g)) => {
            to_delete.push(cmd_update_tag(&message, g).await?.id);
        }
        Ok(Query(gs)) => {
            cmd_query(&message, gs).await?;
        }
        Ok(Best([from, to])) => {
            cmd_best(&message, *from, *to).await?;
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
                BOT.delete_message(chat_id, id).await.log_on_error().await;
            }
        });
    }
    Ok(())
}

async fn poll_handler(poll: Update<Poll>) -> Result<()> {
    let options = poll.update.options;
    let votes = options.iter().map(|s| s.voter_count).collect::<Vec<_>>();
    let score = wilson_score(&votes);
    let votes = serde_json::to_string(&votes)?;
    debug!("投票状态变动：{} -> {}", poll.update.id, score);
    DB.update_score(&poll.update.id, score, &votes)
}

async fn inline_handler(query: Update<InlineQuery>) -> Result<()> {
    let text = query.update.query.trim();
    info!("行内查询：{}", text);
    let mut answer = vec![];
    if EXHENTAI_URL.is_match(text) {
        if let Ok(v) = DB.query_gallery_by_url(&query.update.query) {
            let content = cmd_query_rank(&v)?;
            answer.push(InlineQueryResult::Article(inline_article(v.title, content)));
        }
    }
    if answer.is_empty() {
        answer.push(InlineQueryResult::Article(inline_article(
            "未找到",
            "没有找到",
        )));
    }
    BOT.answer_inline_query(query.update.id, answer).await?;
    Ok(())
}

async fn callback_handler(callback: Update<CallbackQuery>) -> Result<()> {
    let update = callback.update;
    info!("回调：{:?}", update.data);

    let (cmd, data) = match update.data.as_ref().and_then(|v| {
        // TODO: split_once
        v.find(' ').map(|n| (&v[..n], &v[n + 1..]))
    }) {
        Some(v) => v,
        None => return Ok(()),
    };

    let message = match update.message {
        Some(v) => v,
        None => {
            BOT.answer_callback_query(update.id)
                .text("该消息过旧")
                .show_alert(true)
                .await?;
            return Ok(());
        }
    };
    match cmd {
        "<<" | ">>" | "<" | ">" => {
            // vec![from, to, offset]
            let data = data
                .split(' ')
                .map(|s| s.parse::<i64>())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let [from, to, mut offset] = match TryInto::<[i64; 3]>::try_into(data) {
                Ok(v) => v,
                _ => return Ok(()),
            };
            match cmd {
                ">" => offset += 20,
                "<" => offset -= 20,
                ">>" => offset = -1,
                "<<" => offset = 1,
                _ => (),
            };
            let text = query_best_text(from, to, offset)?;
            let reply = query_best_keyboard(from, to, offset);
            BOT.edit_message_text(message.chat.id, message.id, &text)
                .parse_mode(ParseMode::Html)
                .reply_markup(reply)
                .await?;
        }
        _ => error!("未知指令：{}", cmd),
    };
    Ok(())
}

pub async fn start_bot() {
    info!("BOT 启动");
    type DispatcherHandler<T> = DispatcherHandlerRx<AutoSend<Bot>, T>;
    Dispatcher::new(BOT.clone())
        .messages_handler(|rx: DispatcherHandler<Message>| {
            UnboundedReceiverStream::new(rx).for_each_concurrent(8, |message| async {
                message_handler(message).await.log_on_error().await;
            })
        })
        .polls_handler(|rx: DispatcherHandler<Poll>| {
            UnboundedReceiverStream::new(rx).for_each_concurrent(8, |message| async {
                poll_handler(message).await.log_on_error().await;
            })
        })
        .inline_queries_handler(|rx: DispatcherHandler<InlineQuery>| {
            UnboundedReceiverStream::new(rx).for_each_concurrent(8, |message| async {
                inline_handler(message).await.log_on_error().await;
            })
        })
        .callback_queries_handler(|rx: DispatcherHandler<CallbackQuery>| {
            UnboundedReceiverStream::new(rx).for_each_concurrent(8, |message| async {
                callback_handler(message).await.log_on_error().await;
            })
        })
        .dispatch()
        .await;
}
