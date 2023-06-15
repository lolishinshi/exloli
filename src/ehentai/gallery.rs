use crate::ehentai::EHentaiClient;
use reqwest::Client;

/// 画廊信息
#[derive(Debug)]
pub struct Gallery {
    pub(super) client: EHentaiClient,
    /// 画廊标题
    pub title: String,
    /// 画廊日文标题
    pub title_jp: Option<String>,
    /// 画廊地址
    pub url: String,
    /// 父画廊地址
    pub parent: Option<String>,
    /// 标签
    pub tags: Vec<(String, Vec<String>)>,
    /// 图片页面的地址
    pub pages: Vec<String>,
}
