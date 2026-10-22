# -*- coding: utf-8 -*-
"""闸门共用造料纯函数（脚本 500 行门禁）：bencode / 种子构造 / multipart。

与真实客户端同构的最小编码器，仅供 pt_audit_* 闸门造上传语料用；
tracker 侧的解析判据在 pt_audit_wire.py，这里只管「造」。
"""
import hashlib
import time


def benc(obj):
    if isinstance(obj, int):
        return b"i%de" % obj
    if isinstance(obj, bytes):
        return b"%d:%s" % (len(obj), obj)
    if isinstance(obj, str):
        return benc(obj.encode())
    if isinstance(obj, list):
        return b"l" + b"".join(benc(x) for x in obj) + b"e"
    if isinstance(obj, dict):
        out = b"d"
        for k in sorted(obj.keys()):
            out += benc(k) + benc(obj[k])
        return out + b"e"
    raise TypeError(type(obj))


# 本轮唯一盐（任务 #13）：服务端按 pieces_hash 判重复发布，写死摘要会让
# 第 2 轮起全数撞「同内容种子已存在」。
PIECE_SALT = ("%f" % time.time()).encode()

def make_torrent(name, total=4096, piece=16384):
    count = max(1, (total + piece - 1) // piece)
    info = {b"name": name.encode(), b"piece length": piece,
            b"pieces": hashlib.sha1(b"piece:" + PIECE_SALT).digest() * count,
            b"private": 1, b"length": total}
    return benc({b"info": info,
                 b"announce": b"http://127.0.0.1:7070/announce/x"})


BOUNDARY = "----fluxgate1"


def multipart(fields, content, filename="probe.torrent"):
    out = b""
    for k, v in fields.items():
        if v is None:
            continue
        out += ('--%s\r\nContent-Disposition: form-data; name="%s"\r\n'
                '\r\n%s\r\n' % (BOUNDARY, k, v)).encode()
    out += ('--%s\r\nContent-Disposition: form-data; name="file"; '
            'filename="%s"\r\n'
            "Content-Type: application/x-bittorrent\r\n\r\n"
            % (BOUNDARY, filename)).encode()
    out += content + b"\r\n" + ("--%s--\r\n" % BOUNDARY).encode()
    return out
