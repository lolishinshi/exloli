use failure::Error;
use reqwest::{multipart::Form, Client, StatusCode};
use serde::Deserialize;
use std::io;
use tempfile::NamedTempFile;

/// 图片上传结果
#[derive(Debug, Deserialize)]
pub struct UploadResult {
    /// 图片 URL, 为相对 "telegra.ph" 的地址
    pub src: String,
}

/// 通过 URL 上传图片至 telegraph
pub fn upload_by_url(url: &str) -> Result<Vec<UploadResult>, Error> {
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
    let json = response.json()?;
    debug!("结果: {:?}", json);

    Ok(json)
}

/// 发布文章, 返回文章地址
pub fn publish_article(
    access_token: &str,
    title: &str,
    author_name: &str,
    author_url: &str,
    content: &str,
) -> Result<String, Error> {
    let client = Client::new();

    let mut response = client
        .post("https://api.telegra.ph/createPage")
        .form(&[
            ("access_token", access_token),
            ("title", title),
            ("author_name", author_name),
            ("author_url", author_url),
            ("content", content),
        ])
        .send()?;
    let text = response.text()?;
    if response.status() != StatusCode::OK {
        return Err(format_err!("{}", text));
    }
    let json = json::parse(&text)?;
    Ok(json["result"]["url"].to_string())
}

#[cfg(test)]
mod tests {
    use crate::telegraph::{publish_article, upload_by_url};
    use reqwest::Client;

    #[test]
    fn upload() {
        let result = upload_by_url(
            "https://www.google.com/images/branding/googlelogo/2x/googlelogo_color_272x92dp.png",
        );
        println!("{:?}", result);
    }

    #[test]
    fn publish() {
        let result = publish_article(
            "b968da509bb76866c35425099bc0989a5ec3b32997d55286c657e6994bbb",
            "Sample Page",
            "Page",
            "https://t.me/exlolicon",
            r#"[{"tag":"p","children":["Hello, world!"]}]"#,
        );
        println!("{:?}", result);
    }
}
