use failure::{format_err, Error};
use reqwest::Client;
use telegram_types::bot::types;

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

    pub fn send_message(&self, chat_id: &str, text: &str, url: &str) -> Result<(), Error> {
        let button = types::InlineKeyboardMarkup {
            inline_keyboard: vec![vec![types::InlineKeyboardButton {
                text: "原始地址".to_owned(),
                pressed: types::InlineKeyboardButtonPressed::Url(url.to_owned()),
            }]],
        };

        let mut response = self
            .client
            .get(&format!(
                "https://api.telegram.org/bot{}/sendMessage",
                self.token,
            ))
            .query(&[
                ("chat_id", chat_id),
                ("text", text),
                ("parse_mode", "HTML"),
                ("reply_markup", &*serde_json::to_string(&button).unwrap()),
            ])
            .send()?;
        let json = json::parse(&response.text()?)?;
        if json["ok"] == true {
            Ok(())
        } else {
            Err(format_err!("{}", json))
        }
    }
}
