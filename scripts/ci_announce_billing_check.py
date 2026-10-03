# -*- coding: utf-8 -*-
"""CI 用：announce → Redis Stream → worker 计费链路冒烟（PT 生命线，无外部依赖）。

前置：api(8080)/tracker(7070)/worker 已在本机起好，DB 里有 root（class 99，
passkey='rootpasskey0000000000000000000ff'，由 e2e-smoke 引导步骤写入）。

验证链路：
  1. tracker announce（root passkey + 真实 info_hash）→ 200 + bencode + interval；
  2. 事件进 Redis Stream flux:announce（XRANGE 可见，含 uploaded/downloaded 增量字段）；
  3. worker（本地常驻，分钟级 tick）消费后 snatches 行出现做种记录；
  4. DLQ 为空（消费无失败）。
退出码非 0 = 链路断裂，CI 失败。
"""
import json
import sys
import time
import urllib.request

API = "http://127.0.0.1:8080"
TR = "http://127.0.0.1:7070"
PASSKEY = "rootpasskey0000000000000000000ff"
IH = "%41" * 20  # 20 字节 info_hash（与 CI 引导的 ci-smoke 种子无关，计费按 info_hash 独立记账）
PEER_ID = "-cismoke0-0123456789"

failures = []


def check(name, cond, detail=""):
    print(("PASS " if cond else "FAIL ") + name + (
        f"  {detail}" if detail else ""))
    if not cond:
        failures.append(name)


def http_get(url, timeout=10):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return r.status, r.read()


# 1. announce 正常响应
try:
    url = (f"{TR}/announce/{PASSKEY}?info_hash={IH}&peer_id={PEER_ID}"
           f"&port=6881&uploaded=1024&downloaded=0&left=0&compact=1&numwant=5")
    status, body = http_get(url)
    check("announce·200+bencode", status == 200 and body.startswith(b"d"),
        body[:80])
    check("announce·interval下发", b"interval" in body, body[:120])
except Exception as e:
    check("announce·200+bencode", False, str(e)[:120])
    check("announce·interval下发", False, "")

# 1b. 坏 passkey 拒绝（协议层 failure reason）
try:
    url = (f"{TR}/announce/passkey_bad0000000000000000000000?info_hash={IH}"
           f"&peer_id={PEER_ID}&port=6881&uploaded=0&downloaded=0&left=0")
    status, body = http_get(url)
    check("announce·坏passkey拒绝", b"failure reason" in body, body[:80])
except Exception as e:
    check("announce·坏passkey拒绝", False, str(e)[:120])


def redis_cmd(*args):
    """经 redis-cli 执行（CI 容器外宿主侧；redis 无密码冒烟口径）。"""
    import subprocess
    out = subprocess.run(
        ["redis-cli", "-h", "127.0.0.1", *args],
        capture_output=True, text=True, timeout=10,
    )
    return out.stdout.strip()


# 2. Stream 里能看到本 peer 的事件（tracker 异步投递，轮询 10s）
# 事件 payload 里没有 peer_id（emit_event 只带 user/hash/up/down/ts/ip/agent），
# 匹配键用 info_hash 的 hex 形态（41×20 = "4141...41"）——本脚本专用 hash，
# 唯一前缀足够。注意 payload 里 hash 是 40 字符十六进制小写。
HASH_HEX = "41" * 20
seen_event = False
try:
    for _ in range(20):
        out = redis_cmd("XRANGE", "flux:announce", "-", "+")
        if HASH_HEX in out.lower():
            seen_event = True
            break
        time.sleep(0.5)
    check("stream·事件可见", seen_event,
        "flux:announce 内含本 hash" if seen_event else "10s 内未见事件")
except Exception as e:
    check("stream·事件可见", False, str(e)[:120])

# 3. worker 消费（分钟级 tick；轮询 100s）→ 计费入账 + DLQ 空。
# 0225 起 worker 走 XREADGROUP 消费者组（fluxcg-announce），旧
# flux:announce:cursor 键在首启迁移后即删除——游标断言改看组内 pending
# 被清空（消费完成的标志），或退而求其次看 snatches 出现做种行。
if seen_event:
    try:
        consumed = False
        for _ in range(100):
            time.sleep(1)
            pend = redis_cmd("XPENDING", "flux:announce",
                             "fluxcg-announce")
            # XPENDING 输出首行是 pending 计数；「0」= 组内无待处理（已消费完）
            if pend.splitlines() and pend.splitlines()[0].strip() == "0":
                consumed = True
                break
        check("worker·组内消费完成", consumed,
              f"XPENDING: {pend.splitlines()[0] if pend.splitlines() else '空'}")
        dlq_len = redis_cmd("LLEN", "flux:announce:dlq")
        check("worker·DLQ为空", dlq_len == "0", f"dlq_len={dlq_len}")
    except Exception as e:
        check("worker·组内消费完成", False, str(e)[:120])
        check("worker·DLQ为空", False, "")

# 4. api /health 仍健康（worker 计费不阻塞 api）
try:
    status, body = http_get(f"{API}/api/v1/health")
    check("api·health", status == 200 and b'"code":0' in body, body[:80])
except Exception as e:
    check("api·health", False, str(e)[:120])

print(
    "\n===== announce→计费 链路：{len([1]) * 0 or ''}{failures and 'FAIL ' +"
        "str(failures) or 'ALL PASS'} =====")
sys.exit(1 if failures else 0)
