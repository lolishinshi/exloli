# exloli

从 E 站下载指定关键词的画(ben)廊(zi)并上传到 Telegraph 并发布到 Telegram 频道

## 用法

1. 创建 Telegram Channel, 并设为公开 (私有 Channel 需要手动获取 chat id
2. 创建 Telegram Bot, 记录 TOKEN, 并拉进 Channel
3. 创建 Telegraph 账号, 记录 TOKEN. 创建方法: 访问 `https://api.telegra.ph/createAccount?short_name={}&author_name={}&author_url={}`
4. 在当前目录下建立 config.toml, 然后运行 exloli

模板如下

```toml
# 日志等级, 可选 INFO, DEBUG, ERROR
log_level = "INFO"
# 抓取线程
threads_num = "4"

[exhentai]
# E 站用户名
username = "username"
# E 站密码
password = "password"
# 搜索关键词
keyword = "female:lolicon language:Chinese"

[telegraph]
# telegraph 账号 token
access_token = "TOKEN"
# 作者名称
author_name = "exloli"
# 作者地址(通常为频道链接)
author_url = "https://t.me/exlolicon"

[telegram]
# telegram 频道 ID, 公共频道直接 @+频道名, 私有频道需要需要获取数字格式的 id
channel_id = "@exlolicon"
# 机器人 token
token = "TOKEN"

```

第一次启动将会默认从前两天的画廊开始抓取, 
抓取完一本本子后将会在当前目录下生成 LAST_TIME 文件,
下次抓取会一直抓取到这个时间
