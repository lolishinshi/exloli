use anyhow::Error;
use reqwest::Client;
use serde::de::DeserializeOwned;
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

    async fn make_request<T, M>(&self, method: &M) -> Result<T, Error>
    where
        T: DeserializeOwned,
        M: Method,
    {
        let response = self
            .client
            .get(&M::url(&self.token))
            .json(&method)
            .send()
            .await?;
        let result: Result<T, ApiError> = response.json::<TelegramResult<T>>().await?.into();
        Ok(result?)
    }

    pub async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        url: &str,
    ) -> Result<Message, Error> {
        let button = ReplyMarkup::InlineKeyboard(InlineKeyboardMarkup {
            inline_keyboard: vec![vec![InlineKeyboardButton {
                text: "原始地址".to_owned(),
                pressed: InlineKeyboardButtonPressed::Url(url.to_owned()),
            }]],
        });
        let message = SendMessage::new(ChatTarget::username(chat_id), text)
            .parse_mode(ParseMode::HTML)
            .reply_markup(button);

        self.make_request::<Message, _>(&message).await
    }
}
