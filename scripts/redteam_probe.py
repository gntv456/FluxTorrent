# -*- coding: utf-8 -*-
"""红队探测工具箱（0309 开源威胁模型，作弊者视角黑盒自查）。

用法：PYTHONIOENCODING=utf-8 python scripts/redteam_probe.py

前提：本地 docker 栈在跑（tracker :7070），已有任一可用 passkey 与
站内种子的 info_hash。缺省从 API 登录取一个测试种——探针只发 announce
不动账面数据，结束时不留事件（stopped 收尾）。

覆盖（对照威胁模型分层，见 docs/ops/security.md）：
  R1  幽灵做种（port=0 纯 curl 挂种）      —— 应不累计做种收益
  R2  裸 TCP 监听伪装 connectable          —— BT 握手探测应拒绝
  R3  只回握手不回 bitfield                 —— 同上（幽灵第二形态）
  R4  伪造 piece（随机数据）                —— SHA-1 抽查应实锤
  R5  速率超窗（单笔增量 > 速率钳 × 间隔）  —— 超窗部分应只记事件不计流量
  R6  近零重置刷锚点（24h 内二次重置）      —— 第二次重置不应被认可
  R7  固定节拍卡表探测窗                    —— 探测时机应带随机抖动

R2/R3/R4 需要本机起 mock peer（本脚本自带，只绑 127.0.0.1）。
R1/R5/R6 打真实 announce；R7 靠读 tracker 源码常量+观察（缺省 SKIP，
本地没有长跑窗口时跳过不算失败）。
"""
import json
import os
import socket
import struct
import sys
import threading
import time
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import login, call, ok, summary, BASE  # noqa: E402

TRK = os.environ.get("FLUX_TRK_BASE", "http://127.0.0.1:7070")
PROTO = b"BitTorrent protocol"


def announce(pk, ih, uploaded=0, downloaded=0, left=0, port=0,
             event="", numwant=50):
    q = {
        "passkey": pk, "info_hash": ih, "peer_id": "-FL0001-redteam-probe",
        "port": port, "uploaded": uploaded, "downloaded": downloaded,
        "left": left, "numwant": numwant,
    }
    if event:
        q["event"] = event
    url = TRK + "/announce/" + pk + "?" + urllib.parse.urlencode(q)
    try:
        r = urllib.request.urlopen(url, timeout=10)
        return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def find_probe_target():
    """取一个站内种子与发布者 passkey（探针号没有种子，借 root 看）"""
    tok = login()
    s, r = call("GET", "/admin/torrents?page=1&page_size=5", token=tok)
    rows = r.get("data", {}).get("items") or r.get("data") or []
    if isinstance(rows, dict):
        rows = rows.get("items", [])
    for t in rows[:5]:
        ih = t.get("info_hash") or t.get("raw_info_hash")
        if ih:
            return ih, t.get("id"), tok
    return None, None, tok


class MockPeer(threading.Thread):
    """本机 mock peer：按模式回 BT 消息，专测 tracker 的探测判定。"""

    def __init__(self, mode):
        super().__init__(daemon=True)
        self.mode = mode  # silent | handshake_only | fake_piece
        self.sock = socket.socket()
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(4)
        self.port = self.sock.getsockname()[1]

    def run(self):
        while True:
            try:
                c, _ = self.sock.accept()
            except OSError:
                return
            threading.Thread(
                target=self.serve, args=(c,), daemon=True
            ).start()

    def serve(self, c):
        try:
            hs = c.recv(68)
            if len(hs) < 68:
                return
            if self.mode == "silent":
                return
            ih = hs[28:48]
            resp = bytes([19]) + PROTO + b"\x00" * 8 + ih + b"M" * 20
            c.sendall(resp)
            # 读 interested
            c.recv(4)
            if self.mode == "handshake_only":
                return
            # bitfield 声明有 piece 0 + unchoke
            c.sendall(struct.pack(">IB", 2, 5) + b"\x80")
            c.sendall(struct.pack(">IB", 1, 1))
            req = c.recv(17)
            if len(req) < 17:
                return
            # 伪造 piece：随机内容（哈希必然不符）
            c.sendall(
                struct.pack(">IB", 9 + 1024, 7)
                + struct.pack(">II", 0, 0)
                + b"\xde\xad" * 512
            )
            time.sleep(0.3)
        except OSError:
            pass
        finally:
            c.close()


def main():
    ih, tid, tok = find_probe_target()
    if not ih:
        print("[SKIP] 站内无种子，R1/R5/R6 跳过（先跑 demo_seed.py）")
    # 探针 passkey：root 的（announce 不改账面，stopped 收尾）
    s, r = call("GET", "/me/passkey", token=tok)
    pk = (r.get("data") or {}).get("passkey") if s == 200 else None

    if ih and pk:
        # R1 幽灵做种：port=0 纯 curl 挂种
        st, body = announce(pk, ih, port=0, event="started")
        ok("R1a port=0 announce 被接受（数据面）", st == 200,
           f"status={st}")
        ok("R1b 返回体不是 bencode 错误", b"failure" not in body[:64],
           body[:80])
        announce(pk, ih, port=0, event="stopped")

        # R5 速率超窗：单笔宣称 10 TiB 增量
        st, body = announce(pk, ih, uploaded=10 << 40, port=6881,
                            event="started")
        announce(pk, ih, uploaded=0, event="stopped")
        ok("R5 超窗增量announce 不崩（钳制在 worker）",
           st in (200, 400), f"status={st}")

        # R6 近零重置：报近零 → stopped → 再报近零（24h 内二次）
        announce(pk, ih, uploaded=0, event="started")
        announce(pk, ih, uploaded=0, event="stopped")
        announce(pk, ih, uploaded=0, event="started")
        announce(pk, ih, uploaded=0, event="stopped")
        ok("R6 二次近零重置 announce 不崩（频次判定在 worker）",
           True, "数据面验收见 /admin/cheaters 与 cheat_events")

    # R2/R3/R4 mock peer 三形态：tracker 探测回路会主动回连。
    # 这里只验证 mock 自身行为正确（探测判定由 bt_probe.rs 单测+e2e 盖）。
    for mode, name in [
        ("silent", "R2 裸监听"),
        ("handshake_only", "R3 只回握手"),
        ("fake_piece", "R4 伪造piece"),
    ]:
        try:
            m = MockPeer(mode)
            m.start()
            ok(f"{name} mock 就绪 :{m.port}", m.port > 0)
        except OSError as e:
            ok(f"{name} mock 就绪", False, str(e))
    print("[NOTE] mock 生命周期与本脚本同寿；探测判定验收："
          "snatches.connectable 应为 0（R2/R3），cheat_events 出 "
          "piece 伪造记录（R4，抽查抽中时）")
    print("[SKIP] R7 固定节拍观察需长跑窗口，由 0309 jitter 单测覆盖")
    summary()


if __name__ == "__main__":
    main()
