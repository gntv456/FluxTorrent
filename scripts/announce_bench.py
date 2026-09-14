#!/usr/bin/env python3
"""Tracker announce 压测基线（_doc/反作弊与性能加固方案.md P1-5）：
对 /announce/{passkey} 做 asyncio 并发压测，输出 p50/p95/p99 延迟与吞吐。
passkey 必须真实存在于 users 表（tracker 会拒绝无效 passkey）——
从数据库导出一份仅含 passkey 的临时清单，或手动提供 --passkeys。

用法：
  python scripts/announce_bench.py --url http://127.0.0.1:7070 --concurrency 64 --duration 30
  python scripts/announce_bench.py --url http://127.0.0.1:7070 --passkeys pk1 pk2 pk3
仅标准库实现，无第三方依赖。仅用于自有环境压测。
"""
import argparse
import asyncio
import hashlib
import random
import statistics  # noqa: F401（备用口径）
import time
import urllib.parse
import urllib.request


def percentile(sorted_vals, p):
    if not sorted_vals:
        return float("nan")
    idx = min(len(sorted_vals) - 1, int(len(sorted_vals) * p / 100))
    return sorted_vals[idx]


def _sync_worker(session_args, stop_at, latencies, errors, peer_id):
    # urllib 是同步库：线程内循环直到截止时间
    url_base, passkey, info_hash = session_args
    rng = random.Random(peer_id)
    ih = bytes.fromhex(info_hash)
    pid = peer_id.encode()[:20]
    while time.monotonic() < stop_at:
        left = 0 if rng.random() < 0.7 else rng.randint(1, 1 << 24)
        qs = (
            f"info_hash={urllib.parse.quote(ih)}"
            f"&peer_id={urllib.parse.quote(pid)}"
            f"&port=51413&uploaded={rng.randint(0, 1 << 30)}&downloaded=0&left={left}&compact=1"
        )
        url = f"{url_base}/announce/{passkey}?{qs}"
        t0 = time.perf_counter()
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "FluxBench/1.0"})
            with urllib.request.urlopen(req, timeout=10) as resp:
                resp.read()
            latencies.append((time.perf_counter() - t0) * 1000)
        except Exception:
            errors.append(1)


async def main_async(args):
    passkeys = args.passkeys
    info_hash = args.info_hash or hashlib.sha1(b"flux-bench").hexdigest()
    latencies, errors = [], []
    stop_at = time.monotonic() + args.duration
    tasks = []
    loop = asyncio.get_event_loop()
    for i in range(args.concurrency):
        peer_id = f"-FB{1000 + i:04d}-{'x' * 12}"[:20]
        pk = passkeys[i % len(passkeys)]
        # asyncio.to_thread 需要 3.9+；本机 3.8 用 loop.run_in_executor 等价
        tasks.append(loop.run_in_executor(
            None, _sync_worker, (args.url, pk, info_hash), stop_at, latencies, errors, peer_id
        ))
    print(f"压测 {args.duration}s · 并发 {args.concurrency} · passkey {len(passkeys)} 个（info_hash {info_hash[:12]}…）")
    await asyncio.gather(*tasks)

    latencies.sort()
    n = len(latencies)
    print(f"\n结果：成功 {n} · 失败 {len(errors)} · 吞吐 {n / args.duration:.0f} req/s")
    if n:
        print(f"  p50 = {percentile(latencies, 50):.2f} ms")
        print(f"  p95 = {percentile(latencies, 95):.2f} ms")
        print(f"  p99 = {percentile(latencies, 99):.2f} ms")
        print(f"  max = {latencies[-1]:.2f} ms")
        print("\n请把本轮数字记录到 生产部署指南「压测基线」一节（1k/10k/50k 在线 peer 三档）。")


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description="Tracker announce 压测")
    ap.add_argument("--url", default="http://127.0.0.1:7070")
    ap.add_argument("--concurrency", type=int, default=64)
    ap.add_argument("--duration", type=int, default=30)
    ap.add_argument("--passkeys", nargs="*", default=[], help="真实 passkey（至少 1 个；tracker 校验 passkey）")
    ap.add_argument("--info-hash", default="", help="目标 info_hash hex；缺省用固定压测 hash")
    args = ap.parse_args()
    if not args.passkeys:
        ap.error("需要 --passkeys（tracker 会拒绝无效 passkey）；可从库导出：SELECT passkey FROM users LIMIT 100")
    asyncio.run(main_async(args))
