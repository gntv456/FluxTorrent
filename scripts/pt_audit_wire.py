# -*- coding: utf-8 -*-
"""announce 线上字节的解码小工具（给 pt_audit_tracker_events.py 用）。

只有纯函数、import 期零副作用——闸门脚本被 import 会把整套断言当副作用
跑一遍（10-06 踩过），可复用的判据部件因此单独放这里。
"""

# ---------- 极简 bdecode（只用得到 dict/list/int/bytes） ----------
def bdec(d, i=0):
    c = d[i:i + 1]
    if c == b"d":
        i += 1
        out = {}
        while d[i:i + 1] != b"e":
            k, i = bdec(d, i)
            v, i = bdec(d, i)
            out[k] = v
        return out, i + 1
    if c == b"l":
        i += 1
        out = []
        while d[i:i + 1] != b"e":
            v, i = bdec(d, i)
            out.append(v)
        return out, i + 1
    if c == b"i":
        j = d.index(b"e", i)
        return int(d[i + 1:j]), j + 1
    j = d.index(b":", i)
    n = int(d[i:j])
    return d[j + 1:j + 1 + n], j + 1 + n

def failure_reason(body):
    try:
        d, _ = bdec(body)
        v = d.get(b"failure reason")
        return v.decode("utf-8", "replace") if isinstance(v, bytes) else None
    except Exception:  # noqa: BLE001
        return None

def parse_peers(body, key):
    """compact 列表 → [(ip, port)]；key=b'peers' 6 字节/条、b'peers6' 18。"""
    try:
        d, _ = bdec(body)
    except Exception:  # noqa: BLE001
        return []
    raw = d.get(key) or b""
    size = 6 if key == b"peers" else 18
    out = []
    for i in range(0, len(raw) - size + 1, size):
        chunk = raw[i:i + size]
        octets = chunk[:size - 2]
        p = int.from_bytes(chunk[size - 2:], "big")
        if size == 6:
            ip = ".".join(str(b) for b in octets)
        else:
            grp = [octets[j:j + 2].hex() for j in range(0, 16, 2)]
            ip = ":".join(grp).replace(":0000", ":0")
        out.append((ip, p))
    return out

def announce_counts(body):
    """announce 响应里的 (complete, incomplete)——这两个数由内存实时表算出，
    不像 scrape 只认 60s 白名单快照（10-08 记过这条，本轮又被它假红了一次）。"""
    try:
        d, _ = bdec(body)
        if b"complete" not in d:
            return None
        return (d[b"complete"], d.get(b"incomplete", 0))
    except Exception:  # noqa: BLE001
        return None
