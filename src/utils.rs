use crate::trans::TRANS;
use std::borrow::Cow;
use std::str::FromStr;

/// 将图片地址格式化为 html
pub fn img_urls_to_html(img_urls: &[String]) -> String {
    img_urls
        .iter()
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
    let trans = |k: &str, s: &str| -> String {
        // 形如 "usashiro mani | mani" 的 tag 只需要取第一部分翻译
        let name = s.split(" | ").next().unwrap();
        let mut s = TRANS.trans(k, s).to_owned();
        if s == name {}
        for (from, to) in replace_table.iter() {
            s = s.replace(from, to);
        }
        format!("#{}", s)
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
pub fn get_id_from_image(url: &str) -> (i32, i32) {
    let tmp = url.split('/').nth(5).unwrap();
    let ids = tmp
        .split('-')
        .map(i32::from_str)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    (ids[0], ids[1])
}
