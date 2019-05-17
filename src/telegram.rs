use failure::{format_err, Error};
use reqwest::Client;

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

    pub fn send_message(&self, chat_id: &str, text: &str) -> Result<(), Error> {
        let mut response = self
            .client
            .get(&format!(
                "https://api.telegram.org/bot{}/sendMessage",
                self.token,
            ))
            .query(&[("chat_id", chat_id), ("text", text), ("parse_mode", "HTML")])
            .send()?;
        let json = json::parse(&response.text()?)?;
        if json["ok"] == true {
            Ok(())
        } else {
            Err(format_err!("{}", json))
        }
    }
}
