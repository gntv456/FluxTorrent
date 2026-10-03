# 1Panel 部署方式（从零到上线）

> 本篇是用 **1Panel 面板**建站的完整步骤：装面板 → 起 compose 栈 → 安装向导 → 域名 HTTPS → 发种邀请。如果你是用命令行（SSH）建站，看 [SSH 部署方式](build-tutorial.md)；用宝塔看 [宝塔部署方式](deploy-baota.md)。域名部分的原理见 [域名部署](domain-deploy.md)。不懂的词查 [术语小词典](glossary.md)。

## TL;DR（赶时间看这版）

| 阶段 | 一句话做什么 |
| :--- | :--- |
| A 准备服务器 | 2 核 4G 起、Ubuntu、装 1Panel（它自带 Docker） |
| B 起 compose 栈 | 在面板里粘贴编排、导入 `.env`、启动 |
| C 安装向导 | 浏览器开 `/setup`，填四步 |
| D 域名 HTTPS | 面板里建网站 + 反代 + 证书，**手动补 tracker 那条反代** |
| E 发种邀请 | 传种子、发邀请码 |

**没耐心读全文？** 先看 [快速开始](quick-start.md)，或用命令行方式（[SSH 部署](build-tutorial.md)）。

## 这篇能带你走到哪

从「买台干净服务器」到「`https://你的域名.com` 公网可访问、用户能做种」，全程在 1Panel 里点 + 少量终端命令。预计 30–60 分钟。

---

## 阶段 A：准备服务器

1. 买一台 VPS：**2 核 4G 起步**、**有公网 IPv4**、系统选 **Ubuntu 22.04/24.04 LTS**（1Panel 对 Ubuntu/Debian 支持最好）。
2. SSH 登录 `ssh root@你的IP`。
3. **基础安全（建议）**：`adduser deploy && usermod -aG sudo deploy`；`ufw` 先别开，阶段 G 再开 80/443。
4. 记下公网 IP 和 root 密码。

> 1Panel 会自己装 Docker，所以**不用手动装 Docker**。

---

## 阶段 B：安装 1Panel

在服务器终端跑官方一键脚本（装完会打印面板地址、账号、密码，务必记下来）：

```bash
curl -sSL https://resource.fit2cloud.com/1panel/package/quick_start.sh -o quick_start.sh && sudo bash quick_start.sh
```

浏览器打开面板地址登录。首次进入会自动装好 Docker 与 Compose 编排能力。

---

## 阶段 C：上传代码 + 配环境

**方式一（推荐，终端）：**
```bash
ssh root@你的IP
git clone https://github.com/gntv456/FluxTorrent.git && cd FluxTorrent
# 或用 SSH：git clone git@github.com:gntv456/FluxTorrent.git && cd FluxTorrent
cp docker/.env.example docker/.env
```

**方式二（面板「文件」）：** 把仓库打包上传到 `/opt/fluxtorrent` 并解压，然后终端里 `cp docker/.env.example docker/.env`。

编辑 `.env`，至少填三项：

| 配置项 | 填什么 | 例子 |
| :--- | :--- | :--- |
| `DB_PASSWORD` | 强密码 | `Db#2026xk9` |
| `REDIS_PASSWORD` | 强密码 | `Rd#2026xk9` |
| `JWT_SECRET` | **≥32 位随机串**，别含 `change_me` | `openssl rand -base64 48` 取一段 |

---

## 阶段 D：用面板起 compose 栈

1. 面板左侧 **「容器 → 编排 → 创建编排」**；
2. 名称随意（如 `fluxtorrent`），**路径指向仓库根目录**（compose 文件选 `docker/docker-compose.yml`）；
3. 点「创建并启动」。起栈后应有 **5 个容器**（postgres/redis/api/worker/tracker）全部健康。

> ⚠️ **两不要**：① 应用商店里**没有** FluxTorrent 条目，也**不要**装成面板应用（应用商店应用的升级路径不受我们控制）；② **不要**让面板「自动拉取镜像更新」——升级按 `git pull && 重新 up` 节奏，先看 CHANGELOG。

**验证：** 面板「容器」里 5 个容器状态为「运行中/健康」；终端跑 `curl http://localhost:8080/api/v1/health` 返回 `{"status":"up"}`。

---

## 阶段 E：安装向导（逐屏）

浏览器开 `http://你的IP:3000/setup`，四步向导（选站型 → 站点信息 → 新手模板 → 合规确认）。**每一屏点哪、填什么、看到什么，和 SSH 方式完全一样**，直接看 [SSH 部署·阶段 C](build-tutorial.md#阶段-c安装向导逐屏走) 的逐屏说明。

⚠️ 向导里 **announce 地址先留空**（域名还没配），阶段 G 再改。第一次装会弹黄框强制改 root 临时密码（`password123` → 两遍 ≥8 位新密码），改完记住新密码。

完成后看到「✅ 安装完成」+ 首个邀请码 + 落点按钮。

---

## 阶段 F：开站前检查

向导结束页的「开站检查清单」每条可直达设置页：announce（先记下位置）、SMTP、注册模式。逐项说明见 [开站 checklist](launch-checklist.md)。**announce 留到阶段 G 一并改对。**

---

## 阶段 G：域名 + HTTPS（1Panel 里做）

### G.1 买域名 + DNS 解析
域名商后台加 **A 记录**（`@` → 你的公网 IPv4）。`dig 你的域名.com +short` 能解析到服务器 IP 才算生效（可能要等几分钟到几小时）。

### G.2 开放端口
云安全组入站放行 **80/TCP、443/TCP**（22 保持）。tracker 走 443 反代，**7070/6969 不用对公网开**。

### G.3 建网站 + 反向代理 + 证书
1. 面板 **「网站 → 创建网站」**，域名填你的域名，协议 HTTPS；
2. 进入该网站 → **「反向代理 → 创建反向代理」**，目标 URL 填 `http://127.0.0.1:3000`（网站和同源 /api 都由 web 容器处理）；
3. 网站设置里 **申请 Let's Encrypt 证书** 并开启「强制 HTTPS」。

### G.4 补 tracker 反代（重要，面板不会自动加）
默认反代只代理了网站（3000），**tracker 的 `/announce/` 还要手动补**，否则用户做不了种。在该网站的 **「配置文件」**（Nginx 配置）里，在 `server` 块、其他 `location` **之前**加：

```nginx
location /announce/ {
    proxy_pass http://127.0.0.1:7070;
    proxy_set_header Host $host;
}
```

保存后面板会自动重载 Nginx。

### G.5 后台改 announce 为域名
1. 浏览器开 `https://你的域名.com/admin`（用阶段 E 的新管理员密码）；
2. **站点设定 → 基础设定 → Tracker 地址** 填 `https://你的域名.com`（**根地址，不要带 `/announce`**，系统自动拼）；
3. （推荐）同页「HTTPS announce 地址」也填 `https://你的域名.com` → 种子走 BEP12 双 tier；
4. 编辑服务器 `docker/.env` 加 `CORS_ORIGINS=https://你的域名.com`，然后面板里重新「启动/up」栈让配置生效。

### G.6 验证上线
- `curl https://你的域名.com/api/v1/health` → `{"status":"up"}`；
- 浏览器开 `https://你的域名.com`，地址栏**小锁**亮、能登录；
- 下载一颗种子，客户端能连上 tracker（做种一小时魔力入账）。

**阶段 G 完成标志：** `https://域名` 可开、小锁亮、tracker 可连。

---

## 阶段 H：发第一颗种子 + 邀请

点阶段 E 结束页「**发布第一颗种子**」传一个真实资源打底（建议管理组先传 20–50 个，见 [内容冷启动](playbook/06-coldstart.md)）。邀请：向导给的首个邀请码交给第一人；更多在后台「邀请管理」发，或 `/invites` 自助生成；开放注册改注册模式为 `open`。

---

## 阶段 I：上线前最后检查
- **备份**：把 `scripts/backup.sh` 挂 cron（pg_dump + 附件卷 + 14 天轮转），演练 `scripts/restore.sh --drill`；
- **监控（可选）**：`docker compose --profile monitoring up -d` 起 Prometheus + Grafana。

---

# 🎉 完成

`https://你的域名.com` 公网可访问、用户能注册能做种、有备份。后续运营看 [运营 playbook](playbook/README.md)。

---

## 常见问题

| 现象 | 原因 | 怎么办 |
| :--- | :--- | :--- |
| 应用商店找不到 FluxTorrent | 本来就没有 | 走「编排」起 compose 栈，别装成面板应用 |
| 网站能开但种子连不上 tracker | `/announce/` 反代没补 | 回 G.4 在网站配置文件加那段 `location /announce/` |
| 443 打不开 | 安全组/防火墙没开 443 | 云安全组 +（若开 ufw）`ufw allow 443` |
| announce 仍 127.0.0.1 | 后台值没改 | G.5 改域名根地址，旧种子重下 |
| 升级后界面没变 | 面板自动拉镜像干扰 | 关掉自动拉取，按 `git pull && 重新 up` 升 |

---

词看不懂？→ [术语小词典](glossary.md)
