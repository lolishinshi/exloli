mod command;
mod handler;
mod utils;

use crate::BOT;
use handler::*;
use teloxide::prelude::*;
use tokio_stream::wrappers::UnboundedReceiverStream;

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
