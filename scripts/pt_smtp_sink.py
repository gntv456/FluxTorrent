# -*- coding: utf-8 -*-
"""闸门共用的 SMTP 诱捕信箱（批三十四起从 AZ 拆出，供需要读「用户真收到的
邮件」的闸门复用）。

为什么要有它：找回密码 / 邮箱激活这类链，接口 200 不代表用户能用——只有落进
信箱的那串字符才算数。所以判据一律取自诱捕容器，不看 API 的返回值。

两个坑（都是踩过才知道的）：
  1. 诱捕容器必须跑进 compose 网络。宿主上开端口对容器是 SYN 黑洞（Docker
     Desktop 防火墙），表现为「发信失败」的假红。
  2. 捕获内容走容器 stdout，由 `docker logs` 读回；**不要**写 bind mount 里
     的文件——宿主是 Windows 时容器内没有那条宿主路径，写盘异常被 sink 的
     `except: pass` 静静吞掉，表现为「一封都没抓到」。同理，`docker logs` 不
     能用 text=True：通用换行会把邮件里的 CRLF 折成 LF，而「头部与正文之间
     那个空行」恰恰是 \\r\\n\\r\\n，折了就再也切不出正文。

用法：start_sink() → setv("smtp_host", SINK_NAME) 等四键 → selftest()（信箱
通电正控）→ 触发业务 → wait_mails(n) / find_link("/reset")；退出前
stop_sink()（AZ 的 atexit 里已挂）。
"""
import base64
import os
import quopri
import re
import subprocess
import time

SINK_NAME = "flux-audit-smtp-sink"
SINK_DIR = os.path.join(os.environ.get("TEMP", "/tmp"), "flux_audit_smtp")
_started = False

SINK_SRC = """import socket, sys, threading
lock = threading.Lock()

def handle(c):
    c.sendall(b"220 sink ESMTP\\r\\n")
    buf = b""
    data = False
    c.settimeout(60)
    try:
        while True:
            ch = c.recv(4096)
            if not ch:
                break
            if data:
                buf += ch
                if b"\\r\\n.\\r\\n" in buf or b"\\n.\\n" in buf:
                    with lock:
                        sys.stdout.buffer.write(b"===== MSG =====\\n")
                        sys.stdout.buffer.write(buf)
                        sys.stdout.buffer.flush()
                    buf = b""
                    data = False
                    c.sendall(b"250 queued\\r\\n")
                continue
            up = ch.strip().upper()
            if up.startswith(b"DATA"):
                data = True
                c.sendall(b"354 go\\r\\n")
            elif up.startswith(b"QUIT"):
                c.sendall(b"221 bye\\r\\n")
                break
            elif up.startswith(b"EHLO") or up.startswith(b"HELO"):
                c.sendall(b"250-sink\\r\\n250 ok\\r\\n")
            else:
                c.sendall(b"250 ok\\r\\n")
    except Exception:
        pass
    finally:
        try:
            c.close()
        except Exception:
            pass

s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("0.0.0.0", 2525))
s.listen(8)
print("sink up", flush=True)
while True:
    threading.Thread(target=handle, args=(s.accept()[0],),
                     daemon=True).start()
"""


def _run(args, timeout=60):
    return subprocess.run(args, capture_output=True, text=True,
                          encoding="utf-8", timeout=timeout)


def net_name(api_container="flux-api"):
    r = _run(["docker", "inspect", api_container, "--format",
              "{{range $k,$v := .NetworkSettings.Networks}}{{$k}}{{end}}"])
    return (r.stdout or "").strip() or "docker_default"


def start_sink(api_container="flux-api"):
    """拉起诱捕容器，返回是否成功（失败时调用方应当整体判红，别往下蒙）。"""
    global _started
    os.makedirs(SINK_DIR, exist_ok=True)
    with open(os.path.join(SINK_DIR, "sink.py"), "w", encoding="utf-8") as f:
        f.write(SINK_SRC)
    chk = _run(["docker", "ps", "-a", "--format", "{{.Names}}"])
    if SINK_NAME in (chk.stdout or "").split():
        _run(["docker", "rm", "-f", SINK_NAME])
    r = _run(["docker", "run", "-d", "--name", SINK_NAME, "--network",
              net_name(api_container), "-v", "%s:/sink" % SINK_DIR,
              "python:3.12-alpine", "python", "/sink/sink.py"])
    _started = r.returncode == 0
    print("   [sink] 启动 %s（网=%s）" % (
        "OK" if _started else "FAIL " + (r.stderr or "")[:150],
        net_name(api_container)))
    time.sleep(3)
    return _started


def stop_sink():
    global _started
    if _started:
        _run(["docker", "rm", "-f", SINK_NAME])
        _started = False


def reachable(api_container="flux-api"):
    """基础设施正控：api 容器到诱捕容器的 2525 必须真通，否则后面全是假红。"""
    if not _started:
        return False, "sink 未启动"
    r = _run(["docker", "exec", api_container, "sh", "-c",
              "wget -T 3 -O /dev/null http://%s:2525/ 2>&1 | head -1"
              % SINK_NAME])
    txt = (r.stdout or "") + (r.stderr or "")
    return ("Connection refused" not in txt
            and "bad address" not in txt), txt[:120]


def sink_raw():
    r = subprocess.run(["docker", "logs", SINK_NAME], stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE)
    return (r.stdout or b"").decode("utf-8", "replace")


def selftest():
    """信箱通电正控：自己往诱捕口发一封，必须能在 mails() 里看到它。
    缺这一步就分不清「0 封」到底是站点没发信，还是信箱根本没接上（本模块
    的前一版正是后者：写 bind mount 文件在 Windows 宿主下静默失败，整条
    「读真邮件」的判据空转了一整轮红线）。"""
    prev = len(mails())
    if not prev and not _started:
        return False, "sink 未启动"
    code = ("import smtplib;"
            "c=smtplib.SMTP('%s',2525,timeout=8);"
            "c.mail('sink-selftest@local');c.rcpt('gate@local');"
            "print(c.data('Subject: AZ-SELFCTEST-BODY')[0])" % SINK_NAME)
    r = subprocess.run(["docker", "run", "--rm", "--network", net_name(),
                        "python:3.12-alpine", "python", "-c", code],
                       capture_output=True, text=True, timeout=120)
    ok_send = "250" in (r.stdout or "")
    # 标记只在头部（自测那封没有正文，正文分隔空行都没有），所以查整块
    seen = wait_mails(prev) and any("AZ-SELFCTEST-BODY" in m["raw"]
                                    for m in mails())
    detail = "发信=%s 回读=%s rc=%s" % (ok_send, seen, r.returncode)
    return ok_send and seen, detail[:160]


def mails():
    """诱捕到的邮件列表（base64 / quoted-printable 都解回可读文本）。"""
    out = []
    for blk in sink_raw().split("===== MSG =====")[1:]:
        head, _, body = blk.partition("\r\n\r\n")
        body = body.strip()
        hl = head.lower()
        text = body
        try:
            if "base64" in hl:
                text = base64.b64decode(
                    re.sub(r"\s+", "", body.split("===== ")[0])).decode(
                        "utf-8", "replace")
            elif "quoted-printable" in hl:
                # lettre 对中文正文默认走 quoted-printable：URL 里的 = 会被
                # 写成 =3D、长行用 =\r\n 折行，不解开就找不到链接
                text = quopri.decodestring(
                    body.encode("utf-8", "ignore")).decode("utf-8", "replace")
        except Exception:  # noqa: BLE001
            text = body
        out.append({"head": head, "body": text, "raw": blk})
    return out


def wait_mails(prev, timeout=12):
    """发信常是 rt::spawn 的副作用，接口先返回：轮询到「真多出一封」为止。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        if len(mails()) > prev:
            return True
        time.sleep(0.5)
    return len(mails()) > prev


def find_link(needle):
    """取链接：**从最新一封往前找**（用户点的永远是最后收到的那封；站内
    重发会让旧链接作废）。先找绝对 URL，再退回引号里的相对路径——相对路径
    正是红线形态，得能把它抓出来当证据。"""
    for m in reversed(mails()):
        for txt in (m["body"], m["raw"]):
            for u in re.findall(r"https?://[^\s\"'<>)\]]+", txt):
                if needle in u and "token=" in u:
                    return u, u.split("token=", 1)[1]
            for u in re.findall(r'"(/[^\s"]*token=[^"]*)"', txt):
                if needle in u:
                    return u, u.split("token=", 1)[1]
    if os.environ.get("AZ_DEBUG_MAIL"):
        for i, m in enumerate(mails()):
            print("   [mail %d] %s\n   body=%r" % (i, m["head"][:160],
                                                   m["body"][:500]))
    return None, None


def link_note(tag, href, tok):
    """把「用户信箱里到底写着什么」打在日志里（token 只露前 10 位）。"""
    if href:
        print("   [证据] %s 真邮件链接=%stoken=%s…"
              % (tag, href.split("token=")[0], (tok or "")[:10]))
