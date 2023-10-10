# exloli

**项目转移到 [exloli-next](https://github.com/lolishinshi/exloli-next)**

从 E 站里站下载指定关键词的画(ben)廊(zi)并上传到 Telegraph 并发布到 Telegram 频道

## 安装

### Docker

参见 docker-compose.yml

### 手动编译

#### 编译前的准备

编译环境：Rust

依赖：libxml2、gcc、sqlite3

#### 在 Linux Server 下编译安装

在终端上执行：

``` 
[user@host ~]$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

[user@host ~]$ source $HOME/.cargo/env

[user@host ~]$ cargo install --git https://github.com/lolishinshi/exloli

[user@host ~]$ cd && mkdir -p Document/exloli

[user@host ~]$ [包管理系统|yum|apt|pacman|...] screen -y

[user@host ~]$ screen -S exloli

[user@host ~]$ cd Document/exloli
```

## 用法

### 使用 exloli bot 前的准备

1. 创建 Telegram Channel, 并设为公开.
   私有 Channel 需要手动获取 chat id, 方法: 邀请 `@get_id_bot` 到 Channel 中, 然后发送 `/my_id@get_id_bot`
2. 在@BotFather创建 Telegram Bot, 记录 TOKEN, 并拉进 Channel
3. 创建 Telegraph 账号, 记录 TOKEN. 创建方法: 访问 `https://api.telegra.ph/createAccount?short_name={}&author_name={}&author_url={}`
4. 在`*/exloli`目录下建立 config.toml, 并将 仓库中的 db.text.json 复制一份，然后运行 exloli

### exloli 服务

```
exloli #启动exloli服务
exloli --debug #启动exloli，模式为调试
```

#### Bot指令(commit 2c01fcd)

```
/ping - 测试存活
/upload - 上传画廊
/full - 上传画廊的完整版本
/delete - 删除画廊
/real_delete - 彻底删除画廊(调试用命令)
/query - 查询画廊
/best - 获取第 $1 ~ $2 天间的画廊排行
/uptag - 更新画廊tag
```

#### 使用 exloli bot 的权限判断

需满足以下条件：

1. 是否为频道管理员
2. 是否为频道关联讨论组的管理员

## 模板

config.toml 模板如下

```toml
# 日志等级, 可选 INFO, DEBUG, ERROR. 默认 INFO
log_level = "INFO"
# 图片下载并发数. 默认 4
threads_num = 4
# 每隔多少秒检查一次，默认一小时
interval = 3600
# 数据库储存位置
database_url = "db.sqlite"

[exhentai]
# E 站用户名
username = "username"
# E 站密码
password = "password"
# 可选, 使用 cookie 登录
cookie = "ipb_member_id=xx; ipb_pass_hash=xx; igneous=xx;"
# 搜索 URL
search_url = "https://exhentai.org"
# 搜索参数
search_params = [
    ["f_cats", "704"],
    ["f_search", "female:lolicon language:Chinese"]
]
# 上传前多少页的本子, 重复的会自动过滤
max_pages = 2
# 最大展示的图片数量
max_img_cnt = 50
# 超过多少天后更新的本子将重新发送消息
outdate = 14
# [可选] 代理
proxy = "socks5://127.0.0.1:1234"

[telegraph]
# telegraph 账号 token
access_token = "TOKEN"
# 作者名称
author_name = "exloli"
# 作者地址(通常为频道链接)
author_url = "https://t.me/exlolicon"
# [可选] 代理
proxy = "socks5://127.0.0.1:1234"

[telegram]
# telegram 频道 ID, 公共频道直接 @+频道名, 私有频道需要需要获取数字格式的 id
channel_id = "@exlolicon"
# 机器人 token
token = "TOKEN"
# 机器人 ID
bot_id = "@crypko_bot"
# telegram 频道对应讨论组的 ID，暂时只能为数字
group_id = -2147483647
# 受信用户，拥有除了删本以外的权限
trusted_users = ["test"]
```
