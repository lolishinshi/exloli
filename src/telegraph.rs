use failure::Error;
use json::JsonValue;
use reqwest::{multipart::Form, Client, StatusCode};
use std::{collections::HashMap, io};
use tempfile::NamedTempFile;

#[derive(Debug)]
pub struct Telegraph {
    access_token: String,
}

impl Telegraph {
    pub fn new(access_token: &str) -> Self {
        Self {
            access_token: access_token.to_owned(),
        }
    }

    /// 通过 URL 上传图片至 telegraph
    pub fn upload_by_url(url: &str) -> Result<JsonValue, Error> {
        let client = Client::new();
        // 下载图片
        debug!("下载图片: {}", url);
        let mut file = NamedTempFile::new()?;
        let mut response = client.get(url).send()?;
        io::copy(&mut response, &mut file)?;

        // 上传图片
        debug!("上传图片: {:?}", file.path());
        let form = Form::new().file("file", file.path())?;
        let mut response = client
            .post("https://telegra.ph/upload")
            .multipart(form)
            .send()?;
        let json = json::parse(&response.text()?)?;
        debug!("结果: {}", json);

        Ok(json[0].clone())
    }

    /// 创建页面
    pub fn create_page(&self, title: &str, content: &str) -> PageBuilder {
        PageBuilder {
            access_token: self.access_token.to_owned(),
            title: title.to_owned(),
            content: content.to_owned(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Default)]
pub struct PageBuilder {
    access_token: String,
    title: String,
    content: String,
    author_name: Option<String>,
    author_url: Option<String>,
    return_content: Option<bool>,
}

impl PageBuilder {
    pub fn author_name(mut self, author_name: &str) -> Self {
        self.author_name = Some(author_name.to_owned());
        self
    }

    pub fn author_url(mut self, author_url: &str) -> Self {
        self.author_url = Some(author_url.to_owned());
        self
    }

    pub fn return_content(mut self, return_content: bool) -> Self {
        self.return_content = Some(return_content);
        self
    }

    /// 发布文章, 返回 Page Object: https://telegra.ph/api#Page
    pub fn publish(self) -> Result<JsonValue, Error> {
        let mut params = HashMap::new();
        params.insert("access_token", self.access_token.to_owned());
        params.insert("title", self.title.to_owned());
        params.insert("content", self.content.to_owned());
        if let Some(author_name) = &self.author_name {
            params.insert("author_name", author_name.to_owned());
        }
        if let Some(author_url) = &self.author_url {
            params.insert("author_url", author_url.to_owned());
        }
        if let Some(return_content) = &self.return_content {
            params.insert("return_content", return_content.to_string());
        }

        let mut response = Client::new()
            .post("https://api.telegra.ph/createPage")
            .form(&params)
            .send()?;
        if response.status() != StatusCode::OK {
            return Err(format_err!("Status Code: {}", response.status()));
        }
        let text = response.text()?;
        debug!("text: {}", text);
        let mut json = json::parse(&text)?;
        if json["ok"] != true {
            return Err(format_err!("error: {}", json));
        }
        Ok(json.remove("result"))
    }
}

#[cfg(test)]
mod tests {
    use crate::telegraph::Telegraph;
    use reqwest::Client;

    #[test]
    fn upload() {
        let result = Telegraph::upload_by_url(
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png",
        );
        println!("{:?}", result);
    }

    #[test]
    fn publish() {
        // Sample TOKEN
        let result = Telegraph::new("b968da509bb76866c35425099bc0989a5ec3b32997d55286c657e6994bbb")
            .create_page(
                "Sample Page",
                r#"[{"tag":"p","children":["Hello, world!"]}]"#,
            )
            .author_name("exloli")
            .author_url("https://t.me/exlolicon")
            .publish();
        println!("{:?}", result);
    }
}
