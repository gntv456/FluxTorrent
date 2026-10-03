# 宝塔面板部署方式（从零到上线）

> 本篇是用 **宝塔（BT-Panel）面板**建站的完整步骤：装宝塔 → 起 compose 栈 → 安装向导 → 域名 HTTPS → 发种邀请。用命令行（SSH）看 [SSH 部署方式](build-tutorial.md)；用 1Panel 看 [1Panel 部署方式](deploy-1panel.md)。域名原理见 [域名部署](domain-deploy.md)。不懂的词查 [术语小词典](glossary.md)。

## TL;DR（赶时间看这版）

| 阶段 | 一句话做什么 |
| :--- | :--- |
| A 准备服务器 | 2 核 4G 起、Ubuntu、装宝塔（它自带 Docker） |
| B 起 compose 栈 | 终端跑一条 `up -d`（宝塔免费版无编排 UI） |
| C 安装向导 | 浏览器开 `/setup`，填四步 |
| D 域名 HTTPS | 宝塔建网站 + 反代 + 证书，**手动补 tracker 那条反代** |
| E 发种邀请 | 传种子、发邀请码 |

**没耐心读全文？** 先看 [快速开始](quick-start.md)，或用命令行方式（[SSH 部署](build-tutorial.md)）。

## 这篇能带你走到哪

从「买台干净服务器」到「`https://你的域名.com` 公网可访问、用户能做种」，终端命令很少（宝塔免费版没有 compose 编排 UI，起栈用终端跑一条命令）。预计 30–60 分钟。

---

## 阶段 A：准备服务器

1. 买一台 VPS：**2 核 4G 起步**、**有公网 IPv4**、系统选 **Ubuntu 22.04/24.04 LTS**（宝塔对 CentOS/Ubuntu 都支持，本文以 Ubuntu 为例）。
2. SSH 登录 `ssh root@你的IP`。
3. **基础安全（建议）**：`adduser deploy && usermod -aG sudo deploy`；防火墙 `ufw` 先别开，阶段 G 再开 80/443。
4. 记下公网 IP 和 root 密码。

---

## 阶段 B：安装宝塔

在服务器终端跑官方脚本（装完会打印面板地址、账号、密码，**务必记下**）：

```bash
wget -O install.sh http://download.bt.cn/install/install-ubuntu_6.0.sh && sudo bash install.sh
```

浏览器开面板地址登录。首次进入到软件商店装 **Docker 管理器**（会带上 compose 插件）。

> 宝塔本身**不自动装 Docker**，要先装 Docker 管理器。

---

## 阶段 C：上传代码 + 配环境

1. 宝塔 **「文件」** 把仓库上传解压到 `/opt/fluxtorrent`（或终端 `git clone https://github.com/gntv456/FluxTorrent.git && cd Fluxtorrent`；SSH：`git clone git@github.com:gntv456/FluxTorrent.git && cd Fluxtorrent`）；
2. 终端里 `cp docker/.env.example docker/.env` 并编辑，至少填三项：

| 配置项 | 填什么 | 例子 |
| :--- | :--- | :--- |
| `DB_PASSWORD` | 强密码 | `Db#2026xk9` |
| `REDIS_PASSWORD` | 强密码 | `Rd#2026xk9` |
| `JWT_SECRET` | **≥32 位随机串**，别含 `change_me` | `openssl rand -base64 48` 取一段 |

---

## 阶段 D：起 compose 栈（终端）

宝塔免费版没有 compose 编排界面，起栈用终端：

```bash
cd /opt/fluxtorrent
docker compose -f docker/docker-compose.yml up -d
```

Docker 管理器里能看到 **5 个容器**即成功（postgres/redis/api/worker/tracker）。

> ⚠️ **不要在面板里逐个容器点「启动/重启」**——整个栈的生命周期交给 compose。也**别**用宝塔「Python/Node 项目」形态部署（那会把栈拆散，升级对不上）。MySQL/Redis **别**在面板另装（栈内自带，再装抢端口）。

**验证：** 终端 `curl http://localhost:8080/api/v1/health` 返回 `{"status":"up"}`。

---

## 阶段 E：安装向导（逐屏）

浏览器开 `http://你的IP:3000/setup`，四步向导（选站型 → 站点信息 → 新手模板 → 合规确认）。**逐屏点哪填什么，和 SSH 方式完全一样**，看 [SSH 部署·阶段 C](build-tutorial.md#阶段-c安装向导逐屏走)。

⚠️ 向导里 **announce 先留空**；第一次装弹黄框强制改 root 临时密码（`password123` → 两遍 ≥8 位新密码），记住新密码。完成后看「✅ 安装完成」+ 首个邀请码。

---

## 阶段 F：开站前检查

向导结束页「开站检查清单」每条可直达：announce（先记位置）、SMTP、注册模式。逐项说明见 [开站 checklist](launch-checklist.md)。announce 留到阶段 G 改。

---

## 阶段 G：域名 + HTTPS（宝塔里做）

### G.1 买域名 + DNS 解析
域名商后台加 **A 记录**（`@` → 公网 IPv4）。`dig 你的域名.com +short` 解析到服务器 IP 才生效（等几分钟到几小时）。

### G.2 开放端口
云安全组入站放行 **80/TCP、443/TCP**（22 保持）。tracker 走 443 反代，**7070/6969 不用对公网开**。

### G.3 建站点 + 反代 + 证书
1. 宝塔 **「网站 → 添加站点」**：域名填你的域名，PHP 版本选「纯静态」，**不开** FTP/数据库；
2. 域名解析到本机后，进站点 → **「反向代理 → 添加反向代理」**，目标 URL `http://127.0.0.1:3000`；
3. 站点 → **「SSL → Let's Encrypt」** 申请证书，开启「强制 HTTPS」。

### G.4 补 tracker 反代（重要）
默认反代只代理了网站（3000），**tracker 的 `/announce/` 要手动补**，否则用户做不了种。进该站点 **「配置文件」**（Nginx 配置），在 `server` 块、其他 `location` **之前**加：

```nginx
location /announce/ {
    proxy_pass http://127.0.0.1:7070;
    proxy_set_header Host $host;
}
```

保存，宝塔自动重载 Nginx。

### G.5 后台改 announce 为域名
1. 浏览器开 `https://你的域名.com/admin`（阶段 E 的新管理员密码）；
2. **站点设定 → 基础设定 → Tracker 地址** 填 `https://你的域名.com`（**根地址，不要带 `/announce`**）；
3. （推荐）同页「HTTPS announce 地址」也填 `https://你的域名.com` → BEP12 双 tier；
4. 编辑服务器 `docker/.env` 加 `CORS_ORIGINS=https://你的域名.com`，然后终端重新 `docker compose up -d` 让配置生效。

### G.6 验证上线
- `curl https://你的域名.com/api/v1/health` → `{"status":"up"}`；
- 浏览器开 `https://你的域名.com`，**小锁**亮、能登录；
- 下载一颗种子，客户端能连 tracker（做种一小时魔力入账）。

**阶段 G 完成标志：** `https://域名` 可开、小锁亮、tracker 可连。

---

## 阶段 H：发第一颗种子 + 邀请

点阶段 E 结束页「**发布第一颗种子**」传真实资源打底（建议先传 20–50 个，见 [内容冷启动](playbook/06-coldstart.md)）。邀请：首个邀请码交第一人；更多后台「邀请管理」发或 `/invites` 自助；开放注册改注册模式 `open`。

---

## 阶段 I：上线前最后检查
- **备份**：`scripts/backup.sh` 挂 cron（pg_dump + 附件卷 + 14 天轮转），演练 `scripts/restore.sh --drill`；
- **监控（可选）**：`docker compose --profile monitoring up -d` 起 Prometheus + Grafana。

---

# 🎉 完成

`https://你的域名.com` 公网可访问、用户能注册能做种、有备份。后续运营看 [运营 playbook](playbook/README.md)。

---

## 常见问题

| 现象 | 原因 | 怎么办 |
| :--- | :--- | :--- |
| 宝塔找不到 compose 入口 | 免费版本就没有 | 阶段 D 用终端 `docker compose up -d` 起栈 |
| 网站能开但种子连不上 tracker | `/announce/` 反代没补 | 回 G.4 在站点配置文件加那段 |
| 443 打不开 | 安全组/防火墙没开 443 | 云安全组 +（若开 ufw）`ufw allow 443` |
| announce 仍 127.0.0.1 | 后台值没改 | G.5 改域名根地址，旧种子重下 |
| 用「Python/Node 项目」部署后升级乱 | 栈被拆散 | 改用 compose 栈方式重来 |

---

词看不懂？→ [术语小词典](glossary.md)
