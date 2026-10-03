# 离线部署（拷包上传，目标机零 registry 接触）

> 适用：目标服务器**没有外网**，或 **Docker Hub / ghcr 抽风拉不动**时。本篇让你像 NP 那样「在有网的机器打好包 → 拷到服务器 → 直接起」，全程目标机不碰任何镜像仓库。不懂的词查 [术语小词典](glossary.md)。
> 配套脚本：`scripts/offline-bundle.sh`。

## 为什么需要这一篇

本站是 **Rust 编译 + Next.js 构建**，不像 NP（PHP 解释执行）那样「传源码就能跑」——源码拷过去也起不来，构建时还得联网拉基础镜像和依赖。所以**不能直接照搬 NP「上传源码」的方式**。

但你要的效果（部署时不依赖镜像仓库）能 100% 达到：把「构建」放在有网的机器上，产出的**镜像**打成包拷过去。NP 拷的是 PHP 源码，我们拷的是预构建镜像——精神一致。

---

## TL;DR

1. **打包机**（有网）：`git clone` + 填 `.env` + 跑 `scripts/offline-bundle.sh` → 得到一个 `.tar`；
2. 把 `.tar` + 仓库源码 + `docker/.env.example` 用 **SFTP / U 盘**拷到目标机；
3. **目标机**（无网）：`docker load -i xxx.tar` → `docker compose up -d`（**不加 `--build`**）。

---

## 第 1 步：在打包机上构建并导出（需要网络）

在一台能正常访问 Docker Hub / ghcr 的机器上：

```bash
git clone https://github.com/gntv456/FluxTorrent.git && cd FluxTorrent
# 或用 SSH：git clone git@github.com:gntv456/FluxTorrent.git && cd FluxTorrent
cp docker/.env.example docker/.env
# 编辑 .env，至少填 DB_PASSWORD / REDIS_PASSWORD / JWT_SECRET 三项
./scripts/offline-bundle.sh
```

脚本会依次：构建 api/worker/tracker/web 四个应用镜像 → 拉取 postgres:16-alpine、redis:7-alpine → 全部 `docker save` 成一个 tar（文件名形如 `fluxtorrent-offline-latest-20261003.tar`）。

> 监控镜像（prometheus/grafana）默认不打进包。需要的话在打包机另跑 `docker pull prom/prometheus grafana/grafana` 后手动 `docker save` 追加。

---

## 第 2 步：把包和源码传到目标机

用 SFTP / scp / U 盘，把这三样一起搬过去：

- 上一步的 `.tar` 镜像包；
- 整个仓库源码（或至少 `docker/` 目录 + `docker-compose.yml` + `scripts/`）；
- `docker/.env.example`（到目标机后复制成 `.env` 并填好三项）。

> 目标机只需装好 **Docker（含 compose 插件）**，不需要能联网。

---

## 第 3 步：目标机加载并启动（零网络）

```bash
# 1) 加载镜像（不联网）
docker load -i fluxtorrent-offline-latest-20261003.tar

# 2) 准备配置
cp docker/.env.example docker/.env
nano docker/.env      # 填 DB_PASSWORD / REDIS_PASSWORD / JWT_SECRET；也可用面板「文件」双击在线编辑

# 3) 启动（注意：不要加 --build，否则会去拉基础镜像）
docker compose -f docker/docker-compose.yml up -d
```

**怎么算成功：** `docker ps` 看到 6 个容器 `Up`；`curl http://localhost:8080/api/v1/health` 返回 `{"status":"up"}`。之后的安装向导步骤，看 [建站手把手教程](build-tutorial.md)。

---

## 三个容易踩的坑

1. **`.env` 的镜像前缀/版本要对得上**：打包机和目标机的 `docker/.env` 里 `FLUX_IMAGE_PREFIX` / `FLUX_VERSION` 必须一致，否则 compose 会因 tag 不符而去尝试拉取（目标机没网就起不来）。默认都是 `ghcr.io/gntv456/fluxtorrent` + `latest`，不动就一致。
2. **千万别加 `--build`**：离线场景用 `up -d` 即可；加 `--build` 会触发重新构建、去拉 rust/node/nginx 基础镜像，目标机没网就卡死。
3. **compose 版本别乱改**：包里含 `postgres:16-alpine` / `redis:7-alpine`，若你改了 compose 里的数据库版本，要和包里的一致，否则缺镜像。

---

## 更新 / 升级

新版本出了，回到打包机：`git pull` → 重新跑 `offline-bundle.sh` 出新版 tar → 拷到目标机 `docker load` → `docker compose up -d`。升级注意事项见 [升级与回滚](upgrade.md)（重点：升级前先备份）。

---

## 对比：三种部署方式怎么选

| 方式 | 目标机需要网络吗 | 适合 |
| :--- | :--- | :--- |
| 常规 `up -d --build`（见[快速开始](quick-start.md)） | 需要（拉基础镜像+构建） | 目标机本身有稳定外网 |
| **本篇离线镜像包** | 不需要 | 无外网 / Docker Hub 不通 / 内网机器 |
| 彻底去 Docker 原生跑（systemd） | 不需要 | 想完全不要 Docker（成本高，另见说明） |

> 「彻底去 Docker、像 NP 那样原生跑」技术上可行（直装 pg/redis/nginx + systemd 跑 Rust 二进制），但要重写部署与升级流程，收益有限，一般不推荐。

---

词看不懂？→ [术语小词典](glossary.md)
