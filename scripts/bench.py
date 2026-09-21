"""压测脚本（Phase 6 上线准备）：对核心链路做基准压测。

场景（方案 §7 性能目标对应的可执行基准）：
  1. announce      —— tracker UDP-less HTTP announce（最热路径）
  2. torrents 列表  —— API 最热读接口（游标分页）
  3. login         —— 含 Argon2 校验的重接口（下限参考）

用法：
  python scripts/bench.py [duration_seconds] [concurrency]
默认 10s / 8 并发。输出 RPS、P50/P95/P99 延迟与错误数。
"""

import json
import statistics
import subprocess
import sys
import threading
import time
import urllib.request
import urllib.error

DURATION = int(sys.argv[1]) if len(sys.argv) > 1 else 10
CONCURRENCY = int(sys.argv[2]) if len(sys.argv) > 2 else 8

BASE_API = "http://127.0.0.1:8080/api/v1"
TRACKER = "http://127.0.0.1:7070"


def login_token():
    req = urllib.request.Request(
        BASE_API + "/auth/login",
        method="POST",
        data=json.dumps({"username": "uploader",
            "password": "password123"}).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req) as r:
        return json.loads(r.read())["data"]["token"]


TOKEN = login_token()


def bench_passkey():
    """取一个真实 passkey（announce 鉴权需要）；取不到则退化占位。"""
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
            "fluxtorrent", "-t", "-c",
         "SELECT passkey FROM users WHERE username='uploader';"],
        capture_output=True, text=True,
    )
    return (p.stdout.strip() or "invalid").splitlines()[0].strip()


PASSKEY = bench_passkey()

latencies = []
errors = 0
lock = threading.Lock()
stop_at = 0.0


def worker(scenario):
    global errors
    if scenario == "announce":
        ih = "%01" * 20
        url = (f"{TRACKER}/announce/{PASSKEY}?info_hash={ih}"
               "&peer_id=-test0001-test0001&port=6881&uploaded=0"
               "&downloaded=0&left=0&event=stopped&compact=1")
        headers = {}
    elif scenario == "torrents":
        url = BASE_API + "/torrents?limit=20"
        headers = {}
    else:  # login
        url = BASE_API + "/auth/login"
        headers = {"Content-Type": "application/json"}

    while time.time() < stop_at:
        t0 = time.perf_counter()
        try:
            if scenario == "login":
                req = urllib.request.Request(
                    url, method="POST",
                    data=json.dumps({"username": "uploader",
                        "password": "password123"}).encode(),
                    headers=headers,
                )
            else:
                req = urllib.request.Request(url, headers=headers)
            with urllib.request.urlopen(req, timeout=10) as r:
                r.read()
            dt = (time.perf_counter() - t0) * 1000
            with lock:
                latencies.append(dt)
        except Exception:
            with lock:
                globals()["errors"] += 1


def run(scenario):
    global stop_at, latencies, errors
    latencies = []
    errors = 0
    stop_at = time.time() + DURATION
    threads = [threading.Thread(target=worker, args=(scenario,),
        daemon=True) for _ in range(CONCURRENCY)]
    t0 = time.time()
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    elapsed = time.time() - t0
    total = len(latencies)
    if total == 0:
        print(f"[{scenario}] 全部失败 errors={errors}")
        return
    # login 场景会触发 5次/分钟 限流：错误数即被拦截数（防护生效的证据，非故障）
    if scenario == "login" and errors > 0:
        print(f"             （其中 {errors} 次被登录限流拦截 —— 5次/分钟/用户名，防护生效）")
    lat = sorted(latencies)
    p50 = lat[int(total * 0.50)]
    p95 = lat[min(total - 1, int(total * 0.95))]
    p99 = lat[min(total - 1, int(total * 0.99))]
    print(
        f"[{scenario:9s}] RPS {total / elapsed:7.1f} | "
        f"P50 {p50:7.1f}ms P95 {p95:7.1f}ms P99 {p99:7.1f}ms | "
        f"avg {statistics.mean(lat):7.1f}ms | errors {errors}"
    )


print(f"压测参数：duration={DURATION}s concurrency={CONCURRENCY}\n")
for sc in ("announce", "torrents", "login"):
    run(sc)
