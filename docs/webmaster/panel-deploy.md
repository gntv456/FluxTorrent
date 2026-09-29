# 面板部署（1Panel / 宝塔）

> 两款国内主流服务器面板的部署口径。**统一原则：面板只当「 Docker 运行时 +
> 反代 + 证书」的壳**，FluxTorrent 永远以 compose 栈运行，不拆装进面板的
> 网站目录——升级/回滚/迁移因此与裸机完全一致（见
> [升级与回滚](upgrade.md)）。
>
> 已经熟悉 Docker 的同学不需要本文：面板路径没有额外魔法。

## 1Panel

1Panel 自带 Docker 与 Compose 编排（应用商店 → 容器 → 编排），是两款面板里
更顺的路径。

### 步骤

1. **装 1Panel**（官方一键脚本），首次进入会自动装好 Docker；
2. **上传代码**：主机终端里 `git clone <本仓库> && cd FluxTorrent`（或面板
   「文件」上传解压），放在如 `/opt/fluxtorrent`；
3. **配环境**：`cp docker/.env.example docker/.env` 后编辑必填三项
   （DB_PASSWORD / REDIS_PASSWORD / JWT_SECRET，口径见
   [快速开始](quick-start.md)）；
4. **建编排**：面板「容器 → 编排 → 创建编排」，路径指向仓库根（compose 文件
   选 `docker/docker-compose.yml`）。起栈后 5 个容器（postgres/redis/api/
   worker/tracker）应全部健康；
5. **反代与证书**：「网站 → 网站 → 创建网站 → 反向代理」，域名指向
   `http://127.0.0.1:3000`（web 容器）；在网站设置里申请 Let's Encrypt
   证书并开启 HTTPS；
6. **放行端口**：面板「主机 → 防火墙」放行 80/443（网站）与 **6969/UDP、
   7070/TCP**（tracker，announce 直连走它，见 launch-checklist）；
7. **收尾**：浏览器 `https://<域名>/setup` 完成安装向导。

### 注意

- 1Panel 的「应用商店」没有 FluxTorrent 条目（也不建议装成面板应用——
  应用商店的应用升级路径不受我们控制）；
- 编排名随意，但**不要**让面板「自动拉取镜像更新」——升级按
  `git pull && 重新 up` 的节奏来，先看 CHANGELOG 再动。

## 宝塔（BT-Panel）

宝塔免费版没有 compose 编排 UI，路径是「终端装 Docker + 网站反代」。

### 步骤

1. **装宝塔**（官方脚本），软件商店里安装 **Docker 管理器**（会带上
   docker compose 插件）；
2. **上传代码**：宝塔「文件」上传仓库到 `/opt/fluxtorrent`（或终端 git
   clone）；
3. **配环境**：同上，`docker/.env` 必填三项；
4. **起栈**：宝塔终端 `docker compose -f docker/docker-compose.yml up -d`。
   Docker 管理器里能看到 5 个容器即成功（不要在面板里逐个容器点「启动/
   重启」——compose 栈整体生命周期交给 compose）；
5. **反代**：宝塔「网站 → 添加站点」（纯静态、不开 FTP/数据库），域名解析
   到本机后进站点设置 → 「反向代理」目标 `http://127.0.0.1:3000`；
6. **证书**：站点设置 → SSL → Let's Encrypt 申请，开启「强制 HTTPS」；
7. **tracker 端口**：宝塔「安全」放行 6969/UDP 与 7070/TCP；
8. **收尾**：`https://<域名>/setup` 完成向导。

### 注意

- 宝塔的 Nginx 反代默认 `proxy_buffering on`，对 web（Next.js）无碍；
  若后续挂大文件下载代理，参照 `_doc/生产部署指南.md` 调 buffer；
- **别用宝塔的「Python/Node 项目」形态部署**——那会把栈拆散成面板托管，
  升级/多机扩容（G30 配方）都对不上；
- MySQL/Redis 别在面板里另装一份——栈内自带，再装只会抢端口。

## 常见问题

| 症状 | 原因 | 处置 |
|---|---|---|
| 容器起一半退出 | `.env` 必填三项没填全 | 看容器日志（面板可看），补齐后 `up -d` |
| 网站打开了但 API 全 404 | 反代只代理了页面没代理 `/api` | web 容器自带 `/api` 转发（同源），确认反代目标是 3000 整站 |
| announce 不通 | 6969/UDP 未放行或被面板防火墙拦 | 面板+云厂商安全组两层都要放 |
| 面板重装后容器消失 | Docker 数据目录被清 | 代码与 `docker/.env` 在仓库目录，`up -d` 即恢复（数据在卷里，先确认卷未被删） |
