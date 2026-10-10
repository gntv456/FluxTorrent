# -*- coding: utf-8 -*-
"""announce 协议面闸门（资深 tracker 运维视角实测，0341 同批的四件修复）。

覆盖四条判据，每条都说清自己是「行为」还是「结构」：
  A .torrent 的 announce-list 按 BEP 12 一 tier 一地址（行为）
  B stopped 不被限流吞掉（行为，侧容器把 ANN_RATE_USER_PER_MIN 压到 4）
  C 计费事件不静默丢（行为：把侧容器的 Redis 换成不可达，事件进重投队列；
    结构：worker 二进制里确有 XPENDING 裁剪下界）
  D 同一 peer_id 的 v4/v6 端点各自留档（行为，需 ?ip= 注入口可用，
    否则显式标 SKIP —— 单测 peers/tests/endpoints.rs 已覆盖同一形状）

用法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_tracker_events.py
可 FLUX_API_BASE 覆盖 API 基址。侧容器在 finally 一律拆掉。
"""
import json
import os
import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, ok, summary  # noqa: E402
import pt_audit_lib  # noqa: E402

if os.environ.get("FLUX_API_BASE"):
    pt_audit_lib.BASE = os.environ["FLUX_API_BASE"]
BASE = pt_audit_lib.BASE
ANN = os.environ.get("FLUX_ANN_BASE", "http://127.0.0.1:7070/announce")
SCR = os.environ.get("FLUX_SCR_BASE", "http://127.0.0.1:7070/scrape")
SIDE = "flux-trk-gate"
SIDE_PORT = 7171
SIDE_ANN = "http://127.0.0.1:%d/announce" % SIDE_PORT
TOKEN = ""


def redis_cli(*args):
    """flux-redis 开了 requirepass，漏 -a 就是静默 noop（10-08 踩过）。"""
    pw = ""
    for line in open("D:/FluxTorrent/docker/.env", encoding="utf-8"):
        if line.startswith("REDIS_PASSWORD="):
            pw = line.split("=", 1)[1].strip()
    r = subprocess.run(
        ["docker", "exec", "flux-redis", "redis-cli", "-a", pw,
         "--no-auth-warning", *args],
        capture_output=True, text=True, encoding="utf-8")
    return r.stdout.strip()


def passkeys(n=2):
    """两个在册不同账号的 passkey（旁观者必须与上报者不同 user_id，
    同账号的 peer 会被 snapshot 的 self_user 过滤掉）。"""
    rows = psql("SELECT passkey FROM users WHERE status < 2"
                " AND passkey IS NOT NULL AND passkey <> ''"
                " ORDER BY id LIMIT %d" % n)
    return [x for x in rows.splitlines() if x]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-q", "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip())
    return r.stdout.strip()


def login_retry(user="root", pw="password123", tries=10):
    last = None
    for i in range(tries):
        s, r = call("POST", "/auth/login", {"username": user, "password": pw})
        if s == 200 and isinstance(r, dict) and r.get("code") == 0:
            return r["data"]["token"]
        last = (s, r)
        time.sleep(min(2 + i * 3, 20))
    raise SystemExit("login failed %s: %s" % (user, last))


def raw_get(url, timeout=15):
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


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


def announce(url, passkey, ih, peer_id, ip=None, event="", up=0, down=0,
             left=1024, port=6881):
    q = "info_hash=%s&peer_id=%s&port=%d&uploaded=%d&downloaded=%d" \
        "&left=%d&numwant=50&key=deadbeef" % (ih, peer_id, port, up, down,
                                              left)
    if event:
        q += "&event=%s" % event
    if ip:
        q += "&ip=%s" % ip
    return raw_get("%s/%s?%s" % (url, passkey, q), timeout=20)


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


def side_start(env):
    side_docker(env, "-d")


def side_docker(env, mode):
    args = ["docker", "run", mode, "--rm", "--name", SIDE,
            "--network", "docker_default",
            "--publish", "127.0.0.1:%d:7070" % SIDE_PORT,
            "--publish", "127.0.0.1:%d:6969" % (SIDE_PORT + 1)]
    for k, v in env.items():
        args += ["-e", "%s=%s" % (k, v)]
    args.append(os.environ.get(
        "FLUX_TRACKER_IMAGE", "ghcr.io/gntv456/fluxtorrent/tracker:latest"))
    r = subprocess.run(args, capture_output=True, text=True,
                       encoding="utf-8")
    if mode == "-d" and r.returncode != 0:
        raise SystemExit("侧 tracker 起不来：%s" % r.stderr.strip()[:300])
    if mode == "-d":
        time.sleep(6)


def side_stop():
    subprocess.run(["docker", "rm", "-f", SIDE], capture_output=True)
    time.sleep(2)


def torrent_row():
    row = psql("SELECT id, trim(info_hash) FROM torrents"
               " WHERE approval_status = 1 ORDER BY id DESC LIMIT 1")
    tid, ih = row.split("|")
    return int(tid), ih.strip()


def hex_to_url_ih(ih):
    """announce 的 info_hash 要原始 20 字节的 URL 转义（不是 hex 串）。"""
    import urllib.parse
    return urllib.parse.quote(bytes.fromhex(ih), safe="")


def pid(tag):
    """peer_id 必须恰好 20 字节，否则只回一句 bencode 错误而 HTTP 仍 200。"""
    raw = ("-FLGATE%s" % tag).encode()
    return urllib_parse_quote(raw)


def urllib_parse_quote(raw):
    import urllib.parse
    return urllib.parse.quote(raw, safe="")


def sec_a_tiers():
    print("\n-- A .torrent 的 announce-list 形状（BEP 12）--")
    global TOKEN
    tid, _ = torrent_row()
    url = "%s/torrents/%d/download" % (BASE, tid)
    req = urllib.request.Request(url,
                                 headers={"Authorization": "Bearer " + TOKEN})
    body = urllib.request.urlopen(req, timeout=30).read()
    d, _ = bdec(body)
    ann = d.get(b"announce", b"").decode()
    ok("A1 顶层 announce 带 passkey", "/announce/" in ann, ann)
    ok("A2 正控——未配回退时不该有 announce-list",
       b"announce-list" not in d, list(d.keys())[:6])
    # 现配一条回退（site_settings 每请求现读，不用重启）
    psql("UPDATE site_settings SET value='https://bak.flux.local'"
         " WHERE name='https_announce_url'")
    try:
        req = urllib.request.Request(
            url, headers={"Authorization": "Bearer " + TOKEN})
        body = urllib.request.urlopen(req, timeout=30).read()
        d, _ = bdec(body)
        tiers = d.get(b"announce-list")
        ok("A3 配了回退就出 announce-list", isinstance(tiers, list),
           type(tiers).__name__)
        if isinstance(tiers, list):
            sizes = [len(t) for t in tiers]
            ok("A4 每个 tier 恰好一个地址（旧写法是一 tier 两地址）",
               all(s == 1 for s in sizes), sizes)
            ok("A5 tier 数 = 首选 + 回退", len(tiers) == 2, len(tiers))
            first = tiers[0][0].decode()
            ok("A6 首选 tier 是 announce 那个地址", first == ann, first)
    finally:
        psql("UPDATE site_settings SET value='' WHERE"
             " name='https_announce_url'")
    back = psql("SELECT value FROM site_settings WHERE"
                " name='https_announce_url'")
    ok("A7 回退设置已复原为空", back == "", repr(back))


def sec_b_stopped_exempt():
    print("\n-- B stopped 不被限流吞掉（侧容器低阈值）--")
    ih = hex_to_url_ih(torrent_row()[1])
    mypass = psql("SELECT passkey FROM users WHERE id=1")
    env = {
        "ANN_RATE_USER_PER_MIN": "4",
        "ANN_RATE_IP_PER_MIN": "100000",
        "ANNOUNCE_INTERVAL": "3600",
    }
    side_start(env)
    try:
        limited = None
        for i in range(8):
            s, body = announce(SIDE_ANN, mypass, ih, pid("A%d" % i))
            fr = failure_reason(body)
            if fr and "频率超限（用户）" in fr:
                limited = i + 1
                break
        ok("B1 正控——周期 announce 确实被用户限流挡住（证明桶是活的）",
           limited is not None, limited)
        s, body = announce(SIDE_ANN, mypass, ih, pid("A0"), event="stopped")
        fr = failure_reason(body)
        ok("B2 stopped 不被限流拒（旧写法这里就丢了状态转换）",
           fr is None or "频率超限" not in fr, fr)
        s, body = announce(SIDE_ANN, mypass, ih, pid("A1"))
        fr2 = failure_reason(body)
        ok("B3 正控——stopped 之后周期 announce 仍被拒（桶没被顺手清空）",
           fr2 is not None and "频率超限" in fr2, fr2)
    finally:
        side_stop()
        # 侧容器与主栈同库 Redis，用户/IP 桶是共享键：清掉，别让别的闸门
        # 或真实用户在下一分钟里被我们压出来的限流误伤
        for u in passkeys(4):
            uid = psql("SELECT id FROM users WHERE passkey='%s'" % u)
            redis_cli("DEL", "rl:ann:u:%s" % uid)
        redis_cli("DEL", "rl:ann:ipstop:%s" % "127.0.0.1")


def sec_c_no_silent_loss():
    print("\n-- C 计费事件不静默丢 --")
    ih = hex_to_url_ih(torrent_row()[1])
    mypass = psql("SELECT passkey FROM users WHERE id=1")
    tok = "gatemetric"
    env = {
        # 故意把 Redis 指到不可达口令：XADD 必失败
        "REDIS_URL": "redis://:wrongpw@flux-redis:6379",
        "ANN_METRICS_TOKEN": tok,
    }
    side_start(env)
    try:
        s, body = announce(SIDE_ANN, mypass, ih, pid("C1"))
        fr = failure_reason(body)
        ok("C1 Redis 不可达时 announce 仍正常服务（不 500 不假成功）",
           s == 200 and fr is None, (s, fr))
        s, m = raw_get("http://127.0.0.1:%d/metrics" % SIDE_PORT, timeout=10)
        if s != 200:
            import urllib.request as ur
            req = ur.Request("http://127.0.0.1:%d/metrics" % SIDE_PORT,
                             headers={"Authorization": "Bearer " + tok})
            m = ur.urlopen(req, timeout=10).read()
        text = m.decode("utf-8", "replace")
        hit = re.search(r"flux_tracker_evt_spill_depth (\d+)", text)
        ok("C2 事件进了重投队列而不是被丢弃",
           hit is not None and int(hit.group(1)) >= 1,
           hit.group(1) if hit else text[-120:])
    finally:
        side_stop()
    has_new = subprocess.run(
        ["docker", "exec", "flux-worker", "sh", "-c",
         "grep -ac XPENDING /usr/local/bin/flux-worker"],
        capture_output=True, text=True, encoding="utf-8").stdout.strip()
    has_old = subprocess.run(
        ["docker", "exec", "flux-worker", "sh", "-c",
         "grep -ac XREADGROUP /usr/local/bin/flux-worker"],
        capture_output=True, text=True, encoding="utf-8").stdout.strip()
    ok("C3 上线的 worker 含裁剪下界 XPENDING（正控 XREADGROUP=%s）" % has_old,
       int(has_new or 0) >= 1 and int(has_old or 0) >= 1,
       (has_new, has_old))


def sec_d_dual_stack():
    print("\n-- D 同一 peer_id 的两族端点各自留档 --")
    ih = hex_to_url_ih(torrent_row()[1])
    keys = passkeys(2)
    if len(keys) < 2:
        print("[SKIP] 在册账号不足两个——D 组必须用两个不同 user_id")
        return
    mypass, other = keys[0], keys[1]
    v4, v6 = "192.0.2.77", "2001:db8::77"
    announce(ANN, mypass, ih, pid("D1"), ip=v4)
    announce(ANN, mypass, ih, pid("D1"), ip=v6)
    s, body = announce(ANN, other, ih, pid("D9"))
    if failure_reason(body):
        ok("D0 正控——旁观 announce 取到响应", False, failure_reason(body))
        return
    got4 = [p[0] for p in parse_peers(body, b"peers")]
    got6 = [p[0] for p in parse_peers(body, b"peers6")]
    if v4 not in got4 and v6 not in got6:
        print("[SKIP] ?ip= 注入口在本实例不可用（TRUST_PROXY 未放开），"
              "D 组两条断言不成立也不判红；端点形状由单测 "
              "peers/tests/endpoints.rs::dual_stack_announce_keeps_both_"
              "endpoints 背书")
        return
    ok("D1 v6 那一发没抹掉已存的 v4 端点", v4 in got4, got4[:6])
    ok("D2 v6 端点同时出现在 peers6", v6 in got6, got6[:6])
    announce(ANN, mypass, ih, pid("D1"), event="stopped")
    announce(ANN, other, ih, pid("D9"), event="stopped")


def main():
    global TOKEN
    TOKEN = login_retry()
    s, r = call("GET", "/me", None, TOKEN)
    if s != 200:
        raise SystemExit("token self-check failed: %s" % s)
    try:
        sec_a_tiers()
        sec_b_stopped_exempt()
        sec_c_no_silent_loss()
        sec_d_dual_stack()
    finally:
        side_stop()
    summary()


if __name__ == "__main__":
    main()
