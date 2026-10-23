# -*- coding: utf-8 -*-
"""VIP 深测探针的共用底座：HTTP/PSQL 工具 + 探针种自愈件。

从 pt_audit_vip_walk.py 拆出（500 行门禁：原文件 803 行）。这里只有
「怎么发请求、怎么记一条结果、怎么把探针种喂活」，没有任何判据：

  - call/expect/ok_envelope/rec：统一打接口并把 PASS/FAIL 记进 RESULTS；
  - mint/fxo_token/psql：铸两枚 JWT（audit_vip class 9 / root class 99）、
    开 API token、直连库读数；
  - _benc/_probe_torrent/upload_probe_torrent/tracker_announce/
    ensure_probe_torrent：造一颗带存档的探针种并 announce 一次，
    下载/做种两条链没有这个前提根本走不动。

RESULTS 与 CSRF 是模块级可变状态，读侧与写侧两支都往同一个列表里记，
汇总只在入口文件做。
"""
from __future__ import annotations


import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

API = os.environ.get("FLUX_API", "http://127.0.0.1:8080")
TRACKER = os.environ.get("FLUX_TRACKER", "http://127.0.0.1:7070")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MINT = os.path.join(ROOT, "_mint_jwt.py")

RESULTS: list[tuple[str, str, str]] = []  # (域名, 级别, 详情)
# CSRF 双提交值（GET /api/v1/me/csrf 下落 cookie，写方法回填头）
CSRF: list[str] = [""]


def csrf_handshake(token: str) -> None:
    """取 flux_csrf cookie 值（浏览器同口径；缺它全站 cookie 写口全 403）。"""
    req = urllib.request.Request(
        API + "/api/v1/me/csrf",
        headers={"Cookie": f"flux_token={token}; flux.session=1"},
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        for line in r.headers.get_all("Set-Cookie") or []:
            if line.startswith("flux_csrf="):
                CSRF[0] = line.split(";")[0][len("flux_csrf="):]


def rec(domain: str, level: str, msg: str) -> None:
    RESULTS.append((domain, level, msg))


def mint(uid: int, cls: int) -> str:
    out = subprocess.run(
        [sys.executable, MINT, "--uid", str(uid), "--class", str(cls)],
        capture_output=True, text=True, check=True,
    )
    return out.stdout.strip()


def psql(sql: str) -> str:
    out = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True,
    )
    return out.stdout.strip()


FXO: list[str] = [""]


def fxo_token(uid: int = 7346) -> str:
    """本轮唯一的一枚 read+upload 探针 Token（无钥 SHA3-256 直接落库）。

    原来写死 `fxo_ptaudit0001`：`ON CONFLICT (token_hash) DO NOTHING` 让
    那一行的 `created_at` 永远停在第一次插入那天，而开放口的判据是
    `created_at > to_timestamp(token_revocations.nbf)`（全站撤销线）——
    任何人重置过「撤销此前所有 Token」，这枚常量 Token 就永久 401
    （2026-10-23 实测：拆前拆后两次全量都红在这两条上，站方行为是对的）。
    """
    import hashlib
    if FXO[0]:
        return FXO[0]
    plain = "fxo_ptaudit%d" % int(time.time())
    psql(
        "INSERT INTO api_tokens (user_id, name, token_hash, scopes, "
            "rate_per_min, expires_at) "
        f"VALUES ({uid}, 'pt_audit', "
            f"'{hashlib.sha3_256(plain.encode()).hexdigest()}', "
        "ARRAY['read','upload'], 600, now() + interval '1 day') "
        "ON CONFLICT (token_hash) DO NOTHING"
    )
    FXO[0] = plain
    return plain


def call(
    method: str,
    path: str,
    token: str,
    body: dict | None = None,
    base: str = API,
    raw: bool = False,
    bearer: bool = False,
) -> tuple[int, object]:
    url = base + path
    data = None
    if bearer:
        headers = {"Authorization": f"Token {token}"}
    else:
        # cookie 鉴权的写方法必须过 CSRF 双提交闸（flux_csrf cookie +
        # x-csrf-token 头逐字相等）——与浏览器同口径
        cookie = f"flux_token={token}; flux.session=1"
        if CSRF[0]:
            cookie += f"; flux_csrf={CSRF[0]}"
        headers = {"Cookie": cookie}
        if method not in ("GET", "HEAD") and CSRF[0]:
            headers["x-csrf-token"] = CSRF[0]
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            payload = r.read()
            if raw:
                return r.status, payload
            try:
                return r.status, json.loads(payload)
            except Exception:
                # 非 JSON 成功体（如 RSS 的 XML）——不当失败，原样带回
                return r.status, payload[:200].decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        payload = e.read()
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, payload[:200].decode("utf-8", "replace")
    except Exception as e:  # 连接失败等
        return 0, str(e)


def expect(domain: str, name: str, cond: bool, detail: str = "") -> bool:
    rec(domain, "PASS" if cond else "FAIL",
        f"{name}{('｜' + detail) if detail else ''}")
    return cond


def ok_envelope(domain: str, name: str, st: int, js: object) -> bool:
    if st != 200 or not isinstance(js, dict) or js.get("code") != 0:
        return expect(domain, name, False, f"st={st} body={str(js)[:160]}")
    return expect(domain, name, True)


def _benc(obj, out=None):
    """最小 bencode 编码（造 .torrent 用）。"""
    out = bytearray() if out is None else out
    if isinstance(obj, int):
        out += b"i" + str(obj).encode() + b"e"
    elif isinstance(obj, bytes):
        out += b"%d:" % len(obj) + obj
    elif isinstance(obj, list):
        out += b"l"
        for v in obj:
            _benc(v, out)
        out += b"e"
    elif isinstance(obj, dict):
        out += b"d"
        for k in sorted(obj.keys()):
            _benc(k, out)
            _benc(obj[k], out)
        out += b"e"
    return bytes(out)


def _probe_torrent(name: str, length: int = 1_048_576) -> bytes:
    import hashlib
    piece_len = 16384
    info = {
        b"piece length": piece_len,
        b"name": name.encode(),
        b"private": 1,
        b"length": length,
        b"pieces": hashlib.sha1(name.encode()).digest()
        * max(1, (length + piece_len - 1) // piece_len),
    }
    return _benc({b"info": info, b"creation date": int(time.time())})


def upload_probe_torrent(tok: str, name: str) -> tuple[int, str] | None:
    """multipart 上传一枚探针种（免审直过），返回 (torrent id, info_hash)。"""
    crlf = chr(13) + chr(10)
    dq = chr(34)
    b = "----auditvipup"
    parts = b""
    for k, v in {
        "category_id": 1,
        "name": name,
        "small_descr": "全站深测探针种",
        "descr": "全站深测探针种：正常长度描述。本资源为测试用途，包含充分的说明"
                 "文字以通过最低内容标准检查。补充来源与出处说明，长度足够通过校验。"
                 "另附媒体参数与转码来源备注，确保描述长度达到站点自动审核的"
                 "最低字数要求，避免探针种在测试周期内被自动审种流程拒绝。",
    }.items():
        parts += (
            f"--{b}{crlf}Content-Disposition: form-data; name={dq}{k}{dq}"
            f"{crlf}{crlf}{v}{crlf}"
        ).encode()
    parts += (
        f"--{b}{crlf}Content-Disposition: form-data; name={dq}file{dq}; "
        f"filename={dq}probe.torrent{dq}{crlf}"
        f"Content-Type: application/x-bittorrent{crlf}{crlf}"
    ).encode() + _probe_torrent(name)
    parts += (crlf + f"--{b}--" + crlf).encode()
    req = urllib.request.Request(
        API + "/api/v1/torrents", method="POST", data=parts)
    req.add_header("Content-Type", f"multipart/form-data; boundary={b}")
    req.add_header(
        "Cookie",
        f"flux_token={tok}; flux.session=1; flux_csrf={CSRF[0]}")
    req.add_header("x-csrf-token", CSRF[0])
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            d = json.loads(r.read()).get("data") or {}
            return d.get("id"), d.get("info_hash")
    except Exception as e:
        rec("自愈", "WARN", f"上传探针种失败：{str(e)[:80]}")
        return None


def tracker_announce(passkey: str, info_hash_hex: str, size: int = 0,
                     downloaded: int = 0, event: str = "started") -> bool:
    """HTTP tracker announce（路径 /announce/{passkey}，info_hash 须原始 20 字节）。
    downloaded>0 时模拟「下载过再挂种」——反幽灵闸要求物理下载量达
    size×10.4%（或可达回连），纯 left=0 的空壳 announce 会被判幽灵
    （snatches.seeding=false），种子不计入活种。"""
    q = urllib.parse.urlencode({
        "info_hash": bytes.fromhex(info_hash_hex),
        "peer_id": b"-pt0001-aud1tv1p0001",
        "port": 6881,
        "uploaded": 0,
        "downloaded": downloaded,
        "left": 0 if downloaded >= size and size > 0 else max(size - downloaded,
            0),
        "compact": 1,
        "event": event,
        "numwant": 10,
    })
    try:
        with urllib.request.urlopen(
            f"{TRACKER}/announce/{passkey}?{q}", timeout=15
        ) as r:
            body = r.read()
            if b"failure reason" in body:
                return False
            return b"peers" in body or b"interval" in body
    except Exception:
        return False


def ensure_probe_torrent(root_tok: str, vip_tok: str) -> None:
    """每轮都自建一枚探针种并挂做种 peer——不能只在「无活种」时补：
    浏览/做种域的断言按 pt_audit 前缀找自家探针（并行会话的 deep_ 种
    不算数），列表非空就跳过会让这些断言必挂（2026-10-22 实测）。"""
    name = f"pt_audit_alive.{int(time.time())}"
    out = upload_probe_torrent(root_tok, name)
    if not out:
        return
    tid, ih = out
    # 反幽灵闸的合法路径：首报只立基线不计增量（防一次 announce 编造
    # 全量下载），所以分两拍——①以下载态立基线 ②以「已下完」态产生
    # size 增量 ⇒ phys_down=size ≥ 10.4% ⇒ 判真做种，种子计入活种。
    ok1 = tracker_announce("a17b2c3d4e5f60718293a4b5c6d7e8f9", ih,
                           size=1_048_576, downloaded=0)
    time.sleep(2)
    ok2 = tracker_announce("a17b2c3d4e5f60718293a4b5c6d7e8f9", ih,
                           size=1_048_576, downloaded=1_048_576)
    if ok1 and ok2:
        # worker 经 Redis 流异步落账：consume_announce 是 60s tick，
        # 轮询必须覆盖一个完整周期（旧 30s 上限在 tick 后半窗写入时必挂）。
        # 且必须等**自家探针**（tid 的 seeders>0）——等「任意活种」会在
        # 站上另有活种时秒过，探针自己还没落账，后续按 pt_audit 前缀
        # 找探针的断言全挂（2026-10-22 实测假成功）。
        for _ in range(24):
            time.sleep(5)
            st, js = call("GET", f"/api/v1/torrents/{tid}", root_tok)
            row = (js.get("data") or {}) if isinstance(js, dict) else {}
            if (row.get("seeders") or 0) > 0:
                rec("自愈", "PASS", f"已建探针种 #{tid} 并挂做种 peer")
                return
        rec("自愈", "WARN", f"探针种 #{tid} 做种已挂但 120s 内未进活种列表")
    else:
        rec("自愈", "WARN", f"探针种 #{tid} 已建但 announce 失败（可能仍不可见）")


