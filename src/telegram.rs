use failure::{format_err, Error};
use reqwest::Client;
use telegram_types::bot::{methods::*, types::*};

#[derive(Debug)]
pub struct Bot {
    token: String,
    client: Client,
}

impl Bot {
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_owned(),
            client: Client::new(),
        }
    }

    pub async fn send_message(&self, chat_id: &str, text: &str, url: &str) -> Result<(), Error> {
        let button = ReplyMarkup::InlineKeyboard(InlineKeyboardMarkup {
            inline_keyboard: vec![vec![InlineKeyboardButton {
                text: "原始地址".to_owned(),
                pressed: InlineKeyboardButtonPressed::Url(url.to_owned()),
            }]],
        });
        let message = SendMessage::new(ChatTarget::username(chat_id), text)
            .parse_mode(ParseMode::HTML)
            .reply_markup(button);

        let response = self
            .client
            .get(&SendMessage::url(&self.token))
            .json(&message)
            .send()
            .await?;

        let result = response.json::<TelegramResult<Message>>().await?;

        if result.ok {
            Ok(())
        } else {
            Err(format_err!(
                "{:?} {:?}",
                result.error_code,
                result.description
            ))
        }
    }
}
