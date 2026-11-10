"""FluxTorrent 辅种适配 · 契约验证（打桩真实站点响应形状）。

跑的是**插件真实代码**（`CrossSeedHelper.get_target_torrent`），对面是
一个模拟 FluxTorrent 响应的本地 HTTP 服务。验证三件事：

1. 插件发出的请求**形状**对不对（路径、请求头、请求体）；
2. 插件能**正确解析**站点响应（拿到真 torrent_id，不是假命中）；
3. 上游方言**没有被改坏**（非 FluxTorrent 站点仍走老路径）。

这不需要 Docker：站点响应形状由 FluxTorrent 的 `dto::ok` + `crossseed`
端点契约决定（`{"code":0,"message":"ok","data":{"results":[...]}}`），
本地按契约复刻即可。

用法：
    python plugins/tests/test_contract_flux.py
"""

from __future__ import annotations

import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from conftest import _load_plugin_module  # noqa: E402

# 站点会回的真实形状（取自 apps/api/src/dto.rs 的 ok() 与
# apps/api/src/openapi_http/crossseed/mod.rs 的 lookup 响应体）
SITE_PIECES = "db6aab38b13a6ae52542471a4a9fef2507973831"
SITE_TORRENT_ID = 12345
SITE_ENVELOPE = {
    "code": 0,
    "message": "ok",
    "request_id": "test-req",
    "data": {
        "results": [
            {
                "match": "pieces_sha1",
                "query": SITE_PIECES,
                "torrent_id": SITE_TORRENT_ID,
                "name": "Some.Release.2024.1080p",
                "size": 1234567890,
                "download_url": f"/api/v1/crossseed/dl/{SITE_TORRENT_ID}",
            }
        ]
    },
}

# 记录收到的请求，供断言
RECEIVED: list = []


# 站点 302 的落点：/api/v1/downloads/{id}?token=fxk_xxx（临时凭证）
DL_LOCATION = f"/api/v1/downloads/{SITE_TORRENT_ID}?token=fxk_test_key"
# 一个最小合法 bencode（d8:announce…）
TORRENT_BYTES = b"d8:announce20:http://tr/ann0e"


class _FakeFlux(BaseHTTPRequestHandler):
    """扮演 FluxTorrent 的 /api/v1/crossseed/lookup 与 /crossseed/dl/{id}。"""

    def log_message(self, *args):
        """静音。"""

    def do_POST(self):
        """接收查询请求，按站点契约回响应。"""
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length)
        body = json.loads(raw.decode("utf-8")) if raw else {}
        RECEIVED.append(
            {
                "path": self.path,
                "authorization": self.headers.get("Authorization"),
                "content_type": self.headers.get("Content-Type"),
                "body": body,
            }
        )
        out = json.dumps(SITE_ENVELOPE).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def do_GET(self):
        """模拟站点的 302 下载链：dl 口跳到带临时凭证的下载端点。"""
        RECEIVED.append(
            {
                "path": self.path,
                "authorization": self.headers.get("Authorization"),
            }
        )
        if self.path.startswith("/api/v1/crossseed/dl/"):
            # 站点行为：302 + Location（凭证在 query 里）
            self.send_response(302)
            self.send_header("Location", DL_LOCATION)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        # 下载端点：200 + .torrent 字节
        self.send_response(200)
        self.send_header("Content-Type", "application/x-bittorrent")
        self.send_header("Content-Length", str(len(TORRENT_BYTES)))
        self.end_headers()
        self.wfile.write(TORRENT_BYTES)


def _run() -> int:
    """跑契约验证。"""
    module, _mod_name = _load_plugin_module(
        Path(__file__).resolve().parents[1] / "crossseed",
        "flux_crossseed_contract",
    )
    helper = module.CrossSeedHelper
    cfg_cls = module.CSSiteConfig

    server = HTTPServer(("127.0.0.1", 0), _FakeFlux)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    failures: list = []

    def check(cond: bool, msg: str):
        """断言并收集失败（不中断，好把问题一次列全）。"""
        if cond:
            print(f"  [OK] {msg}")
        else:
            print(f"  [FAIL] {msg}")
            failures.append(msg)

    try:
        site = cfg_cls(
            name=module.FLUX_SITE_NAME,
            url=f"http://127.0.0.1:{port}",
            passkey="fxo_contract_token",
        )

        print("\n=== 辅种适配 · 站点契约验证 ===\n")

        # 1) 请求形状
        print("[1] 插件发出的请求形状")
        RECEIVED.clear()
        infos, err = helper.get_target_torrent(site, [SITE_PIECES])
        check(err is None, f"查询无错误（err={err!r}）")
        check(len(RECEIVED) == 1, "发出 1 次请求")
        if RECEIVED:
            req = RECEIVED[0]
            check(
                req["path"] == "/api/v1/crossseed/lookup",
                f"路径 = {req['path']}",
            )
            check(
                req["authorization"] == "Token fxo_contract_token",
                f"凭据走 Authorization 头 = {req['authorization']!r}",
            )
            check(
                "passkey" not in req["body"],
                f"请求体不含 passkey（body={req['body']}）",
            )
            check(
                req["body"] == {"pieces_sha1": [SITE_PIECES]},
                f"请求体 = {req['body']}",
            )

        # 2) 响应解析
        print("\n[2] 插件解析站点响应")
        check(len(infos) == 1, f"解析出 1 条命中（实际 {len(infos)}）")
        if infos:
            hit = infos[0]
            check(
                hit.torrent_id == str(SITE_TORRENT_ID),
                f"torrent_id = {hit.torrent_id}",
            )
            check(
                hit.pieces_hash == SITE_PIECES,
                f"pieces_hash = {hit.pieces_hash}",
            )
            check(
                hit.site_name == module.FLUX_SITE_NAME,
                f"site_name = {hit.site_name}",
            )
            # 上游后续拿 pieces_hash 拼下载 URL
            dl = site.get_torrent_url(hit.torrent_id)
            check(
                dl.endswith(f"/api/v1/crossseed/dl/{SITE_TORRENT_ID}"),
                f"下载 URL = {dl}",
            )

        # 3) 上游方言未被改坏
        print("\n[3] 上游方言未被改坏")
        upstream = cfg_cls(
            name="青蛙", url=f"http://127.0.0.1:{port}/")
        check(
            upstream.get_api_url()
            == f"http://127.0.0.1:{port}/api/pieces-hash",
            f"上游查询口 = {upstream.get_api_url()}",
        )
        check(
            upstream.get_torrent_url("7")
            == f"http://127.0.0.1:{port}/download.php?id=7&passkey=None",
            f"上游下载口 = {upstream.get_torrent_url('7')}",
        )

        # 4) 下载链路（302 → 临时凭证 → .torrent）
        print("\n[4] 下载链路跟随 302")
        RECEIVED.clear()
        site_for_dl = cfg_cls(
            name=module.FLUX_SITE_NAME,
            url=f"http://127.0.0.1:{port}",
            passkey="fxo_contract_token",
        )
        dl_url = site_for_dl.get_torrent_url(str(SITE_TORRENT_ID))
        content, dl_err = module.CrossSeed.__dict__[
            "_CrossSeed__download_flux_torrent"].__func__(
                dl_url, site_for_dl)
        check(dl_err == "", f"下载无错误（err={dl_err!r}）")
        check(
            content == TORRENT_BYTES,
            f"拿到 .torrent 字节（{len(content or b'')} 字节）",
        )
        check(
            len(RECEIVED) >= 2,
            f"跟随了重定向（收到 {len(RECEIVED)} 次请求）",
        )
        if len(RECEIVED) >= 2:
            check(
                RECEIVED[0]["path"] == f"/api/v1/crossseed/dl/{SITE_TORRENT_ID}",
                f"第一跳 = {RECEIVED[0]['path']}",
            )
            check(
                RECEIVED[0]["authorization"] == "Token fxo_contract_token",
                f"第一跳带 Authorization 头 = "
                f"{RECEIVED[0]['authorization']!r}",
            )
            check(
                RECEIVED[-1]["path"].startswith("/api/v1/downloads/"),
                f"落点 = {RECEIVED[-1]['path']}",
            )

        print()
        if failures:
            print(f"[FAIL] {len(failures)} 项未通过")
            return 1
        print("[OK] 插件与 FluxTorrent 站点契约一致")
        return 0
    finally:
        server.shutdown()


if __name__ == "__main__":
    sys.exit(_run())
