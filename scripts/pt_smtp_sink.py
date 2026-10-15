# -*- coding: utf-8 -*-
"""闸门共用的 SMTP 诱捕信箱（批三十四起从 AZ 拆出，供需要读「用户真收到的
邮件」的闸门复用）。

为什么要有它：找回密码 / 邮箱激活这类链，接口 200 不代表用户能用——只有落进
信箱的那串字符才算数。所以判据一律取自诱捕容器，不看 API 的返回值。

两个坑（都是踩过才知道的）：
  1. 诱捕容器必须跑进 compose 网络。宿主上开端口对容器是 SYN 黑洞（Docker
     Desktop 防火墙），表现为「发信失败」的假红。
  2. 宿主侧的测试进程解析不了 Docker 网里的容器名（批一百五十三实测：侧进程连
     flux-audit-smtp-sink:2525 一直 timeout，容器内一切正常）。对宿主侧进程
     必须走发布端口。两种地址同时有效：容器内走 SINK_NAME，宿主走
     127.0.0.1；sink_addr() 按 API 实际所在位置选。
  3. 捕获内容走容器 stdout，由 `docker logs` 读回；**不要**写 bind mount 里
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
# 宿主发布端口：容器内部仍是 2525，宿主走这个。
HOST_PORT = int(os.environ.get("AZ_SMTP_HOST_PORT", "2525") or 2525)

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


def _host_port_candidates():
    yield HOST_PORT
    for p in range(HOST_PORT + 1, HOST_PORT + 20):
        yield p


def api_in_container(api_container="flux-api"):
    """问一次：本次验证打的 API 是不是 compose 里那个容器。

    批一百五十三的教训：有的闸门把 API 打到宿主上起的侧进程，它不在 Docker
    网里，解析不了容器名。判法看默认基址的端口是不是主容器的发布端口；另设了
    就必然是侧进程。
    """
    base = os.environ.get("FLUX_API_BASE",
                          "http://127.0.0.1:8080/api/v1")
    try:
        port = int(base.rsplit(":", 1)[1].split("/")[0])
    except Exception:  # noqa: BLE001
        return True
    pub = _run(["docker", "port", api_container]).stdout or ""
    ports = {ln.strip().rsplit(":", 1)[-1]
             for ln in pub.splitlines() if ln.strip()}
    return str(port) in ports


def sink_addr(api_container="flux-api"):
    """API 进程真正连得上的那个地址。"""
    if os.environ.get("AZ_SMTP_HOST"):
        return os.environ["AZ_SMTP_HOST"], HOST_PORT
    if api_in_container(api_container):
        return SINK_NAME, 2525
    return "127.0.0.1", HOST_PORT


def start_sink(api_container="flux-api"):
    """拉起诱捕容器，返回是否成功（失败时调用方应当整体判红，别往下蒙）。

    同时在 compose 网内和宿主上各给一个地址：容器里的 API 用容器名，宿主侧
    的测试进程用发布端口。宿主端口被占用就顺延一个，免得整批闸门假红。
    """
    global _started, HOST_PORT
    os.makedirs(SINK_DIR, exist_ok=True)
    with open(os.path.join(SINK_DIR, "sink.py"), "w", encoding="utf-8") as f:
        f.write(SINK_SRC)
    chk = _run(["docker", "ps", "-a", "--format", "{{.Names}}"])
    if SINK_NAME in (chk.stdout or "").split():
        _run(["docker", "rm", "-f", SINK_NAME])
    net = net_name(api_container)
    r = None
    for port in _host_port_candidates():
        # 两个地址同时有效：容器内走 SINK_NAME，宿主走 127.0.0.1:端口。
        r = _run(["docker", "run", "-d", "--name", SINK_NAME, "--network", net,
                  "-p", "127.0.0.1:%d:2525" % port, "-v",
                  "%s:/sink" % SINK_DIR,
                  "python:3.12-alpine", "python", "/sink/sink.py"])
        if r.returncode == 0:
            HOST_PORT = port
            break
        _run(["docker", "rm", "-f", SINK_NAME])
    _started = r is not None and r.returncode == 0
    print("   [sink] 启动 %s（网=%s 宿主端口=%s）" % (
        "OK" if _started else "FAIL " + ((r.stderr or "") if r else "no try")[:150],
        net, HOST_PORT if _started else "-"))
    time.sleep(3)
    return _started


def stop_sink():
    global _started
    if _started:
        _run(["docker", "rm", "-f", SINK_NAME])
        _started = False


def reachable(api_container="flux-api"):
    """基础设施正控：API 所在位置到诱捕容器的 2525 必须真通，否则后面全是假红。

    测法跟着 API 走：容器内用 docker exec，宿主侧进程用本机 socket——后者
    对容器名是黑洞，硬测只会得到一条与产品无关的红。
    """
    if not _started:
        return False, "sink 未启动"
    host, port = sink_addr(api_container)
    if host == SINK_NAME:
        r = _run(["docker", "exec", api_container, "sh", "-c",
                  "wget -T 3 -O /dev/null http://%s:%d/ 2>&1 | head -1"
                  % (SINK_NAME, port)])
        txt = (r.stdout or "") + (r.stderr or "")
        okc = ("Connection refused" not in txt and "bad address" not in txt)
        return okc, txt[:120]
    import socket as _s
    try:
        sk = _s.create_connection((host, port), timeout=5)
        sk.close()
        return True, "%s:%d 宿主可达" % (host, port)
    except Exception as e:  # noqa: BLE001
        return False, "%s:%d %s" % (host, port, str(e)[:90])


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
                       capture_output=True, text=True, timeout=120, encoding="utf-8", errors="replace")
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
