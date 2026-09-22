"""性能基线采集（六维强化方案 P0-7）：端点延迟 + PG 语句级热点。

背景：仓库此前没有慢查询日志、没有语句统计、没有压测脚本，
所有性能目标（列表 P95 < 120ms、详情页 SQL ≤ 4 条…）都只能靠体感。
本脚本建立可复跑的基线。

用法：python scripts/perf_baseline.py [--rounds 5] [--torrent-id 2]
产物：_doc/性能基线-YYYY-MM-DD.md

注意：dev 库数据量很小（数十行），绝对值不代表生产；它的价值是
    ① 把「怎么测」固化下来 ② 提供同环境前后对比的锚点。
"""
import argparse
import base64
import datetime as dt
import hashlib
import hmac
import json
import re
import statistics
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

API = "http://127.0.0.1:8080"
EVAL_KEYS = (
    "for _,k in ipairs(redis.call('keys','cache:tlist*')) "
    "do redis.call('del',k) end return 1"
)


def sh(*args):
    p = subprocess.run(args, capture_output=True)
    return p.stdout.decode("utf-8", "replace").strip()


def psql(sql):
    return sh("docker", "exec", "flux-postgres", "psql", "-U", "flux",
              "-d", "fluxtorrent", "-t", "-A", "-c", sql)


def env(key):
    txt = open("docker/.env", encoding="utf-8").read()
    m = re.search(rf"^{key}=(.+)$", txt, re.M)
    if not m:
        raise SystemExit(f"docker/.env 缺 {key}")
    return m.group(1).strip().strip('"').strip("'")


def jwt():
    secret = env("JWT_SECRET")
    now = int(time.time())
    b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=")
    hdr = b64(json.dumps({"alg": "HS256", "typ": "JWT"},
                         separators=(",", ":")).encode())
    claims = {"sub": 2, "class_id": 1, "iat": now, "exp": now + 3600}
    pay = b64(json.dumps(claims, separators=(",", ":")).encode())
    sig = hmac.new(secret.encode(), hdr + b"." + pay, hashlib.sha256).digest()
    return (hdr + b"." + pay + b"." + b64(sig)).decode()


def clear_list_cache():
    """清列表缓存：保证测的是冷查询（否则第二次起全是 Redis 命中）"""
    pw = env("REDIS_PASSWORD")
    sh("docker", "exec", "flux-redis", "redis-cli", "-a", pw, "eval",
       EVAL_KEYS, "0")


def timed(path, tok, rounds):
    url = API + urllib.parse.quote(path, safe="/?&=,~")
    req = urllib.request.Request(
        url, headers={"Authorization": "Bearer " + tok})
    samples, status, size = [], None, 0
    for _ in range(rounds):
        clear_list_cache()
        t0 = time.perf_counter()
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                body = r.read()
                status, size = r.status, len(body)
        except urllib.error.HTTPError as e:
            status, size = e.code, 0
        samples.append((time.perf_counter() - t0) * 1000)
    samples.sort()
    p50 = statistics.median(samples)
    p95 = samples[min(len(samples) - 1, int(len(samples) * 0.95))]
    return p50, p95, status, size


CASES = [
    ("列表·默认首屏", "/api/v1/torrents?limit=20"),
    ("列表·按做种数", "/api/v1/torrents?limit=20&sort=seeders"),
    ("列表·按大小", "/api/v1/torrents?limit=20&sort=size"),
    ("列表·按发布时间", "/api/v1/torrents?limit=20&sort=created"),
    ("列表·搜索", "/api/v1/torrents?limit=20&search=物理"),
    ("列表·多选分类", "/api/v1/torrents?limit=20&category_id=1,2"),
    ("列表·促销筛选", "/api/v1/torrents?limit=20&promo=free"),
    ("列表·标签筛选", "/api/v1/torrents?limit=20&tag_id=3"),
]


def build_cases(tid, passkey):
    return CASES + [
        ("详情·主行", f"/api/v1/torrents/{tid}"),
        ("详情·聚合", f"/api/v1/torrents/{tid}/aggregate"),
        ("详情·文件", f"/api/v1/torrents/{tid}/files"),
        ("详情·感谢", f"/api/v1/torrents/{tid}/thanks"),
        ("详情·评论", f"/api/v1/torrents/{tid}/comments"),
        ("详情·NFO", f"/api/v1/torrents/{tid}/nfo"),
        ("RSS·50 条", f"/api/v1/rss/{passkey}?showrows=50"),
    ]


TOP_SQL = (
    "SELECT round(total_exec_time::numeric,2)||'ms | calls='||calls"
    "||' | mean='||round(mean_exec_time::numeric,2)||'ms | '"
    "||left(regexp_replace(query,'\\s+',' ','g'),110) "
    "FROM pg_stat_statements WHERE query NOT ILIKE '%pg_stat_statements%' "
    "ORDER BY total_exec_time DESC LIMIT 12;"
)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--torrent-id", type=int, default=2)
    a = ap.parse_args()
    tok = jwt()
    tid = a.torrent_id
    passkey = psql("SELECT passkey FROM users WHERE id=2;")

    rows = []
    for name, path in build_cases(tid, passkey):
        p50, p95, status, size = timed(path, tok, a.rounds)
        rows.append((name, path, p50, p95, status, size))
        print(f"{name:<14} p50={p50:7.1f}ms p95={p95:7.1f}ms "
              f"status={status} {size}B")

    counts = {t: psql(f"SELECT count(*) FROM {t};")
              for t in ("torrents", "files", "users", "comments")}
    stats = psql(TOP_SQL)
    slow = sh("docker", "logs", "flux-postgres",
              "--tail", "200").count("duration:")

    today = dt.date.today().isoformat()
    stamp = dt.datetime.now().isoformat(timespec="seconds")
    out = [
        f"# 性能基线 · {today}", "",
        "> 采集：`python scripts/perf_baseline.py --rounds 5`（可复跑）。",
        "> 每次采样前清 `cache:tlist*`，测冷查询；",
        "> `pg_stat_statements` 与 `log_min_duration_statement=300ms` "
        "由 compose 启用。",
        "",
        "## 0. 环境与数据量", "",
        f"- 采集时间：{stamp}",
        f"- 数据量：torrents={counts['torrents']}"
        f" / files={counts['files']}"
        f" / users={counts['users']}"
        f" / comments={counts['comments']}",
        "- ⚠️ dev 库仅数十行，绝对值不代表生产；本表用于**同环境前后对比**"
        "与固化测量方法。",
        "",
        "## 1. 端点延迟（每项 N 次，含建连）", "",
        "| 用例 | 路径 | p50 | p95 | HTTP | 字节 |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for name, path, p50, p95, status, size in rows:
        out.append(
            f"| {name} | `{path}` | {p50:.1f}ms | {p95:.1f}ms "
            f"| {status} | {size} |"
        )
    out += [
        "", "## 2. PG 语句级热点（pg_stat_statements，按总耗时）", "",
        "```", stats or "(无)", "```", "",
        f"- 容器日志中 `duration:` 出现次数（≥300ms 慢查询）：{slow}", "",
        "## 3. 怎么用这张表", "",
        "- 改种子页后端后**重跑本脚本**，对比 p50/p95 与热点语句，作为回归证据；",
        "- 判断「排序是否走索引」看 EXPLAIN，不看本表（小表永远快）；",
        "- 生产部署后用真实数据量重跑，另存为 `性能基线-<日期>-prod.md`。",
    ]

    path = f"_doc/性能基线-{today}.md"
    open(path, "w", encoding="utf-8").write("\n".join(out) + "\n")
    print(f"\nwritten {path}")


if __name__ == "__main__":
    main()
