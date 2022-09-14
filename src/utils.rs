use crate::trans::TRANS;
use crate::CONFIG;
use anyhow::Context;
use futures::TryFutureExt;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::*;
use reqwest::{Client, Response};
use std::borrow::Cow;
use std::io::Write;
use std::time::SystemTime;
use tempfile::NamedTempFile;

pub static HOST: Lazy<&'static str> = Lazy::new(|| {
    CONFIG
        .exhentai
        .search_url
        .host_str()
        .expect("failed to extract host from search_url")
});

/// 将图片地址格式化为 html
pub fn img_urls_to_html(img_urls: &[String]) -> String {
    img_urls
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| format!(r#"<img src="{}">"#, s))
        .collect::<Vec<_>>()
        .join("")
}

/// 左填充空格
fn pad_left(s: &str, len: usize) -> Cow<str> {
    let width = unicode_width::UnicodeWidthStr::width(s);
    if width >= len {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(" ".repeat(len - width) + s)
    }
}

/// 将 tag 转换为可以直接发送至 tg 的文本格式
pub fn tags_to_string(tags: &[(String, Vec<String>)]) -> String {
    let replace_table = vec![
        (" ", "_"),
        ("_|_", " #"),
        ("-", "_"),
        ("/", "_"),
        ("·", "_"),
    ];
    let trans = |namespace: &str, string: &str| -> String {
        // 形如 "usashiro mani | mani" 的 tag 只需要取第一部分翻译
        let to_translate = string.split(" | ").next().unwrap();
        let mut result = TRANS.trans(namespace, to_translate).to_owned();
        // 没有翻译的话，还是使用原始字符串
        if result == to_translate {
            result = string.to_owned();
        }
        for (from, to) in replace_table.iter() {
            result = result.replace(from, to);
        }
        format!("#{}", result)
    };
    let mut ret = vec![];
    for (k, v) in tags {
        let v = v.iter().map(|s| trans(k, s)).collect::<Vec<_>>().join(" ");
        ret.push(format!(
            "<code>{}</code>: {}",
            pad_left(TRANS.trans("rows", k), 6),
            v
        ))
    }
    ret.join("\n")
}

/// 从 e 站 url 中获取数字格式的 id，第二项为 token
pub fn get_id_from_gallery(url: &str) -> (i32, String) {
    let url = url.split('/').collect::<Vec<_>>();
    (url[4].parse::<i32>().unwrap(), url[5].to_owned())
}

/// 从图片 url 中获取数字格式的 id，第一个为 id，第二个为图片序号
/// 图片格式示例：
/// https://bhoxhym.oddgxmtpzgse.hath.network/h/33f789fab8ecb4667521e6b1ad3b201936a96415-382043-1280-1817-jpg/keystamp=1619024700-fff70cfa32;fileindex=91876552;xres=2400/00000000.jpg
pub fn get_id_from_image(url: &str) -> Option<i32> {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"fileindex=(\d+)").unwrap());
    let caps = RE.captures(url)?;
    caps.get(1).and_then(|s| s.as_str().parse::<i32>().ok())
}

/// 提取图片哈希，此处为原图哈希的前十位
/// 链接示例：https://exhentai.org/s/03af734602/1932743-1
pub fn get_hash_from_image(url: &str) -> Option<&str> {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"/s/([0-9a-f]+)/").unwrap());
    let caps = RE.captures(url)?;
    caps.get(1).map(|s| s.as_str())
}

/// 根据消息 id 生成当前频道的消息直链
pub fn get_message_url(id: i32) -> String {
    format!("https://t.me/{}/{}", CONFIG.telegram.channel_id, id)
        .replace("/-100", "/")
        .replace('@', "")
}

pub fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("您穿越了？")
        .as_secs()
}

pub fn extract_telegraph_path(s: &str) -> &str {
    s.split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .unwrap()
}

pub async fn download_to_temp(client: &Client, url: &str) -> anyhow::Result<NamedTempFile> {
    let bytes = client
        .get(url)
        .header(CONNECTION, "keep-alive")
        .header(REFERER, "https://exhentai.org/")
        .send()
        .and_then(Response::bytes)
        .await?;
    let suffix = String::from(".") + url.rsplit_once('.').context("找不到图片后缀")?.1;
    let mut tmp = tempfile::Builder::new()
        .prefix("exloli_")
        .suffix(&suffix)
        .rand_bytes(5)
        .tempfile()?;
    tmp.write_all(bytes.as_ref())?;
    Ok(tmp)
}
