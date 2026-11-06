"""按 tracker 活跃 peer 快照回填 snatches.seeding（2026-11-06 迁站修复）。

背景：迁站后客户端集中 announce，announce 事件洪峰把 Redis 流
（flux:announce）压到 1.7 万条积压，worker 消费速率追不上生产速率 ⇒
用户的 announce 迟迟没被消费 ⇒ `snatches.seeding` 不置真 ⇒ 用户资料页
「做种数」显示 0 / 偏小（uid 10058 显示 0、uid 13482 显示 2）。

tracker 侧是 peer 状态的权威：它每 60 秒把完整活跃 peer 快照写到
Redis `flux:tracker:peers`（TTL 1800s，peer 表自带 30 分钟 GC），
格式 `[[info_hash, [Peer, …]], …]`，Peer 里带 `user_id` 与 `left`
（left=0 即做种中）。本脚本按快照直接回填「当前在种」的 (user_id,
torrent_id) 对，让做种数立刻恢复，不等事件流消化。

只做加法（置 seeding=TRUE）；停种的清理由 worker 的 sweep.rs 按
last_seen 超时负责，本脚本不越权改 FALSE，避免把快照未覆盖的历史行误伤。

用法（服务器侧）：
    python3 scripts/resync_seeding_from_tracker.py
    python3 scripts/resync_seeding_from_tracker.py --dry-run
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from collections import defaultdict

REDIS_CONTAINER = "flux-redis"
PG_CONTAINER = "flux-postgres"
REDIS_KEY = "flux:tracker:peers"
ENV_FILE = "/opt/fluxtorrent/docker/.env"
DB_NAME = "fluxtorrent"
DB_USER = "flux"


def read_env(path: str) -> dict[str, str]:
    out: dict[str, str] = {}
    if not os.path.exists(path):
        return out
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, v = line.split("=", 1)
            out[k.strip()] = v.strip().strip('"').strip("'")
    return out


def fetch_snapshot(pw: str) -> list:
    """读 tracker peer 快照（redis-cli --no-raw 输出带引号，需两次 parse）。"""
    proc = subprocess.run(
        [
            "docker", "exec", REDIS_CONTAINER,
            "redis-cli", "-a", pw, "--no-auth-warning", "--no-raw",
            "GET", REDIS_KEY,
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "redis-cli failed")
    raw = proc.stdout.strip()
    if not raw:
        return []
    payload = json.loads(raw)
    if isinstance(payload, str):
        payload = json.loads(payload)
    return payload


def psql(sql: str, pw: str, stdin_data: str | None = None) -> str:
    """跑 SQL（走 stdin，不进命令行——12 万组 VALUES 会撑爆 argv）。"""
    proc = subprocess.run(
        [
            "docker", "exec", "-i", PG_CONTAINER,
            "psql", "-U", DB_USER, "-d", DB_NAME, "-A", "-t",
        ],
        capture_output=True,
        text=True,
        check=False,
        input=stdin_data if stdin_data is not None else sql,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "psql failed")
    return proc.stdout


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument(
        "--with-torrents",
        action="store_true",
        help="同时重算 torrents.seeders（重，默认跳过）",
    )
    args = ap.parse_args()

    env = read_env(ENV_FILE)
    redis_pw = env.get("REDIS_PASSWORD", "")
    snap = fetch_snapshot(redis_pw)
    print(f"snapshot swarms={len(snap)}")

    seeding: dict[str, set[int]] = defaultdict(set)
    total_peers = 0
    for entry in snap:
        if not isinstance(entry, list) or len(entry) < 2:
            continue
        ih, peers = entry[0], entry[1]
        if not isinstance(peers, list):
            continue
        for p in peers:
            if not isinstance(p, dict):
                continue
            total_peers += 1
            uid = p.get("user_id") or 0
            left = p.get("left")
            if uid and (left is None or left == 0):
                seeding[ih].add(int(uid))
    pairs = sum(len(v) for v in seeding.values())
    print(f"peers={total_peers} seeding_pairs={pairs} hashes={len(seeding)}")

    if args.dry_run:
        return

    # info_hash(hex) → torrent_id
    hash_rows = psql(
        "SELECT raw_info_hash || ':' || id FROM torrents "
        "WHERE raw_info_hash IS NOT NULL",
        redis_pw,
    )
    mapping = {}
    for line in hash_rows.splitlines():
        if ":" not in line:
            continue
        h, tid = line.rsplit(":", 1)
        mapping[h.strip()] = int(tid.strip())
    print(f"torrent hash map={len(mapping)}")

    values: list[str] = []
    for ih, uids in seeding.items():
        tid = mapping.get(ih)
        if not tid:
            continue
        for uid in uids:
            values.append(f"({uid},{tid})")
    if not values:
        print("no pairs to apply")
        return

    vals = ",".join(values)
    sql = (
        "UPDATE snatches s SET seeding = TRUE "
        "FROM (VALUES " + vals + ") AS v(user_id, torrent_id) "
        "WHERE s.user_id = v.user_id AND s.torrent_id = v.torrent_id "
        "AND NOT s.seeding;"
    )
    out = psql(sql, redis_pw)
    print(f"updated rows: {out.strip() or '0'}")

    # 种子级 peer 计数（torrents.seeders）重算：一次要刷两万多行，
    # 单独开关——默认跳过（周期任务跑这个太重，跟 worker 抢 DB）。
    if args.with_torrents:
        sql2 = (
            "UPDATE torrents t SET seeders = COALESCE(c.n, 0) FROM ("
            "SELECT torrent_id, count(*) AS n FROM (VALUES "
            + vals
            + ") AS v(user_id, torrent_id) GROUP BY torrent_id) c "
            "WHERE c.torrent_id = t.id AND t.seeders <> c.n;"
        )
        out2 = psql(sql2, redis_pw)
        print(f"torrents seeders resynced: {out2.strip() or '0'}")


if __name__ == "__main__":
    main()
