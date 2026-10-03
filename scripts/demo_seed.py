#!/usr/bin/env python3
"""对接自测实例的种子数据脚本（0268）。

给第三方工具维护者用：三行命令起一套本地实例后，跑本脚本就能拿到
「一枚过审的演示种子 + 一枚 read+upload Token + 全部对接 URL」，
不用自己造数据。纯标准库，无第三方依赖。

前置：已完成网页安装向导（建好管理员）。

用法：
  python scripts/demo_seed.py --base http://127.0.0.1:8080 \\
      --user admin --pass '你的管理员密码'
"""

import argparse
import hashlib
import json
import urllib.error
import urllib.parse
import urllib.request

API = "/api/v1"


# ---------- 极简 bencode 编码（只够生成一枚合法 .torrent） ----------
def benc(obj) -> bytes:
    if isinstance(obj, int):
        return b"i%de" % obj
    if isinstance(obj, str):
        raw = obj.encode()
        return b"%d:%s" % (len(raw), raw)
    if isinstance(obj, bytes):
        return b"%d:%s" % (len(obj), obj)
    if isinstance(obj, list):
        return b"l" + b"".join(benc(x) for x in obj) + b"e"
    if isinstance(obj, dict):
        # BEP3：字典键必须按字节序（键可能混用 str/bytes，统一按编码后字节排）
        body = b"".join(
            benc(k) + benc(v)
            for k, v in sorted(obj.items(), key=lambda kv: benc(kv[0]))
        )
        return b"d" + body + b"e"
    raise TypeError(type(obj))


def make_torrent(name: str, payload: bytes) -> bytes:
    """单文件 torrent：pieces = 逐块 sha1，这里只有一块"""
    piece_len = 16384
    pieces = b"".join(
        hashlib.sha1(payload[i:i + piece_len]).digest()
        for i in range(0, len(payload), piece_len)
    )
    info = {
        b"length": len(payload),
        b"name": name,
        b"piece length": piece_len,
        b"pieces": pieces,
    }
    return benc({
        b"announce": b"http://tracker.invalid/announce",
        # 注意：bytes 字面量只能是 ASCII，中文注释走 str（benc 里按 utf-8 编码）
        "comment": "FluxTorrent demo seed (对接自测)",
        b"info": info,
    })


# ---------- HTTP ----------
def req(method, url, headers=None, body=None):
    r = urllib.request.Request(url, method=method, data=body,
                               headers=headers or {})
    try:
        with urllib.request.build_opener().open(r, timeout=30) as resp:
            return resp.status, lc(resp.headers), resp.read()
    except urllib.error.HTTPError as e:
        return e.code, lc(e.headers), e.read()


def lc(h):
    """响应头 → 全小写键：actix 下发的是小写 set-cookie，
    按字面 'Set-Cookie' 去取会拿空（这就是首跑拿不到 cookie 的原因）"""
    return {k.lower(): v for k, v in h.items()}


def jget(url, tok):
    st, _, b = req("GET", url, {"Authorization": f"Token {tok}"})
    try:
        return st, json.loads(b)
    except Exception:
        return st, {}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:8080")
    ap.add_argument("--user", required=True, help="安装向导创建的管理员")
    ap.add_argument("--pass", dest="password", required=True)
    a = ap.parse_args()
    base = a.base.rstrip("/")
    ok_n, bad = 0, []

    def chk(name, cond, detail=""):
        nonlocal ok_n
        tag = "PASS  " if cond else "FAIL  "
        print(tag + name + (f"  | {detail}" if detail else ""))
        if cond:
            ok_n += 1
        else:
            bad.append(name)

    # 1) 登录（管理员），拿会话 cookie
    st, h, b = req("POST", f"{base}{API}/auth/login",
                   {"Content-Type": "application/json"},
                   json.dumps({"username": a.user,
                               "password": a.password}).encode())
    cookies = [ln.split(";")[0].strip()
               for ln in h.get("set-cookie", "").split("\n") if "=" in ln]
    chk("登录管理员", st == 200 and cookies, f"HTTP {st}")
    if st != 200:
        raise SystemExit(b.decode()[:200])
    cookie_h = {"Cookie": "; ".join(cookies)}

    # 2) 签一枚 read+upload Token
    st, _, b = req("POST", f"{base}{API}/me/tokens",
                   {**cookie_h, "Content-Type": "application/json"},
                   json.dumps({"name": "demo-seed",
                               "scopes": ["read", "upload"],
                               "rate_per_min": 600}).encode())
    tok = (json.loads(b).get("data") or {}).get("token")
    chk("签发 read+upload Token", st == 200 and tok, f"HTTP {st}")

    # 3) 生成并上传演示种子（幂等：重跑返回 duplicate=true）
    name = "FluxTorrent 对接自测种子 (demo)"
    tor = make_torrent(name, b"fluxtorrent-demo-payload" * 4)
    q = urllib.parse.urlencode({"category_id": 1, "name": name})
    st, _, b = req(
        "POST", f"{base}{API}/open/torrents?{q}",
        {"Authorization": f"Token {tok}",
         "Content-Type": "multipart/form-data; boundary=xb"},
        b"--xb\r\nContent-Disposition: form-data; name=\"file\"; "
        b"filename=\"demo.torrent\"\r\n\r\n" + tor + b"\r\n--xb--\r\n")
    d = (json.loads(b).get("data") or {})
    tid, dup = d.get("id"), d.get("duplicate")
    chk("上传演示种子（幂等）", st == 200 and tid, f"id={tid} dup={dup}")
    if st != 200:
        raise SystemExit(b.decode()[:200])

    # 4) 待审则走管理端过审（非管理员账号会 403，提示手动过审）
    st, d = jget(f"{base}{API}/open/torrents/{tid}", tok)
    if st != 200:
        st2, _, _ = req("POST", f"{base}{API}/admin/reviews/decide",
                        {**cookie_h, "Content-Type": "application/json"},
                        json.dumps({"torrent_id": tid, "approve": True,
                                    "reason": ""}).encode())
        chk("管理端过审", st2 in (200, 201), f"HTTP {st2}")
        if st2 not in (200, 201):
            print("      ↑ 请在后台「审核」面板手动通过该种子后重跑本脚本")
    else:
        chk("种子已过审可直接用", True)

    # 5) 拿 passkey（工具拼下载链要用）
    st, d = jget(f"{base}{API}/compat/nexusphp/user.json", tok)
    pk = (d.get("data") or {}).get("passkey", "<passkey>")

    # 6) 冒烟：三条通道各打一发
    st, d = jget(f"{base}{API}/compat/nexusphp/torrents.json?page=1", tok)
    n1 = len((d.get("data") or {}).get("items", []))
    chk("兼容层列表可见演示种子", n1 >= 1, f"items={n1}")
    st, _, b = req(
        "GET", f"{base}{API}/torznab/search?apikey={tok}&t=search&q=demo")
    chk("Torznab 可搜索", st == 200 and b"<item>" in b, f"HTTP {st}")
    st, _, b = req("GET", f"{base}{API}/rss/{pk}")
    chk("RSS 含 enclosure", st == 200 and b"<enclosure" in b, f"HTTP {st}")

    print(f"""
================ 对接信息（可直接粘给工具） ================
站点基址        http://localhost:3000
API 根          {base}
演示 Token      {tok}
passkey         {pk}
Torrent id      {tid}

Prowlarr/Jackett  索引器类型: Generic Torznab
                  URL:    {base}/torznab
                  APIKEY: {tok}

RSS 订阅          {base}/rss/{pk}
下载直链          {base}/api/v1/compat/nexusphp/download.php?id={tid}&passkey={pk}

机器可读契约      {base}/openapi.json
架构自描述        {base}/compat/meta
对接文档          _doc/工具维护者对接页.md
===========================================================
""")
    print(f"{ok_n} PASS / {len(bad)} FAIL")
    raise SystemExit(1 if bad else 0)


if __name__ == "__main__":
    main()
