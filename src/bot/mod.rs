mod command;
mod handler;
mod utils;

use crate::{config, BOT};
use command::{PrivilegedCommand, PublicCommand};
use handler::*;
use teloxide::prelude::*;

pub async fn start_bot(bot: AutoSend<Bot>, config: config::Telegram) {
    info!("BOT 启动");

    let handler = dptree::entry()
        .branch(
            dptree::entry()
                .filter_command::<PublicCommand>()
                .endpoint(message_handler),
        )
        .branch(
            dptree::entry()
                .filter_command::<PrivilegedCommand>()
                .endpoint(message_handler),
        )
        .branch(Update::filter_poll().endpoint(poll_handler))
        .branch(Update::filter_inline_query().endpoint(inline_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![config])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
