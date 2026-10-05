# 域名部署详细教程（DNS + 反代 + HTTPS + announce）

> 服务已经按 [建站手把手教程](build-tutorial.md) 在服务器上跑起来、用 IP 能访问之后，这篇教你怎么用**自己的域名 + HTTPS** 正式对外。每一步都给命令和可直接抄的 Nginx 配置。不懂的词查 [术语小词典](glossary.md)。

## 这篇解决什么问题

现在你大概是用 `http://服务器IP:3000` 访问的。正式开站要换成 `https://你的域名.com`：用户好记、浏览器不报不安全、tracker 走加密 announce（passkey 不会被明文截获）。核心就三件事：**域名解析 → 反代 + 证书 → 把 announce 地址改成域名**。

---

## TL;DR

1. 域名商后台加 **A 记录**指向服务器公网 IP；
2. 开放 **80 / 443** 端口（云安全组 + 系统防火墙两层）；
3. 装 Nginx，**443 反代**到 3000(web)/8080(api)/7070(tracker)，用 certbot 申请 Let's Encrypt 证书；
4. 后台「站点设定 → 基础设定 → Tracker 地址」填 `https://你的域名.com`（根地址即可，系统自动拼 `/announce/<passkey>`）；
5. 验证：浏览器开 `https://你的域名.com/setup`、下载一颗种子确认能连 tracker。

---

## 第 1 步：域名解析（DNS）

登录你买域名的平台（阿里云/Cloudflare/Namecheap 等），做两件事：

| 记录类型 | 主机名 | 指向 | 说明 |
| :--- | :--- | :--- | :--- |
| **A** | `@`（或 `www`） | 服务器**公网 IPv4** | 主域名指向机器 |
| **AAAA**（可选） | `@` | 服务器 IPv6 | 有 IPv6 才加 |

> 要不要单独的 `tracker.你的域名.com` 子域？**不用**。tracker 和网站用同一个域名即可，靠 Nginx 路径区分（下一步）。省一个子域就少一份证书管理。

**怎么算生效：** 终端跑 `ping 你的域名.com` 能解析到你的服务器 IP；或用 `dig 你的域名.com +short`。DNS 生效有几分钟到几小时的传播时间（看 TTL），改完耐心等。

---

## 第 2 步：开放端口（防火墙 / 安全组）

**两层都要开**（云厂商「安全组」+ 服务器自身防火墙如 ufw/firewalld）：

| 端口 | 协议 | 用途 | 是否必须 |
| :--- | :--- | :--- | :--- |
| **80** | TCP | HTTP（证书申请验证 + 跳转 HTTPS） | 必须 |
| **443** | TCP | HTTPS（用户访问 + tracker announce） | 必须 |
| 7070 | TCP | tracker HTTP（**走 443 反代时不需公网开**） | 走 443 则不必 |
| 6969 | UDP | tracker UDP（**默认不启用**，私有站用 HTTP announce 足够） | 通常不必 |

> 关键点：BT 客户端是通过你的 **443 端口**（经 Nginx）连 tracker 的，**不是**直连 7070。所以只要你把 announce 走 HTTPS（推荐），**7070/6969 都不需要对公网开放**，只让 Nginx 在本地转发即可。更安全、端口更少。

---

## 第 3 步：反向代理 + HTTPS（核心）

### 3.1 装 Nginx

```bash
# Debian/Ubuntu
sudo apt update && sudo apt install -y nginx
# CentOS
sudo dnf install -y nginx && sudo systemctl enable --now nginx
```

### 3.2 写入站点配置

新建 `/etc/nginx/sites-available/fluxtorrent.conf`（或 `/etc/nginx/conf.d/fluxtorrent.conf`），内容**整段复制**，把 `pt.example.com` 换成你的域名：

```nginx
# 边缘限流（tracker 防启动风暴；阈值远高于正常 1800s 汇报间隔）
limit_req_zone  $binary_remote_addr zone=ann_perip:10m rate=60r/s;
limit_conn_zone $binary_remote_addr zone=ann_conn:10m;

server {
    listen 80;
    server_name pt.example.com;
    # 证书申请期间允许 ACME 验证；平时这条会把所有流量跳 HTTPS
    location /.well-known/acme-challenge/ { root /var/www/certbot; }
    location / { return 301 https://$host$request_uri; }
}

server {
    listen 443 ssl http2;
    server_name pt.example.com;

    ssl_certificate     /etc/letsencrypt/live/pt.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/pt.example.com/privkey.pem;

    client_max_body_size 32m;   # 种子上传（.torrent 文件）

    # tracker announce：放最前，优先匹配 /announce/，转发到本地 7070
    location /announce/ {
        limit_req  zone=ann_perip burst=120 nodelay;
        limit_conn ann_conn 120;
        limit_req_status 429;
        proxy_pass http://127.0.0.1:7070;
        proxy_set_header Host $host;
    }

    # 后台接口
    location /api/ { proxy_pass http://127.0.0.1:8080; }

    # 网站（页面 + 静态资源 + 同源 /api 转发都由 web 容器处理）
    location /_next/ { proxy_pass http://127.0.0.1:3000; }
    location /       { proxy_pass http://127.0.0.1:3000; }
}
```

启用并校验：

```bash
sudo ln -s /etc/nginx/sites-available/fluxtorrent.conf /etc/nginx/sites-enabled/ 2>/dev/null
sudo nginx -t            # 显示 syntax is ok 才算通过
sudo systemctl reload nginx
```

### 3.3 申请 HTTPS 证书（Let's Encrypt，免费）

```bash
sudo apt install -y certbot python3-certbot-nginx
sudo certbot --nginx -d pt.example.com
```

按提示填邮箱、同意条款，证书会自动签发并写入上面配置的 `/etc/letsencrypt/live/pt.example.com/` 路径。certbot 还会自动把 80→443 跳转和证书续期都配好。

> **用 1Panel / 宝塔？** 面板里「网站 → 创建网站 → 反向代理」填 `http://127.0.0.1:3000`，再在网站设置里「申请 Let's Encrypt 证书 + 强制 HTTPS」即可，不用手敲 Nginx（详见 [面板部署](panel-deploy.md)）。但 `/announce/` → 7070 这条 tracker 反代面板不会自动加，仍需在面板的高级配置里补上那段 `location /announce/`。

---

## 第 4 步：把 announce 地址改成域名（关键）

Nginx 起好后，去后台改 tracker 地址，让种子里的 announce 变成你的域名：

1. 浏览器开 `https://你的域名.com/admin`（或装完向导直接点「去管理后台」）；
2. **站点设定 → 基础设定 → Tracker 地址**，填：

   ```
   https://pt.example.com
   ```

   ⚠️ **填根地址就行，不要手填 `/announce`**。系统会自动在后面拼 `/announce/<你的passkey>`；就算你手滑写了 `https://pt.example.com/announce`，代码也会自动剥掉多余的 `/announce`（这个坑历史上出过 P0 双路径 bug，现已修复并兜底）。

3. **（推荐）HTTPS announce**：同一页还有「HTTPS announce 地址」，也填 `https://pt.example.com`。填了之后，发出的种子会走 **BEP12 双 tier**——`https` 首选 + `http` 回退，passkey 加密传输，老客户端连不上 https 也能自动降级。

4. **改完后，已经发出去的旧种子要重新下载**才带新地址（announce 是烧进 `.torrent` 文件里的，不会自动变）。

**怎么验证 announce 正确：**
- 发一颗测试种子（或后台找一颗），用文本编辑器打开下载的 `.torrent`，搜 `announce`，应看到 `https://pt.example.com/announce/xxxxxxxx`；
- 或更直接：下载种子后用客户端做种，看能否连上 tracker（做种一小时魔力正常入账，见 [开站第一周](playbook/01-first-week.md)）。

---

## 第 5 步：CORS 等收尾

- 编辑服务器上的 `docker/.env`，确保有：`CORS_ORIGINS=https://pt.example.com`（多个域名逗号分隔）。改完 `docker compose up -d` 重启生效。
- WebAuthn（两步验证）的域名会自动取 announce 地址的 host，不用单独设；
- 如果之前向导里 announce 填的是 IP，现在改成域名后，记得回向导结束页或后台确认「开站检查清单」里 announce 不再是本地回环。

---

## 第 6 步：验证全链路

```bash
curl https://pt.example.com/api/v1/health    # 应返回 {"status":"up"}
```

浏览器访问 `https://pt.example.com`：
- 地址栏显示小锁（HTTPS 生效）；
- 页面正常、能登录；
- 下载一颗种子，客户端能连上 tracker（做种统计走动）。

到这，你的站就是正式的公网 HTTPS 站了。

---

## 进阶：多域名（一主一备 / 防封备用域名）

支持给同一个站配多个访问域名（比如 `pt.example.com` 主用、`pt-backup.net` 备用防封）。**系统层面原生支持**——CORS 白名单就是逗号分隔设计、登录 cookie 不绑定域名、内部容器链路不感知域名；要做的只是把每个域名都正确接到 Nginx 并加进白名单。

三步：

**① DNS + Nginx**：每个域名的 DNS 都解析到本服务器；Nginx 站点配置的 `server_name` 写成多值：

```nginx
server_name pt.example.com pt-backup.net;
```

（宝塔/1Panel 用户：网站设置里把第二域名加进「域名列表」，`/announce/` 反代段两个域名共用同一份配置所以自动生效。）

**② 证书**：certbot 一张多域证书，或给第二域名单签一张：

```bash
sudo certbot --nginx -d pt.example.com -d pt-backup.net
```

**③ CORS 白名单（唯一必改的后端配置）**：编辑 `docker/.env`：

```bash
CORS_ORIGINS=https://pt.example.com,https://pt-backup.net
```

改完 `docker compose up -d` 重启。漏了这步的话，备用域名下登录/保存类请求会被浏览器 CORS 拦掉。

**两个如实的边界**（不是坑，是口径）：

- **种子里的 tracker 地址默认只有一个**（可配多 tracker，见下）：`announce_url` 是单值，烧进 `.torrent` 文件。
- **RSS / 分享链接统一指向主域名**：`PUBLIC_SITE_URL` 是单值，RSS 输出、og:url 等规范地址都用主域名。对私站通常无所谓（内容本来就不给搜索引擎）。

**多 tracker（可选进阶）**：想让每个域名**都**当 tracker 用、种子同时携带多个地址轮换，去后台「Tracker URL 管理」（管理工具 → trackers）把备用域名的根地址（如 `https://pt-backup.net`）加一条、启用并设好优先级——之后新生成的 `.torrent` 会在 announce-list 里自动带上所有已启用地址（BEP12 多 tier，客户端自动轮换）。安全护栏：纯内网地址（127.0.0.1/localhost）、不带 `http(s)://`/`udp://` 前缀的裸域名、与主 announce 重复的地址都会被自动跳过，不会误进公网种子。注意旧种子不回填，重新下载才会带新 tier。

---

## 常见问题

| 现象 | 原因 | 怎么办 |
| :--- | :--- | :--- |
| 浏览器打不开 443 | 证书路径错 / 防火墙 443 没开 | `nginx -t` 看报错；确认安全组+系统防火墙都开了 443 |
| 页面能开但种子连不上 tracker | Nginx 没代理 `/announce/` 到 7070 | 检查 3.2 的 `location /announce/` 段，reload nginx |
| 改了 announce 还是 127.0.0.1 | 后台值没改 / 旧种子缓存 | 确认后台是域名；已发种子重新下载 |
| announce 报 404/421 | announce_url 拼错或带了多余端口 | 填根地址 `https://域名`，不要带 `:7070`、不要带 `/announce` |
| 要不要开 6969/UDP | 默认不需要 | 私有站走 HTTP(S) announce 即可（行业惯例）；除非你显式配了 `TRACKER_UDP_URL` |
| certbot 报错「无法验证域名」 | 80 端口没开 / DNS 未生效 | 确认 80 开着、域名已解析到本机 IP，等 DNS 传播 |
| 备用域名登录/保存报跨域错误 | 第二域名没进 CORS 白名单 | `docker/.env` 的 `CORS_ORIGINS` 逗号追加，`up -d` 重启 |

---

词看不懂？→ [术语小词典](glossary.md)
