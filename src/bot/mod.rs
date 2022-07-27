mod command;
mod handler;
mod utils;

use crate::BOT;
use handler::*;
use teloxide::prelude::*;

pub async fn start_bot(bot: AutoSend<Bot>) {
    info!("BOT 启动");

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .branch(Message::filter_text().endpoint(message_handler))
        )
        .branch(Update::filter_poll().endpoint(poll_handler))
        .branch(Update::filter_inline_query().endpoint(inline_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
