use super::utils::*;
use crate::bot::command::*;
use crate::database::Gallery;
use crate::utils::get_message_url;
use crate::*;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use futures::{FutureExt, TryFutureExt};
use std::convert::TryInto;
use std::future::Future;
use teloxide::types::*;
use teloxide::{ApiError, RequestError};

macro_rules! reply_to {
    ($b:expr, $m:expr, $t:expr) => {
        $b.send_message($m.chat.id, $t).reply_to_message_id($m.id)
    };
}

static LIMIT: Lazy<RateLimiter<u64>> =
    Lazy::new(|| RateLimiter::new(std::time::Duration::from_secs(60), 10));
//
async fn on_new_gallery(bot: Bot, message: &Message) -> Result<()> {
    info!("频道消息更新，发送投票");
    // 辣鸡 tg 安卓客户端在置顶消息过多时似乎在进群时会卡住
    bot.unpin_chat_message(message.chat.id)
        .message_id(message.id)
        .await?;
    let message_id = message.forward_from_message_id().unwrap();
    let poll_id = DB.query_poll_id(message_id)?.parse::<i32>()?;
    let votes = Vote::new(DB.query_vote(poll_id)?);
    let options = poll_keyboard(poll_id, &votes);
    reply_to!(bot, message, votes.info())
        .reply_markup(options)
        .await?;
    Ok(())
}
//
async fn cmd_delete(bot: Bot, message: &Message, real: bool) -> Result<Message> {
    info!("执行命令: delete_{} {:?}", real, message.id);
    let to_del = message.reply_to_message().context("找不到回复")?;
    let channel = to_del.forward_from_chat().context("获取来源对话失败")?;
    let msg_id = to_del
        .forward_from_message_id()
        .context("获取转发来源失败")?;
    bot.delete_message(to_del.chat.id, to_del.id).await?;
    bot.delete_message(channel.id, MessageId(msg_id)).await?;
    let gallery = DB.query_gallery(msg_id)?;
    match real {
        false => DB.delete_gallery(msg_id)?,
        _ => DB.real_delete_gallery(msg_id)?,
    }
    let text = format!("画廊 {} 已删除", gallery.get_url());
    Ok(bot.send_message(message.chat.id, text).await?)
}

async fn do_chain_action<T, F, Fut>(
    bot: Bot,
    message: &Message,
    input: &[T],
    action: F,
) -> Result<Message>
where
    F: Fn(&T) -> Fut,
    Fut: Future<Output = Result<Option<()>>>,
{
    let mut text = "收到命令，执行中……".to_owned();
    let mut reply_message = reply_to!(bot, message, &text).await?;
    let mut fail_cnt = 0;
    for (idx, entry) in input.iter().enumerate() {
        let message = match action(entry).await {
            Ok(Some(_)) => format!("\n第 {} 本 - 成功", idx + 1),
            Ok(None) => format!("\n第 {} 本 - 无上传记录", idx + 1),
            Err(e) => {
                let source = e.source().map(|e| e.to_string()).unwrap_or_default();
                fail_cnt += 1;
                format!("\n第 {} 本 - 失败：{} => {}", idx + 1, e, source)
            }
        };
        text.push_str(&message);
        reply_message = bot
            .edit_message_text(reply_message.chat.id, reply_message.id, &text)
            .await?;
    }
    text.push_str("\n执行完毕");
    if fail_cnt > 0 {
        text.push_str(&format!(
            "，失败 {} 个<a href=\"tg://user?id={}\">\u{200b}</a>",
            fail_cnt,
            message.from().map(|u| u.id.0).unwrap_or_default()
        ));
    }
    Ok(bot
        .edit_message_text(reply_message.chat.id, reply_message.id, text)
        .parse_mode(ParseMode::Html)
        .await?)
}

async fn cmd_upload(bot: Bot, message: &Message, urls: &[String]) -> Result<Message> {
    info!("执行命令: upload {:?}", urls);
    do_chain_action(bot, message, urls, |url| {
        let url = url.clone();
        async move { EXLOLI.upload_gallery_by_url(&url).await.map(Some) }.boxed()
    })
    .await
}

async fn cmd_reupload(
    bot: Bot,
    message: &Message,
    old_gallery: &[InputGallery],
) -> Result<Message> {
    info!("执行命令: reupload {:?}", old_gallery);
    let mut text = "收到命令，执行中……".to_owned();
    let reply_message = reply_to!(bot, message, &text).await?;

    for gallery in old_gallery {
        if let InputGallery::Gallery(gallery) = gallery {
            EXLOLI.update_gallery(&gallery, None, true).await?;
        }
    }

    text.push_str("\n执行完毕");
    Ok(bot
        .edit_message_text(reply_message.chat.id, reply_message.id, text)
        .parse_mode(ParseMode::Html)
        .await?)
}

async fn cmd_full(bot: Bot, message: &Message, galleries: &[InputGallery]) -> Result<Message> {
    info!("执行命令: full {:?}", galleries);
    do_chain_action(bot, message, galleries, |gallery| {
        let gallery = match block_on(gallery.to_gallery()) {
            Ok(v) => v,
            _ => return async { Ok(None) }.boxed(),
        };
        async move { EXLOLI.update_gallery(&gallery, None, false).await.map(Some) }.boxed()
    })
    .await
}

async fn cmd_update_tag(
    bot: Bot,
    message: &Message,
    galleries: &[InputGallery],
) -> Result<Message> {
    info!("执行命令: uptag {:?}", galleries);
    do_chain_action(bot, message, galleries, |gallery| {
        // TODO: 为啥要 block
        let gallery = match block_on(gallery.to_gallery()) {
            Ok(v) => v,
            _ => return async { Ok(None) }.boxed(),
        };
        async move { EXLOLI.update_tag(&gallery, None).await.map(Some) }.boxed()
    })
    .await
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

async fn cmd_best(bot: Bot, message: &Message, from: i64, to: i64) -> Result<Message> {
    info!("执行命令: best {} {}", from, to);
    let text = query_best_text(from, to, 1)?;
    let reply_markup = query_best_keyboard(from, to, 1);
    Ok(reply_to!(bot, message, text)
        .reply_markup(reply_markup)
        .parse_mode(ParseMode::Html)
        .await?)
}

/// 查询画廊，若失败则返回失败消息，成功则直接发送
async fn cmd_query(bot: Bot, message: &Message, galleries: &[InputGallery]) -> Result<Message> {
    info!("执行命令: query {:?}", galleries);
    let text = match galleries.len() {
        1 => galleries[0]
            .to_gallery()
            .await
            .and_then(|g| cmd_query_rank(&g))
            .unwrap_or_else(|_| "未找到！".to_owned()),
        _ => futures::future::join_all(galleries.iter().map(|g| {
            g.to_gallery()
                .and_then(|g| async move { Ok(get_message_url(g.message_id)) })
                .unwrap_or_else(|_| "未找到！".to_owned())
        }))
        .await
        .join("\n"),
    };
    Ok(reply_to!(bot, message, text)
        .disable_web_page_preview(true)
        .await?)
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
    if user.id.0 != 777000 {
        return false;
    }
    // 判断是否是新本子的发布信息
    message
        .text()
        .map(|s| s.contains("原始地址"))
        .unwrap_or(false)
}

pub async fn message_handler(message: Message, bot: Bot) -> Result<()> {
    use RuaCommand::*;

    trace!("{:#?}", message);

    // 如果是新本子上传的消息，则回复投票并取消置顶
    if is_new_gallery(&message) && message.is_from_my_group() {
        on_new_gallery(bot.clone(), &message)
            .await
            .log_on_error()
            .await;
    }

    // 其他命令
    let mut to_delete = vec![message.id];
    let cmd = RuaCommand::parse(bot.clone(), &message, &CONFIG.telegram.bot_id).await;
    match &cmd {
        Err(CommandError::WrongCommand(help)) => {
            warn!("错误的命令：{}", help);
            if !help.is_empty() {
                let msg = reply_to!(bot, message, *help).await?;
                to_delete.push(msg.id);
            } else {
                bot.delete_message(message.chat.id, message.id).await?;
            }
        }
        Ok(Ping) => {
            info!("执行命令：ping");
            let msg = reply_to!(bot, message, "pong").await?;
            to_delete.push(msg.id);
        }
        Ok(Full(g)) => {
            to_delete.push(cmd_full(bot.clone(), &message, g).await?.id);
        }
        Ok(Delete) => {
            to_delete.push(cmd_delete(bot.clone(), &message, false).await?.id);
        }
        Ok(RealDelete) => {
            to_delete.push(cmd_delete(bot.clone(), &message, true).await?.id);
        }
        Ok(Upload(urls)) => {
            to_delete.push(cmd_upload(bot.clone(), &message, urls).await?.id);
        }
        Ok(UpdateTag(g)) => {
            to_delete.push(cmd_update_tag(bot.clone(), &message, g).await?.id);
        }
        Ok(Query(gs)) => {
            cmd_query(bot.clone(), &message, gs).await?;
        }
        Ok(Best([from, to])) => {
            to_delete.push(cmd_best(bot.clone(), &message, *from, *to).await?.id);
        }
        Ok(ReUpload(g)) => {
            to_delete.push(cmd_reupload(bot.clone(), &message, g).await?.id);
        }
        // 收到无效命令则立即返回
        Err(CommandError::NotACommand) => return Ok(()),
    }

    // 对 query 和 best 命令的调用保留
    if matches!(cmd, Ok(Query(_))) {
        to_delete.clear();
    }
    // 没有直接回复画廊的 upload full update_tag 则保留
    if matches!(cmd, Ok(Upload(_)) | Ok(Full(_)) | Ok(UpdateTag(_)))
        && message.reply_to_gallery().is_none()
    {
        to_delete.clear();
    }

    // 定时删除群组内的 BOT 消息
    if !to_delete.is_empty() && message.is_from_my_group() {
        let chat_id = message.chat.id;
        tokio::spawn(async move {
            sleep(time::Duration::from_secs(300)).await;
            for id in to_delete {
                info!("清除消息 {:?}", id);
                bot.delete_message(chat_id, id).await.log_on_error().await;
            }
        });
    }
    Ok(())
}

pub async fn poll_handler(poll: Poll, _bot: Bot) -> Result<()> {
    let options = poll.options;
    let votes = options.iter().map(|s| s.voter_count).collect::<Vec<_>>();
    let score = Vote::wilson_score(&votes);
    let votes = serde_json::to_string(&votes)?;
    info!("收到投票：{} -> {}", poll.id, score);
    DB.update_score(&poll.id, score, votes)
}

pub async fn inline_handler(query: InlineQuery, bot: Bot) -> Result<()> {
    let text = query.query.trim();
    info!("行内查询：{}", text);
    let mut answer = vec![];
    if EXHENTAI_URL.is_match(text) {
        if let Ok(v) = DB.query_gallery_by_url(&query.query) {
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
    bot.answer_inline_query(query.id, answer).await?;
    Ok(())
}

fn split_vec<T: FromStr>(s: &str) -> std::result::Result<Vec<T>, T::Err> {
    s.split(' ')
        .map(T::from_str)
        .collect::<std::result::Result<Vec<_>, _>>()
}

async fn callback_change_page(bot: Bot, message: &Message, cmd: &str, data: &str) -> Result<()> {
    info!("翻页：{:?} {}", message.id, cmd);
    // vec![from, to, offset]
    let data = split_vec::<i64>(data)?;
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
    bot.edit_message_text(message.chat.id, message.id, &text)
        .parse_mode(ParseMode::Html)
        .reply_markup(reply)
        .await?;
    Ok(())
}

async fn callback_poll(bot: Bot, message: &Message, user_id: u64, data: &str) -> Result<()> {
    let data = split_vec::<i32>(data)?;
    let [poll_id, option] = match TryInto::<[i32; 2]>::try_into(data) {
        Ok(v) => v,
        _ => return Ok(()),
    };
    DB.insert_vote(user_id, poll_id, option)?;
    let votes = Vote::new(DB.query_vote(poll_id)?);
    let reply = poll_keyboard(poll_id, &votes);
    let score = votes.score();
    let ret = bot
        .edit_message_text(message.chat.id, message.id, votes.info())
        .reply_markup(reply)
        .await;
    // 用户可能会点多次相同选项，此时会产生一个 MessageNotModified 的错误
    match ret {
        Err(RequestError::Api(ApiError::MessageNotModified)) => Ok(()),
        _ => ret.map(|_| ()),
    }?;
    DB.update_score(&poll_id.to_string(), score, serde_json::to_string(&*votes)?)?;
    info!("收到投票：[{}] {} -> {}", user_id, poll_id, score);
    Ok(())
}

pub async fn callback_handler(callback: CallbackQuery, bot: Bot) -> Result<()> {
    debug!("回调：{:?}", callback.data);

    if let Some(d) = LIMIT.insert(callback.from.id.0) {
        warn!("用户 {} 操作频率过高", callback.from.id.0);
        bot.answer_callback_query(callback.id)
            .text(format!("操作频率过高，请 {} 秒后再尝试", d.as_secs()))
            .show_alert(true)
            .await?;
        return Ok(());
    }

    let (cmd, data) = match callback.data.as_ref().and_then(|v| v.split_once(' ')) {
        Some(v) => v,
        None => return Ok(()),
    };

    let message = match callback.message {
        Some(v) => v,
        None => {
            bot.answer_callback_query(callback.id)
                .text("该消息过旧")
                .show_alert(true)
                .await?;
            return Ok(());
        }
    };

    tokio::spawn({
        let id = callback.id;
        let bot = bot.clone();
        async move {
            bot.answer_callback_query(id).await.log_on_error().await;
        }
    });

    match cmd {
        "<<" | ">>" | "<" | ">" => {
            callback_change_page(bot, &message, cmd, data).await?;
        }
        "vote" => {
            callback_poll(bot, &message, callback.from.id.0, data).await?;
        }
        _ => warn!("未知指令：{}", cmd),
    };

    Ok(())
}
