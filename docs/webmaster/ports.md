# 端口一览与改端口指南

6 个容器的端口全部集中在 `docker/.env` 一个文件里改。
绝大多数「端口被占」只需要改**宿主机侧映射**，容器内部监听端口是固定的、不用动。

> 本文只讲 docker compose 栈（`docker/docker-compose.yml`，多机的 `compose.multi-node.yml` 用的是同一套变量，改法相同）。云安全组/防火墙要开哪些端口，见[域名部署第 2 步](domain-deploy.md#第-2-步开放端口防火墙--安全组)。

## 先确认被占的是哪个端口

服务器上（Linux）：

```bash
ss -lntp | grep 6379        # 换成你冲突的端口号
# 看 users: 里是不是 docker-proxy——是 docker 说明是别的容器在抢，
# 是普通进程名说明是宿主上的服务（面板装的 Redis/MySQL 最常见）
```

本机电脑上（Windows PowerShell）：

```powershell
netstat -ano | findstr :8080
Get-NetTCPConnection -LocalPort 8080 | Select-Object LocalPort,State,OwningProcess
```

## 端口总表

| 服务 | 宿主端口变量 | 默认 | 容器内部端口（固定，别动） | 默认只绑本机? |
|---|---|---|---|---|
| postgres | `FLUX_DB_PORT` | 5432 | 5432 | 是（127.0.0.1） |
| redis | `FLUX_REDIS_PORT` | 6379 | 6379 | 是（127.0.0.1） |
| api | `FLUX_API_PORT` | 8080 | 8080 | 是（`API_BIND`） |
| web | `FLUX_WEB_PORT` | 3000 | 3000 | 是（`WEB_BIND`） |
| tracker（TCP） | `FLUX_TRACKER_PORT` | 7070 | 7070 | 否（`FLUX_TRACKER_BIND`，默认 0.0.0.0） |
| tracker（UDP） | `FLUX_TRACKER_UDP_PORT` | 6969 | 6969 | 否（同上） |
| prometheus | `FLUX_PROM_PORT` | 9090 | 9090 | 是（127.0.0.1） |
| grafana | `FLUX_GRAFANA_PORT` | 3001 | 3000 | 是（127.0.0.1） |

两种改法（效果一样，选顺手的）：

1. `docker/.env.example` 第 68-75 行就有这组注释模板，复制取消注释改值；
2. 或直接在 `docker/.env` 里加一行，例如 8080 被占：

   ```ini
   FLUX_API_PORT=8088
   ```

   容器里 api 还是听 8080，只是宿主侧映射到 8088。

> 🔒 **`API_BIND` / `WEB_BIND` / `FLUX_TRACKER_BIND` 不是端口**——它们控制绑哪个网卡地址（`127.0.0.1` 只本机可达、`0.0.0.0` 对外），和端口变量是两回事，别混。

## 改哪些端口需要联动（最容易踩的坑）

**改 tracker 端口 → 必须同步 `PUBLIC_TRACKER_URL`**

`.torrent` 文件里烤死的 announce 地址来自 `PUBLIC_TRACKER_URL`（默认 `http://localhost:7070`）。
把 `FLUX_TRACKER_PORT` 改成 7071 后，必须在 `.env` 里**新增**一行：

```ini
PUBLIC_TRACKER_URL=http://你的域名或IP:7071
```

否则已发出去的老种子会停种（新种子按新地址烤，老文件里写死的是旧地址）。
走了 Nginx 反代 announce 的站（见[域名部署](domain-deploy.md)）客户端走 443，一般不用动这行。

**改 web 端口 → 检查指向 3000 的变量**

- `CORS_ORIGINS=http://localhost:3000`：浏览器基址和 CORS 要一起改成新端口；
- `PUBLIC_SITE_URL`：同理（影响对外链接、种子页地址等）。

**改 api 端口 → 浏览器侧访问基址**

- `NEXT_PUBLIC_API_URL` 默认留空即可（容器网络内自动寻址，不用管宿主端口）；
- 只有「浏览器直连 api」（不走 Nginx `/api/` 反代）的本机调试场景才需要显式设
  `NEXT_PUBLIC_API_URL=http://localhost:8088`。注意它在 Next 构建期固化，
  设了要 `docker compose up -d --build web` 重建 web 才可靠；
- 走了 Nginx 反代 `/api/` 的站，浏览器只认 443/80，改 api 宿主端口无感。

## 改完怎么生效

```bash
cd docker
docker compose up -d        # 只改了端口映射 → 这样就够
docker compose ps           # 看六个容器是否 healthy
```

只有同时改了镜像源码/版本才需要 `--build`。

> ⚠️ 如果你想改的是**容器内部监听端口**（compose 文件 `environment` 里那些
> `*_BIND` / 端口写死的值）：那要改 `docker-compose.yml` 本身并重建镜像，
> 但容器内端口没有任何理由要改——宿主端口冲突一律用上面的 `FLUX_*_PORT` 解决。
