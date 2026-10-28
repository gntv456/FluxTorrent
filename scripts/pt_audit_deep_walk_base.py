# -*- coding: utf-8 -*-
"""全站深测探针的共用底座（从 pt_audit_deep_walk.py 拆出，500 行门禁）。

只有工具与「把探针种喂活」这一段前提，没有任何旅程判据：铸 JWT、直连库读数、
统一记结果（expect/ok/data_of/rows_of），以及造种子 + announce + 等它进活种
列表——分享率与下载两条链没有这个前提走不动。

`RESULTS`/`CSRF` 是模块级可变状态：三支域文件都往同一个列表里记，汇总与
退出码只在入口文件做，所以这里不复制列表、也不清它。
"""
from __future__ import annotations

import hashlib
import json
import os
import re
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
VIP_UID = 7346
VIP_PASSKEY = "a17b2c3d4e5f60718293a4b5c6d7e8f9"

RESULTS: list[tuple[str, str, str]] = []
CSRF: list[str] = [""]



# ---------------------------------------------------------------- 基础助手

def csrf_handshake(token: str) -> None:
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
        with urllib.request.urlopen(req, timeout=25) as r:
            payload = r.read()
            if raw:
                return r.status, payload
            try:
                return r.status, json.loads(payload)
            except Exception:
                return r.status, payload[:300].decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        payload = e.read()
        try:
            return e.code, json.loads(payload)
        except Exception:
            return e.code, payload[:300].decode("utf-8", "replace")
    except Exception as e:
        return 0, str(e)


def expect(domain: str, name: str, cond: bool, detail: str = "") -> bool:
    rec(domain, "PASS" if cond else "FAIL",
        f"{name}{('｜' + detail) if detail else ''}")
    return cond


def ok(domain: str, name: str, st: int, js: object) -> bool:
    if st != 200 or not isinstance(js, dict) or js.get("code") != 0:
        return expect(domain, name, False, f"st={st} body={str(js)[:160]}")
    return expect(domain, name, True)


def data_of(js: object) -> dict:
    return (js or {}).get("data") or {} if isinstance(js, dict) else {}


def rows_of(js: object) -> list:
    """取行集：兼容三种信封——data 裸数组（消息中心）、data.items
    （种子列表）、data.rows（订单/分页面板）。"""
    d = (js or {}).get("data") if isinstance(js, dict) else None
    if isinstance(d, list):
        return d
    if isinstance(d, dict):
        return d.get("items") or d.get("rows") or []
    return []


def wait_worker_tick(rounds: int = 12, secs: float = 8.0) -> None:
    """worker consume_announce 是 60s tick：等一个完整周期让计数落账。"""
    for _ in range(rounds):
        time.sleep(secs)



# ---------------------------------------------------------------- 种子/announce 助手

def _benc(obj, out=None):
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


def upload_probe_torrent(tok: str, name: str,
    descr: str = "全站深测探针种") -> tuple[int, str] | None:
    crlf = chr(13) + chr(10)
    dq = chr(34)
    b = "----deepauditup"
    parts = b""
    for k, v in {
        "category_id": 1,
        "name": name,
        "small_descr": descr,
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
            tid = d.get("id")
            # P-444 回收登记：这颗探针种是本底座造的，登记后由
            # pt_audit_lib.summary() 按 id 精确删。底座被 deep_walk /
            # deep_walk_member / deep_walk_admin 星号导入，所以登记收在
            # 这一处就覆盖全部调用方（各入口自己再写一遍必然漏）。
            try:
                from pt_audit_lib import note_torrent
                note_torrent(tid)
            except Exception as e:  # noqa: BLE001
                rec("清理", "WARN", "回收登记失败，这颗种会留成残料：%s"
                    % str(e)[:60])
            return tid, d.get("info_hash")
    except Exception as e:
        rec("探针", "WARN", f"上传探针种失败：{str(e)[:80]}")
        return None


def tracker_announce(passkey: str, info_hash_hex: str, size: int = 0,
                     downloaded: int = 0, uploaded: int = 0,
                     event: str = "started") -> tuple[bool, str]:
    q = urllib.parse.urlencode({
        "info_hash": bytes.fromhex(info_hash_hex),
        "peer_id": b"-pt0001-aud1tv1p0001",
        "port": 6881,
        "uploaded": uploaded,
        "downloaded": downloaded,
        "left": 0 if downloaded >= size and size > 0 else max(size - downloaded,
            0),
        "compact": 1,
        "event": event,
        "numwant": 10,
    }, quote_via=urllib.parse.quote)  # 严格 %XX：默认 quote_plus 把 0x20
    # 编成 '+'，tracker 2026-10-25 前不认（含空格字节的 info_hash 全挂）
    try:
        with urllib.request.urlopen(
            f"{TRACKER}/announce/{passkey}?{q}", timeout=15
        ) as r:
            body = r.read()[:160].decode("utf-8", "replace")
            # tracker 的拒绝也是 HTTP 200 + bencoded failure reason——
            # 不当成功，原样带回供断言判读
            if "failure reason" in body:
                return False, body
            return True, body
    except urllib.error.HTTPError as e:
        return False, f"HTTP {e.code}: {e.read()[:120].decode('utf-8', 'replace')}"
    except Exception as e:
        return False, str(e)[:120]


def ensure_alive_probe(root_tok: str) -> tuple[int, str] | None:
    """确保至少一枚活种可测：无则上传 + 两段式 announce + 等 worker tick。"""
    st, js = call("GET", "/api/v1/torrents?limit=1", root_tok)
    items = (data_of(js)).get("items", [])
    if items:
        return items[0].get("id"), items[0].get("info_hash")
    name = f"deep_alive.{int(time.time())}"
    out = upload_probe_torrent(root_tok, name)
    if not out:
        return None
    tid, ih = out
    tracker_announce(VIP_PASSKEY, ih, size=1_048_576, downloaded=0)
    time.sleep(2)
    tracker_announce(VIP_PASSKEY, ih, size=1_048_576, downloaded=1_048_576)
    wait_worker_tick(rounds=10, secs=8)
    return tid, ih


