#!/usr/bin/env python3
"""IYUU 查重/上报桥接器 —— 工具侧，不改站点代码。

背景：xqw8889/np-iyuu-push-torrent 是 NexusPHP 站内 PHP 插件，
依赖 Laravel 插件宿主、`do_action` 钩子、NP 的 ORM 与本地
.torrent 文件，FluxTorrent（Rust + Next.js）装不了它。
这个脚本把其中「与站点耦合」的部分换成 FluxTorrent 已有的开放
API，IYUU 侧协议逐字照抄上游插件（find.iyuu.cn/api/）。

站点侧用到的端点（都已在线上确认可达）：
  GET  /api/v1/compat/nexusphp/torrents.json?page=&pagesize=
       -> data.items[].info_hash（read scope 的 fxo_ Token）
  POST /api/v1/crossseed/iyuu      （IYUU 格式指纹查询，备选）
  GET  /api/v1/crossseed/iyuu/site （站点自描述，公开）
IYUU 侧：
  POST find.iyuu.cn/api/searchhash    查重
  POST find.iyuu.cn/api/createTorrent 上报本站种子指纹

凭据全部走环境变量，不落盘：
  FLUX_BASE       默认 https://www.hxpt.org
  FLUX_TOKEN      fxo_ 开头的 Token，需 read scope
  IYUU_SIGN_KEY   IYUU 官方签发的 sign_key
  IYUU_SITE_NAME  IYUU 官方登记的 site_name
  IYUU_API_BASE   可选，默认 https://find.iyuu.cn/api/

用法：
  python scripts/iyuu_bridge.py check <info_hash>
  python scripts/iyuu_bridge.py report [--dry-run] [--max-pages N]
  python scripts/iyuu_bridge.py selftest
"""

import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

UA = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
      "AppleWebKit/537.36 (KHTML, like Gecko) "
      "Chrome/123.0.0.0 Safari/537.36")

FLUX_BASE = os.environ.get("FLUX_BASE", "https://www.hxpt.org").rstrip("/")
FLUX_TOKEN = os.environ.get("FLUX_TOKEN", "")
IYUU_API = os.environ.get(
    "IYUU_API_BASE", "https://find.iyuu.cn/api/").rstrip("/") + "/"
SIGN_KEY = os.environ.get("IYUU_SIGN_KEY", "")
SITE_NAME = os.environ.get("IYUU_SITE_NAME", "")


def _http_json(req, timeout=20):
    """发一次请求并解 JSON；HTTP 错误带 body 一起抛。"""
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", "replace")[:300]
        raise RuntimeError(f"HTTP {exc.code}: {body}") from exc


def _php_json(value):
    """对齐 PHP json_encode：无多余空格，保持键的插入顺序。"""
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


def _sign(extra):
    """上游签名：sign = sha1(timestamp + sign_key)，timestamp 同一份。"""
    ts = int(time.time())
    fields = dict(extra)
    fields["sign"] = hashlib.sha1(
        f"{ts}{SIGN_KEY}".encode("utf-8")).hexdigest()
    fields["timestamp"] = str(ts)
    fields["site"] = SITE_NAME
    return fields


def _iyuu_post(path, fields):
    data = urllib.parse.urlencode(fields).encode("utf-8")
    req = urllib.request.Request(
        IYUU_API + path, data=data,
        headers={"User-Agent": UA,
                 "Content-Type": "application/x-www-form-urlencoded"})
    return _http_json(req)


def _require_iyuu_cred():
    if not SIGN_KEY or not SITE_NAME:
        raise RuntimeError(
            "缺 IYUU_SIGN_KEY / IYUU_SITE_NAME。"
            "这两个值只能向 IYUU 官方申请，代码无法代劳。")


def check(info_hash):
    """查重：按 info_hash 问 IYUU 中央，列出其他站的同种。"""
    _require_iyuu_cred()
    h = info_hash.strip().lower()
    if len(h) != 40 or any(c not in "0123456789abcdef" for c in h):
        raise RuntimeError("info_hash 必须是 40 位 hex")
    packed = _php_json([h])
    fields = _sign({
        "hash": packed,
        "sha1": hashlib.sha1(packed.encode("utf-8")).hexdigest(),
    })
    resp = _iyuu_post("searchhash", fields)
    if resp.get("msg") != 200:
        return {"hit": False, "msg": resp.get("msg")}
    rows = []
    for item in resp.get("data") or []:
        rows.append({
            "site": item.get("nickname"),
            "torrent_id": item.get("torrent_id"),
            "url": _detail_url(item),
            "tips": item.get("tips"),
            "updated": item.get("timestamp"),
        })
    return {"hit": True, "rows": rows}


def _detail_url(item):
    """上游插件用 http:// 拼链；这里保留 https 优先。"""
    base = (item.get("base_url") or "").strip().rstrip("/")
    tail = item.get("details_url")
    if not base or not tail:
        return None
    return f"https://{base}/{str(tail).lstrip('/')}"


def _flux_page(page, page_size):
    qs = urllib.parse.urlencode({"page": page, "pagesize": page_size})
    url = (f"{FLUX_BASE}/api/v1/compat/nexusphp/"
           f"torrents.json?{qs}")
    req = urllib.request.Request(url, headers={
        "User-Agent": UA,
        "Authorization": f"Token {FLUX_TOKEN}",
    })
    return _http_json(req)


def iter_info_hashes(max_pages=400, page_size=50):
    """分页拉全站 info_hash。page≤400、pagesize≤50 是服务端硬上界。"""
    if not FLUX_TOKEN:
        raise RuntimeError("缺 FLUX_TOKEN（fxo_ Token，需 read scope）")
    page_size = max(1, min(50, page_size))
    seen = set()
    for page in range(1, max(1, min(400, max_pages)) + 1):
        env = _flux_page(page, page_size)
        if env.get("code") not in (0, None):
            raise RuntimeError(f"站点返回错误：{env}")
        data = env.get("data") or {}
        items = data.get("items") or []
        for item in items:
            h = (item.get("info_hash") or "").strip().lower()
            if h and h not in seen:
                seen.add(h)
                yield h
        if not items or not data.get("has_more"):
            return


def report(max_pages=400, page_size=50, dry_run=False):
    """上报本站种子指纹给 IYUU 中央（等价于插件的 iyuu:update --all）。"""
    if not dry_run:
        _require_iyuu_cred()
    total = ok = fail = 0
    for h in iter_info_hashes(max_pages, page_size):
        total += 1
        if dry_run:
            print(f"[dry-run] would report {h}")
            continue
        # 上游 Bencode::reseed() 只产出 info_hash 一项。
        fields = _sign({"data": _php_json({"info_hash": h})})
        try:
            resp = _iyuu_post("createTorrent", fields)
            if resp.get("msg") == 200:
                ok += 1
            else:
                fail += 1
                print(f"[fail] {h}: {resp.get('msg')}", file=sys.stderr)
        except RuntimeError as exc:
            fail += 1
            print(f"[fail] {h}: {exc}", file=sys.stderr)
    return {"total": total, "ok": ok, "fail": fail}


def selftest():
    """不触网：校验签名、字段形状、分页终止与查重解析。"""
    import unittest.mock as mock

    global SIGN_KEY, SITE_NAME, FLUX_TOKEN
    old = (SIGN_KEY, SITE_NAME, FLUX_TOKEN)
    SIGN_KEY, SITE_NAME, FLUX_TOKEN = "unit-test-key", "hxpt", "fxo_test"
    try:
        with mock.patch.object(time, "time", return_value=1700000000):
            f = _sign({"data": _php_json({"info_hash": "a" * 40})})
        assert f["data"] == '{"info_hash":"' + "a" * 40 + '"}'
        assert f["sign"] == hashlib.sha1(
            b"1700000000unit-test-key").hexdigest()
        assert f["timestamp"] == "1700000000" and f["site"] == "hxpt"

        packed = _php_json(["b" * 40])
        assert packed == '["' + "b" * 40 + '"]'
        assert hashlib.sha1(packed.encode()).hexdigest() == hashlib.sha1(
            b'["' + b"b" * 40 + b'"]').hexdigest()

        class FakeResp:
            def __init__(self, payload):
                self.payload = payload

            def read(self):
                return json.dumps(self.payload).encode()

            def __enter__(self):
                return self

            def __exit__(self, *args):
                return False

        pages = [
            {"code": 0, "data": {
                "items": [{"info_hash": "a" * 40},
                          {"info_hash": "A" * 40},
                          {"info_hash": "c" * 40}],
                "has_more": True}},
            {"code": 0, "data": {
                "items": [{"info_hash": "d" * 40}],
                "has_more": False}},
        ]
        with mock.patch.object(urllib.request, "urlopen",
                               side_effect=[FakeResp(p) for p in pages]):
            got = list(iter_info_hashes(max_pages=2, page_size=50))
        assert got == ["a" * 40, "c" * 40, "d" * 40], got

        hit = {"msg": 200, "data": [{
            "nickname": "某站", "torrent_id": 42,
            "details_url": "details.php?id=42",
            "base_url": "example.org", "tips": "free",
            "timestamp": "2026-11-11 12:00:00"}]}
        with mock.patch.object(urllib.request, "urlopen",
                               return_value=FakeResp(hit)):
            out = check("e" * 40)
        assert out["hit"] and out["rows"][0]["url"] == \
            "https://example.org/details.php?id=42"
        print("selftest OK: 签名 / JSON 形状 / 分页 / 查重解析")
    finally:
        SIGN_KEY, SITE_NAME, FLUX_TOKEN = old


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p_check = sub.add_parser("check", help="按 info_hash 查其他站点同种")
    p_check.add_argument("info_hash")
    p_rep = sub.add_parser("report", help="上报本站种子指纹")
    p_rep.add_argument("--dry-run", action="store_true")
    p_rep.add_argument("--max-pages", type=int, default=400)
    p_rep.add_argument("--page-size", type=int, default=50)
    sub.add_parser("selftest", help="不触网的契约自测")
    args = ap.parse_args()

    try:
        if args.cmd == "check":
            out = check(args.info_hash)
        elif args.cmd == "report":
            out = report(args.max_pages, args.page_size, args.dry_run)
        else:
            selftest()
            return 0
        print(json.dumps(out, ensure_ascii=False, indent=2))
        return 0
    except (RuntimeError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
