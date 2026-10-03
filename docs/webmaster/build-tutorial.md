# SSH 部署方式（命令行，从买服务器到上线，一步不漏）

> 这是给「第一次建站、不知道下一步点哪」的站长的**完整从头到尾教程**：从买一台服务器、装系统装 Docker，一直到用你自己的域名 + HTTPS 公网可访问、发出第一颗种子。每一步都告诉你「做什么 / 看到什么 / 怎么算成功」。
> 只想快速过一遍看 [快速开始](quick-start.md)；域名部分想深挖原理看 [域名部署](domain-deploy.md)。不懂的词查 [术语小词典](glossary.md)。

## TL;DR（赶时间看这版）

本文分 A–G 七个阶段，**按顺序做完就是一个能上线的站**：

| 阶段         | 一句话做什么                             | 做完的标志                             |
| :----------- | :--------------------------------------- | :------------------------------------- |
| A 买服务器   | 2 核 4G 起、Ubuntu、有公网 IP、装 Docker | 能 SSH 登录、`docker --version` 有输出 |
| B 部署服务   | 拉代码、填 3 个密码、起容器              | `health` 返回 up、6 个容器 Up          |
| C 安装向导   | 浏览器开 `/setup`，填四步                | 看到「✅ 安装完成」                    |
| D 开站检查   | announce / 邮件 / 注册模式               | 三项心里有数（E 步会改 announce）      |
| E 域名 HTTPS | DNS + 防火墙 + Nginx + 证书              | `https://域名` 能开、小锁亮            |
| F 发种邀请   | 传种子、发邀请码                         | 站正式可运营                           |
| G 兜底       | 配备份、起监控                           | 出事能恢复                             |

**没耐心读全文？** 直接看 [快速开始](quick-start.md)（速通版）或跑下面的无脑脚本。

> ### 🚀 最无脑路径（不想看长教程先看这）
>
> 如果你只想「一行命令把服务器侧跑起来」，直接用官方无脑脚本 `scripts/quick-deploy.sh`：
>
> ```bash
> curl -fsSL https://github.com/gntv456/FluxTorrent/raw/master/scripts/quick-deploy.sh | sudo bash -s -- --install-docker
> ```
>
> 它会自动装 Docker（若需要）→ 克隆仓库 → 生成三个密钥 → 起栈 → 健康检查，然后让你去 `http://服务器IP:3000/setup` 走向导。
> **本篇剩下的内容就是那个脚本「没帮你做」的部分**：逐屏向导、域名 HTTPS、发种邀请。想完全掌控每一步再往下读；想偷懒就跑上面那行，然后只补 [阶段 C / E / F](build-tutorial.md#阶段-c安装向导逐屏走)。

## 你用哪种方式部署？

| 方式                   | 适用                                 | 教程                                |
| :--------------------- | :----------------------------------- | :---------------------------------- |
| **SSH 命令行**（本篇） | 习惯终端、自己掌控一切               | 你正在看这篇                        |
| **1Panel 面板**        | 想用图形面板管 Docker/反代/证书      | [1Panel 部署方式](deploy-1panel.md) |
| **宝塔面板**           | 熟悉宝塔、用网站反代 + Let's Encrypt | [宝塔部署方式](deploy-baota.md)     |

> 三种方式的**安装向导四步、开站前检查、发种邀请完全一样**，区别只在「怎么起栈」和「怎么配反代/证书」。本篇是命令行基线，面板方式在对应专文里走完同样的七阶段。

## 这篇能带你走到哪

从「什么都没有」到「`https://你的域名.com` 能打开、用户能注册能做种」，**全程不跳步**。预计 30–60 分钟，大部分时间在等镜像下载和 DNS 生效。

## 阶段总览（先有全局观）

| 阶段             | 做什么                                | 里程碑                      |
| :--------------- | :------------------------------------ | :-------------------------- |
| A 准备服务器     | 买 VPS、装系统、装 Docker             | 能 SSH 登录、有 Docker      |
| B 部署服务       | 拉代码、填 `.env`、起容器             | `health` 返回 up、6 容器 Up |
| C 安装向导       | 四步填完向导                          | 看到「✅ 安装完成」         |
| D 开站前检查     | announce / SMTP / 注册模式            | 三项都过关                  |
| E 域名 + HTTPS   | DNS、防火墙、Nginx、证书、改 announce | `https://域名` 能开、小锁亮 |
| F 发种 + 邀请    | 传种子、发邀请码                      | 站正式可运营                |
| G 上线前最后检查 | 备份、监控                            | 有兜底、看得见              |

---

# 阶段 A：准备服务器（从零开始）

## A.1 买一台 VPS

去任意云厂商（阿里云/腾讯云/UCloud/Vultr/DigitalOcean 等）买一台：

- **配置**：2 核 4G 起步（实测够千级用户）；
- **系统**：选 **Ubuntu 22.04 / 24.04 LTS**（本文命令以 Ubuntu 为准，其他发行版类似）；
- **网络**：必须有**独立公网 IPv4**（站要对外，没公网 IP 不行）；
- 记下三样：**公网 IP**、**root 密码**（或 SSH 密钥）、**登录方式**。

> 国内服务器域名需备案；不想备案可用境外机房，但访问速度看地区。这不是本教程重点。

## A.2 登录 + 基础安全（建议做）

用终端 SSH 登录（Windows 用 PuTTY / PowerShell `ssh`）：

```bash
ssh root@你的公网IP
```

登录后建议做几件安全的事（不做也能继续，但上线前最好做）：

- **改 SSH 端口 / 禁密码登录用密钥**（防爆破）——新手可暂跳过；
- **建一个普通用户 + sudo**（不用一直 root 操作）：

```bash
adduser deploy && usermod -aG sudo deploy
```

- **防火墙 `ufw` 先别急着开**：等阶段 E 确定要开 80/443 时再开，否则会把自己挡在外面。现在保持默认（多数云厂商默认只开 22）。

## A.3 装 Docker

在服务器上跑一行官方安装脚本：

```bash
curl -fsSL https://get.docker.com | sudo sh
```

装完验证（看到版本号就成功）：

```bash
docker --version
docker compose version      # 注意是 compose v2（有空格），不是老版 docker-compose
```

> **用 1Panel 面板的**：面板首次进入会自动装好 Docker 和 compose，A.3 这步可跳过，后面步骤在面板里操作即可（详见 [面板部署](panel-deploy.md)）。

**阶段 A 完成标志**：`ssh root@IP` 能登进去，且 `docker --version` 有输出。

---

# 阶段 B：部署服务

## B.1 拉代码 + 准备配置

```bash
git clone https://github.com/gntv456/FluxTorrent.git && cd FluxTorrent
# 或用 SSH：git clone git@github.com:gntv456/FluxTorrent.git && cd FluxTorrent
cp docker/.env.example docker/.env
```

用编辑器打开 `.env` 改（任选其一）：终端 `nano docker/.env`（保存 Ctrl+O 回车、退出 Ctrl+X，比 vim 对新手友好）；或装了宝塔/1Panel 的，在面板「文件」里进 `docker/` 目录双击 `.env` 在线编辑；或把文件下载到自己电脑用记事本改完再传回去。至少有三项必须填：

| 配置项           | 填什么                             | 例子                                      |
| :--------------- | :--------------------------------- | :---------------------------------------- |
| `DB_PASSWORD`    | 任意强密码                         | `Db#2026xk9`                              |
| `REDIS_PASSWORD` | 任意强密码                         | `Rd#2026xk9`                              |
| `JWT_SECRET`     | **≥32 位随机串**，别含 `change_me` | 跑 `openssl rand -base64 48` 拿一段粘进去 |

> 还有个 `CORS_ORIGINS` 现在先不管，阶段 E 配域名时再填。

## B.2 起服务

```bash
docker compose -f docker/docker-compose.yml up -d
```

## B.3 验证（别跳过）

1. 等 1–2 分钟让镜像下载完；
2. `curl http://localhost:8080/api/v1/health` → 应返回 `{"status":"up"}`；
3. `docker ps` → 应看到 **6 个容器**都是 `Up`。

> **卡住了？** 某容器一直重启 → `docker logs <容器名>`，多半是 `.env` 三项没填或 `JWT_SECRET` 太短/含 `change_me`。

**阶段 B 完成标志**：`health` 返回 up，6 容器 Up。

---

# 阶段 C：安装向导（逐屏走）

**浏览器打不开 3000 时**：web 容器默认只绑 `127.0.0.1`（安全默认）。向导期间临时对外开：`sed -i 's/^WEB_BIND=127.0.0.1/WEB_BIND=0.0.0.0/' docker/.env && docker compose -f docker/docker-compose.yml up -d`，并在云厂商安全组放行 3000/TCP。**配好域名后改回 `127.0.0.1` 并关掉 3000**，流量全走 443。

浏览器访问 **`http://你的公网IP:3000/setup`**。

你会看到标题「🚀 站点安装向导」和四步进度条：**① 选择站型 → ② 站点信息 → ③ 新手模板 → ④ 合规确认**。

### C.1 选择站型

一排卡片（综合/教育/影视/音乐/动漫/电子书/体育/游戏/软件/纪录片/无损）+ 底部「跳过」按钮。

- 拿不准 → 点「**综合**」；
- 想从零自配 → 点「**跳过**」。

> 站型决定默认分类和模块，以后随时能换。

### C.2 站点信息

输入框从上到下四个，逐个怎么填：

- **站点名称**：填你的站名（如 `魔法王国`），或留空沿用默认；
- **管理员用户名 / 密码**（页面下方那组）：**必须是预置管理员 `root` + 当前密码**——第一次安装就是 `root` / `password123`；如果上方黄框里你已经设好新密码，这里就填 `root` / 新密码。填 `admin` 之类不存在的用户名会报「凭证无效」。这组框是「向导替你登录后台」用的，不是注册新账号；
- **Tracker 公网地址（announce）**：**这步清空留空**——域名还没解析、HTTPS 还没配，现在填进去生成的种子会指向不通的地址。等阶段 E 全配好后再去后台填 `https://你的域名`。

⚠️ **第一次装，页面顶部有黄框「管理员正在使用初始密码：先设置新密码再继续」**：黄框里**新密码 / 确认新密码**两框填**相同**的 ≥8 位新密码。点「下一步」时向导一次做完三件事：用下方凭证框的密码登录 → 把密码改成黄框的新密码 → 用新密码继续安装。所以**下方凭证框永远填「当前密码」**（第一次就是 `password123`），不用自己手动换成新密码。改完记住黄框里那串新密码，以后进后台就靠它。

点「**下一步**」。

### C.3 新手运营模板（可选）

两张卡 + 跳过：

- **考核淘汰制**：高压高留存（适合老手定向站）；
- **缓冲宽进制**：宽进宽养（适合开放/内容型站）；
- 不确定先「跳过」，装完后台随时切。

点卡片或「跳过」。

### C.4 合规确认 → 完成

一个必勾框（关于内置娱乐玩法合规）。**不勾，「完成安装」是灰的。** 勾上 → 点「**完成安装**」。

完成后看到：

- 绿色「✅ 安装完成」；
- 邀请制时给一枚「首个邀请码」+ 复制按钮；
- 四个落点按钮：进入站点 / 去管理后台 / 发布第一颗种子 / 导入内容包；
- 一张「开站检查清单」警示卡。

**阶段 C 完成标志**：看到「✅ 安装完成」。

---

# 阶段 D：开站前三项检查

向导结束页的「开站检查清单」每条都能直接点进设置页：

1. **announce 还是本地回环（127.0.0.1）** → 点去「基础设定 → Tracker 地址」。阶段 E 配域名后会顺手改掉，**这里先记下位置**。
2. **SMTP 未配置** → 点去配置。不配，找回密码/邀请函都发不出去。
3. **注册模式**：显示 `invite_only`（邀请制，默认）或 `open`。邀请制需后台「邀请管理」发码。

> 这三项逐项说明见 [开站 checklist](launch-checklist.md)。**现在先不用逐项改完**——阶段 E 配域名时会把 announce 一并改对。

**额外建议**：后台「站点设定 → 邮件」点「测试发信」给自己发一封确认能收到；确认 `docker ps` 里 `worker` 容器是 Up（它挂了数据就不更新）。

---

# 阶段 E：域名 + HTTPS（网站真正上线）

> 这是「上线」的关键。做完这阶段，用户就能用 `https://你的域名.com` 访问，且 tracker 走加密 announce。域名部分的原理和排错详见 [域名部署](domain-deploy.md)，下面是把要做的动作全都列出来。

## E.1 买域名 + DNS 解析

在域名商（阿里云/Cloudflare/Namecheap 等）买好域名后，加一条 **A 记录**：

| 记录         | 主机名          | 指向              | 说明             |
| :----------- | :-------------- | :---------------- | :--------------- |
| A            | `@`（或 `www`） | 你的**公网 IPv4** | 主域名指向服务器 |
| AAAA（可选） | `@`             | 你的 IPv6         | 有 IPv6 才加     |

验证生效：`dig 你的域名.com +short` 能解析到你的服务器 IP（DNS 传播可能要几分钟到几小时，等）。

## E.2 开放 80 / 443 端口

**两层都要开**（云厂商「安全组」+ 服务器自身防火墙）：

- 云安全组：入站放行 **80/TCP、443/TCP**（22/TCP 保持开着以便 SSH）；
- 若开了 `ufw`：`sudo ufw allow 80,443,22/tcp`。

> tracker 走 443 反代，**7070/6969 不用对公网开放**。

## E.3 装 Nginx

```bash
sudo apt update && sudo apt install -y nginx
```

## E.4 写 Nginx 配置

新建 `/etc/nginx/conf.d/fluxtorrent.conf`，**整段复制**，把 `pt.example.com` 换成你的域名：

```nginx
limit_req_zone  $binary_remote_addr zone=ann_perip:10m rate=60r/s;
limit_conn_zone $binary_remote_addr zone=ann_conn:10m;

server {
    listen 80;
    server_name pt.example.com;
    location /.well-known/acme-challenge/ { root /var/www/certbot; }
    location / { return 301 https://$host$request_uri; }
}

server {
    listen 443 ssl http2;
    server_name pt.example.com;
    ssl_certificate     /etc/letsencrypt/live/pt.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/pt.example.com/privkey.pem;
    client_max_body_size 32m;

    # tracker announce（放最前优先匹配），转发到本地 7070
    location /announce/ {
        limit_req  zone=ann_perip burst=120 nodelay;
        limit_conn ann_conn 120;
        limit_req_status 429;
        proxy_pass http://127.0.0.1:7070;
        proxy_set_header Host $host;
    }
    location /api/    { proxy_pass http://127.0.0.1:8080; }
    location /_next/  { proxy_pass http://127.0.0.1:3000; }
    location /        { proxy_pass http://127.0.0.1:3000; }
}
```

校验并生效：

```bash
sudo nginx -t && sudo systemctl reload nginx
```

## E.5 申请 HTTPS 证书（免费 Let's Encrypt）

```bash
sudo apt install -y certbot python3-certbot-nginx
sudo certbot --nginx -d pt.example.com
```

按提示填邮箱、同意条款，证书自动签发并写入上面配置的路径，certbot 还会自动配好 80→443 跳转和续期。

## E.6 后台把 announce 改成域名

1. 浏览器开 `https://你的域名.com/admin`（用阶段 C 设的新管理员密码登录）；
2. **站点设定 → 基础设定 → Tracker 地址**，填：

   ```
   https://pt.example.com
   ```

   ⚠️ **填根地址就行，不要手填 `/announce`**——系统会自动拼 `/announce/<passkey>`，手滑带 `/announce` 也会被自动纠正。

3. （推荐）同页「HTTPS announce 地址」也填 `https://pt.example.com` → 种子走 BEP12 双 tier（https 首选 + http 回退），passkey 加密传输。
4. **已发出的旧种子要重新下载**才带新地址。

同时把 `.env` 里的跨域白名单补上：

```bash
# 编辑 docker/.env，加一行（多个域名逗号分隔）
CORS_ORIGINS=https://pt.example.com
# 改完重启
docker compose -f docker/docker-compose.yml up -d
```

## E.7 验证上线

```bash
curl https://你的域名.com/api/v1/health    # 应返回 {"status":"up"}
```

浏览器访问 `https://你的域名.com`：

- 地址栏有**小锁**（HTTPS 生效）；
- 页面正常、能登录；
- 下载一颗种子，客户端能连上 tracker（做种一小时魔力正常入账，见 [开站第一周](playbook/01-first-week.md)）。

**阶段 E 完成标志**：`https://域名` 能开、小锁亮、tracker 可连。

---

# 阶段 F：发第一颗种子 + 邀请用户

**发种子**：点阶段 C 结束页的「**发布第一颗种子**」（或后台「上传 / upload」），传一个真实资源打底。开站前建议管理组先传 20–50 个种子（见 [内容冷启动](playbook/06-coldstart.md)）。

**邀请用户**：

- 阶段 C 给的「首个邀请码」交给第一个人，注册页填入即可；
- 更多邀请码：后台「邀请管理」发放，或等级够的用户在 `/invites` 自助生成；
- 想开放注册：把注册模式改成 `open`。

---

# 阶段 G：上线前最后检查（兜底）

- **自动备份**：把 `scripts/backup.sh` 挂上 cron（pg_dump + 附件卷 + 14 天轮转）；恢复演练 `scripts/restore.sh --drill`。**别等出事才想起来没备份**。
- **监控（可选）**：`docker compose --profile monitoring up -d` 起 Prometheus + Grafana，五条告警规则已预置。

---

# 🎉 完成

到这，你的站从零到正式上线全走完了：`https://你的域名.com` 公网可访问、用户能注册能做种、有备份兜底。后面怎么运营（促销/考核/反作弊/内容）看 [运营 playbook](playbook/README.md)。

---

# 常见问题（按阶段）

| 阶段 | 现象                  | 原因                              | 怎么办                                               |
| :--- | :-------------------- | :-------------------------------- | :--------------------------------------------------- |
| A    | SSH 连不上            | 安全组没开 22 / IP 填错           | 云面板确认 22 入站放行                               |
| B    | 容器一直重启          | `.env` 三项没填 / JWT_SECRET 太短 | 看 `docker logs`，补齐合规值                         |
| C    | 向导打不开 / 404      | 访问错端口                        | 向导在 `:3000`，不是 `:8080`；`/setup/status` 看状态 |
| C    | 2.2「下一步」灰       | root 临时密码没改                 | 黄框填两遍 ≥8 位一致新密码                           |
| C    | 2.4「完成安装」灰     | 合规框没勾                        | 勾上必勾框                                           |
| D/E  | announce 仍 127.0.0.1 | 后台值没改 / 旧种子               | 后台改域名；旧种子重下                               |
| E    | 443 打不开            | 证书路径错 / 防火墙没开 443       | `nginx -t` 查错；安全组+ufw 开 443                   |
| E    | 种子连不上 tracker    | Nginx 没代理 `/announce/`→7070    | 检查 E.4 的 `location /announce/` 并 reload          |
| E    | certbot 报无法验证    | 80 没开 / DNS 未生效              | 开 80、确认域名解析到本机，等 DNS 传播               |

---

词看不懂？→ [术语小词典](glossary.md)
