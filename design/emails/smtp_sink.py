# -*- coding: utf-8 -*-
"""极简本地 SMTP 捕获器（只监听 127.0.0.1，不投递到任何外部地址）。

用途：把 api 的真实投递路径指向 127.0.0.1:<port>，捕获每封邮件的**原始字节**
（含头部与 MIME 分隔），落成 .eml 供 MIME 结构 / 编码 / 转义核查。

刻意不宣告 8BITMIME / SMTPUTF8：让 lettre 走 quoted-printable 或 base64，
这正是线上会发生的编码路径。

用法：python design/emails/smtp_sink.py [port]
      python design/emails/smtp_sink.py 2526 d:/FluxTorrent/design/emails/_smtp_capture
"""
import os
import socket
import sys
import threading

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 2526
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "_smtp_capture")
os.makedirs(OUT, exist_ok=True)

_seq = [0]
_lock = threading.Lock()
for _f in os.listdir(OUT):
    if _f.startswith("msg") and _f.endswith(".eml"):
        try:
            _seq[0] = max(_seq[0], int(_f[3:-4]))
        except ValueError:
            pass


def save(raw):
    with _lock:
        _seq[0] += 1
        n = _seq[0]
    path = os.path.join(OUT, "msg%02d.eml" % n)
    with open(path, "wb") as f:
        f.write(raw)
    print("saved msg%02d.eml (%d bytes)" % (n, len(raw)), flush=True)


def handle(conn):
    conn.sendall(b"220 sink ESMTP\r\n")
    buf = b""
    in_data = False
    conn.settimeout(90)
    try:
        while True:
            chunk = conn.recv(65536)
            if not chunk:
                break
            if in_data:
                buf += chunk
                end = b"\r\n.\r\n"
                if end in buf:
                    raw, buf = buf.split(end, 1)
                    save(raw + b"\r\n")
                    in_data = False
                    conn.sendall(b"250 2.0.0 queued\r\n")
                continue
            buf += chunk
            while b"\r\n" in buf:
                line, buf = buf.split(b"\r\n", 1)
                up = line.strip().upper()
                if up.startswith(b"DATA"):
                    in_data = True
                    conn.sendall(b"354 end with .\r\n")
                    break
                if up.startswith(b"QUIT"):
                    conn.sendall(b"221 bye\r\n")
                    return
                if up.startswith(b"EHLO"):
                    conn.sendall(b"250-sink\r\n250 SIZE 10485760\r\n")
                elif up.startswith(b"HELO"):
                    conn.sendall(b"250 sink\r\n")
                elif up.startswith(b"RSET"):
                    conn.sendall(b"250 ok\r\n")
                elif up.startswith(b"NOOP"):
                    conn.sendall(b"250 ok\r\n")
                else:
                    conn.sendall(b"250 2.1.0 ok\r\n")
    except Exception as exc:  # noqa: BLE001
        print("conn error: %r" % (exc,), flush=True)
    finally:
        try:
            conn.close()
        except Exception:  # noqa: BLE001
            pass


def main():
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", PORT))
    srv.listen(16)
    print("sink listening on 127.0.0.1:%d -> %s" % (PORT, OUT), flush=True)
    while True:
        conn, _ = srv.accept()
        threading.Thread(target=handle, args=(conn,), daemon=True).start()


if __name__ == "__main__":
    main()
