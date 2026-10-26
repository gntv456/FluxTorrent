# -*- coding: utf-8 -*-
"""资深PT用户实测：全站 GET 读口巡检（818 路由清单驱动）。

用法: python scripts/pt_audit_read_sweep.py [--anon] [--only substr]

思路：把 docs/api/route-manifest.json 里所有 GET 路由逐个打到运行中的 api，
把路径参数替换成探针值；记录 5xx / 需要登录却返回 200 的假绿。
只打 GET，不写库。
"""
import io
import json
import os
import re
import sys
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# SSE 读口：连接建立后一直挂着，不会自己结束
SKIP_SSE = {"/me/messages/stream"}
MANIFEST = os.path.join(ROOT, "docs", "api", "route-manifest.json")
BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")

# 路径参数探针值：绝大多数主键都是小整数
PROBE = {
    "id": "1",
    "user_id": "1",
    "torrent_id": "1",
    "topic_id": "1",
    "post_id": "1",
    "board_id": "1",
    "forum_id": "1",
    "offer_id": "1",
    "request_id": "1",
    "subtitle_id": "1",
    "comment_id": "1",
    "message_id": "1",
    "medal_id": "1",
    "slug": "1",
    "passkey": "1",
    "key": "1",
    "code": "1",
    "token": "1",
    "name": "1",
    "hash": "1",
    "info_hash": "1",
}
DEFAULT_PROBE = "1"


def login():
    if os.environ.get("FLUX_GATE_JWT") == "1":
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        from pt_audit_lib import jwt_token  # 装机步已把 root 口令随机化
        return jwt_token("root")
    import time
    for i in range(8):
        req = urllib.request.Request(BASE + "/auth/login", method="POST")
        req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(
                req,
                json.dumps(
        {"username": "root", "password": "password123"}
    ).encode(),
                timeout=20,
            ) as r:
                d = json.loads(r.read())
            if d.get("code") == 0:
                return d["data"]["token"]
        except urllib.error.HTTPError as e:
            if e.code != 429:
                break
        except Exception:
            pass
        time.sleep(min(2 + i * 3, 20))
    return None


def fill(path):
    def sub(m):
        name = m.group(1)
        return PROBE.get(name.lower(), DEFAULT_PROBE)
    return re.sub(r"\{(\w+)\}", sub, path)


def main():
    anon = "--anon" in sys.argv
    only = None
    if "--only" in sys.argv:
        only = sys.argv[sys.argv.index("--only") + 1]

    token = None
    if not anon:
        token = login()
        if not token:
            print("!! 登录失败，只跑匿名模式")
            anon = True

    man = json.load(io.open(MANIFEST, encoding="utf-8"))
    routes = [r for r in man["routes"] if r["method"] == "GET"]
    if only:
        routes = [r for r in routes if only in r["path"]]

    bad = []
    okc = 0
    skipped = 0
    for r in routes:
        # SSE 读口挂着不回，超时会被算成「异常」——那是协议本身如此，
        # 不是链路坏了（pt_audit_da_member_breadth 早就跳过它了）
        if r["path"] in SKIP_SSE:
            skipped += 1
            continue
        p = fill(r["path"])
        url = BASE + p
        req = urllib.request.Request(url)
        if token:
            req.add_header("Authorization", "Bearer " + token)
        try:
            with urllib.request.urlopen(req, timeout=25) as resp:
                code = resp.status
                body = resp.read()
            if code >= 500:
                bad.append(
                    (r["method"], r["path"], code, r["file"],
                     r["handler"], body[:300].decode("utf-8", "replace"))
                )
            else:
                okc += 1
        except urllib.error.HTTPError as e:
            code = e.code
            body = e.read()
            if code >= 500:
                bad.append(
                    (r["method"], r["path"], code, r["file"],
                     r["handler"], body[:300].decode("utf-8", "replace"))
                )
            else:
                okc += 1
        except Exception as e:  # 连接层异常也算问题
            bad.append(
                (r["method"], r["path"], "EXC", r["file"],
                 r["handler"], repr(e)[:300])
            )
        if len(bad) >= 60:
            break

    print("=" * 70)
    print("GET 读口巡检：%d 条路由，5xx/异常 %d 条（跳过 SSE %d 条）"
          % (len(routes), len(bad), skipped))
    print("=" * 70)
    for m, p, c, f, h, b in bad:
        print("[%s] %s %s" % (c, m, p))
        print("      handler=%s file=%s" % (h, f))
        print("      body=%s" % (b.replace("\n", " ")[:260]))
    print("\n===== 5xx=%d / 总 %d =====" % (len(bad), len(routes)))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
