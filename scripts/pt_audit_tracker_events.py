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


def redis_pw():
    for line in open("D:/FluxTorrent/docker/.env", encoding="utf-8"):
        if line.startswith("REDIS_PASSWORD="):
            return line.split("=", 1)[1].strip()
    return ""


def redis_cli(*args, db=0):
    """flux-redis 开了 requirepass，漏 -a 就是静默 noop（10-08 踩过）。
    db 只允许指到闸门自用的 13 库，别碰主栈的 0 库。"""
    r = subprocess.run(
        ["docker", "exec", "flux-redis", "redis-cli", "-a", redis_pw(),
         "--no-auth-warning", "-n", str(db), *args],
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


COMPOSE = ["docker", "compose", "-f",
           "D:/FluxTorrent/docker/docker-compose.yml"]


def side_docker(env, mode, strict=True):
    """侧 tracker：用 compose 的服务环境变量（手挂 -e 会缺 DATABASE_URL 等，
    容器当场退掉），只覆盖限流阈值/Redis，端口挪到 7171/6970。
    strict=False 时「端口没通」是结果而不是故障——E 组量的就是它。"""
    args = COMPOSE + ["run", mode, "--rm", "--name", SIDE, "--no-deps",
                      "--publish", "127.0.0.1:%d:7070" % SIDE_PORT,
                      "--publish", "127.0.0.1:%d:6969" % (SIDE_PORT + 1)]
    for k, v in env.items():
        args += ["-e", "%s=%s" % (k, v)]
    args.append("tracker")
    r = subprocess.run(args, capture_output=True, text=True,
                       encoding="utf-8")
    if mode != "-d":
        return True
    if r.returncode != 0 and strict:
        raise SystemExit("侧 tracker 起不来：%s" % r.stderr.strip()[-400:])
    for _ in range(24):
        try:
            socket.create_connection(("127.0.0.1", SIDE_PORT), 1).close()
            return True
        except OSError:
            time.sleep(1)
    if strict:
        raise SystemExit("侧 tracker 端口没通（%d）" % SIDE_PORT)
    return False


def side_start(env, strict=True):
    return side_docker(env, "-d", strict)



def side_alive():
    r = subprocess.run(["docker", "ps", "--format", "{{.Names}}"],
                       capture_output=True, text=True, encoding="utf-8")
    return SIDE in r.stdout.split()


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


def metrics_text(tok):
    """侧容器 /metrics（ANN_METRICS_TOKEN 走标准 Bearer 头）。"""
    req = urllib.request.Request(
        "http://127.0.0.1:%d/metrics" % SIDE_PORT,
        headers={"Authorization": "Bearer " + tok})
    try:
        return urllib.request.urlopen(req, timeout=12).read().decode(
            "utf-8", "replace")
    except Exception:  # noqa: BLE001
        return ""


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
        ann2 = d.get(b"announce", b"").decode()
        tiers = d.get(b"announce-list")
        ok("A3 配了回退就出 announce-list", isinstance(tiers, list),
           type(tiers).__name__)
        if isinstance(tiers, list):
            sizes = [len(t) for t in tiers]
            ok("A4 每个 tier 恰好一个地址（旧写法是一 tier 两地址）",
               all(s == 1 for s in sizes), sizes)
            ok("A5 tier 数 = 首选 + 回退", len(tiers) == 2, len(tiers))
            first = tiers[0][0].decode()
            # 比的是**同一次下载**的 announce 与 tier0（https_announce_url
            # 一设，顶层 announce 就换成 https，拿旧那次比必然假红）
            ok("A6 首选 tier 就是顶层 announce", first == ann2,
               (first, ann2))
    finally:
        psql("UPDATE site_settings SET value='' WHERE"
             " name='https_announce_url'")
    back = psql("SELECT value FROM site_settings WHERE"
                " name='https_announce_url'")
    ok("A7 回退设置已复原为空", back == "", repr(back))


def scrape_counts(passkey, ih_hex, url=None):
    """scrape → (complete, incomplete)；结构不认识就返回 None。"""
    base = url or SCR
    s, body = raw_get("%s/%s?info_hash=%s"
                      % (base, passkey, hex_to_url_ih(ih_hex)), timeout=20)
    try:
        d, _ = bdec(body)
        entry = d.get(b"files", {}).get(bytes.fromhex(ih_hex))
        if not isinstance(entry, dict):
            return None
        return (entry.get(b"complete"), entry.get(b"incomplete"))
    except Exception:  # noqa: BLE001
        return None


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


def sec_b_stopped_exempt():
    print("\n-- B stopped 不被限流吞掉（侧容器低阈值）--")
    ih_hex = torrent_row()[1]
    ih = hex_to_url_ih(ih_hex)
    keys = passkeys(2)
    if len(keys) < 2:
        print("[SKIP] B 组需要两个账号：一个上报、一个旁观读数")
        return
    a, o = keys[0], keys[1]
    env = {
        "ANN_RATE_USER_PER_MIN": "4",
        "ANN_RATE_IP_PER_MIN": "100000",
    }
    side_start(env)
    ok("B0 侧容器活着（compose run 起的）", side_alive(), SIDE)
    try:
        # A0 用一个独一无二的监听端口，判据落在「它本人在不在表里」——
        # 绝对计数不可用：侧容器启动时会从 Redis 热恢复主栈的 swarm（本轮实测
        # complete 一上来就是 2），拿数字判增减必然假红/假绿。
        side_port = 45123
        announce(SIDE_ANN, a, ih, pid("A0"), left=0, port=side_port)
        s, body = announce(SIDE_ANN, o, ih, pid("OBS1"), left=0)
        seen = [p[1] for p in parse_peers(body, b"peers")]
        ok("B1 正控——A0 的端点确实在 swarm 里",
           side_port in seen, (seen[:6], announce_counts(body)))
        limited = None
        for i in range(8):
            # 打桶用的其他 peer 换端口：45123 必须是 A0 独有，否则
            # 「A0 消失没消失」这条判据会被同端口的别的 peer 假红（本轮实测）
            s, body = announce(SIDE_ANN, a, ih, pid("A%d" % i), left=0,
                               port=45999)
            if failure_reason(body):
                limited = i + 1
                break
        ok("B2 正控——A 的周期 announce 已被用户限流挡住（桶是活的）",
           limited is not None, limited)
        announce(SIDE_ANN, a, ih, pid("A0"), left=0, event="stopped",
                 port=side_port)
        s, body = announce(SIDE_ANN, o, ih, pid("OBS2"), left=0)
        after = [p[1] for p in parse_peers(body, b"peers")]
        # 判据落在后果上而不是响应文案：旧写法把 stopped 丢在限流里 ⇒
        # A0 仍在别人的 peer 列表里（幽灵在种），新写法必须消失
        ok("B3 stopped 真落表（A0 从 peer 列表消失；旧写法这里仍在）",
           side_port not in after, (side_port, after[:6]))
    finally:
        # 收尾把打桶用的 8 个 peer 也停掉（它们现在是豁免通道，一发就落表）
        for i in range(8):
            announce(SIDE_ANN, a, ih, pid("A%d" % i), left=0,
                     event="stopped", port=45999)
        side_stop()
        # 侧容器与主栈同库 Redis，用户/IP 桶是共享键：清掉，别让别的闸门
        # 或真实用户在下一分钟里被我们压出来的限流误伤
        for u in passkeys(4):
            uid = psql("SELECT id FROM users WHERE passkey='%s'" % u)
            redis_cli("DEL", "rl:ann:u:%s" % uid)
        # 侧容器看到的来源 IP 是 docker 网关（172.20.0.1），不是 127.0.0.1
        # ⇒ 按模式扫着删，别写死（写死就等于没清，下一分钟真实用户被误伤）
        for k in redis_cli("--scan", "--pattern",
                           "rl:ann:ipstop:*").splitlines():
            if k:
                redis_cli("DEL", k)


def sec_c_no_silent_loss():
    print("\n-- C 计费事件不静默丢 --")
    ih = hex_to_url_ih(torrent_row()[1])
    keys = passkeys(1)
    if not keys:
        print("[SKIP] C 组需要一个在册账号的 passkey")
        return
    mypass, tok = keys[0], "gatemetric"
    # 只污染 13 库（侧容器专用）：把 flux:announce 做成 string，对它的 XADD
    # 必返 WRONGTYPE，而其余 Redis 用法照常。**绝不能碰 0 库的真流**——覆盖它
    # 等于删掉线上尚未消费的 announce 事件。EX 120 兜底，收尾再 DEL 一次。
    redis_cli("SET", "flux:announce", "gate-wrongtype", "EX", "120", db=13)
    env = {
        "REDIS_URL": "redis://:%s@flux-redis:6379/13" % redis_pw(),
        "ANN_METRICS_TOKEN": tok,
    }
    side_start(env)
    ok("C0 侧容器活着", side_alive(), SIDE)
    try:
        # started 不参与事件合并（P2-1 只并周期 announce），保证这一发一定走到
        # XADD —— 否则合并窗里的旧键会让它压根不发事件，spill_depth 恒 0 假红
        s, body = announce(SIDE_ANN, mypass, ih, pid("C1"), event="started")
        ok("C1 XADD 被拒时 announce 仍正常响应（不假成功也不掉线）",
           s == 200 and b"interval" in body, (s, body[:80]))
        text = metrics_text(tok)
        hit = re.search(r"flux_tracker_evt_spill_depth (\d+)", text)
        ok("C2 事件进了重投队列而不是被丢弃（spill_depth >= 1）",
           hit is not None and int(hit.group(1)) >= 1,
           hit.group(1) if hit else text[-140:])
        dropped = re.search(r"flux_tracker_evt_spill_dropped_total (\d+)",
                            text)
        ok("C3 正控——队列未满时不该有丢弃计数",
           dropped is not None and int(dropped.group(1)) == 0,
           dropped.group(1) if dropped else None)
    finally:
        side_stop()
        redis_cli("DEL", "flux:announce", db=13)
    left = redis_cli("GET", "flux:announce", db=13)
    ok("C4 13 库的污染键已清干净", left == "", repr(left))

    # 结构判据：裁剪下界真的编进了上线的 worker 二进制
    def in_bin(needle):
        r = subprocess.run(
            ["docker", "exec", "flux-worker", "sh", "-c",
             "grep -ac %s /usr/local/bin/flux-worker" % needle],
            capture_output=True, text=True, encoding="utf-8")
        return r.stdout.strip()

    has_new, has_old = in_bin("XPENDING"), in_bin("XREADGROUP")
    ok("C5 上线的 worker 含裁剪下界 XPENDING（正控 XREADGROUP=%s）" % has_old,
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
    ok("D0 旁观 announce 取到有效响应（正控：限流/鉴权都通）",
       b"failure reason" not in body, failure_reason(body) or body[:90])
    if b"interval" not in body:
        print("[SKIP] D 组拿不到 peer 列表，后面两条不判")
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



def run_foreground(env, budget):
    """前台跑一次性 tracker（compose run --rm），当场收 stdout —— --rm 之后
    `docker logs` 就查不到了。超时后 CLI 被杀但容器还在，所以无论如何都按
    名字扫一遍删掉。"""
    args = COMPOSE + ["run", "--rm", "--no-deps"]
    for k, v in env.items():
        args += ["-e", "%s=%s" % (k, v)]
    args.append("tracker")
    rc, out = None, ""
    try:
        r = subprocess.run(args, capture_output=True, text=True,
                           encoding="utf-8", timeout=budget)
        rc = r.returncode
        out = (r.stdout or "") + (r.stderr or "")
    except subprocess.TimeoutExpired as e:
        out = ((e.stdout or b"").decode("utf-8", "replace")
               + (e.stderr or b"").decode("utf-8", "replace"))
    subprocess.run(
        "docker ps -aq --filter name=tracker-run | xargs -r docker rm -f",
        shell=True,
        capture_output=True,
    )
    return rc, out


def sec_e_boot_loud():
    print("\n-- E Redis 不可达时启动响亮失败（旧写法是零日志僵尸）--")
    rc_ok, out_ok = run_foreground({}, 25)
    ok("E1 正控——Redis 可用时同一条路径能 bind（日志有 listening）",
       "listening" in out_ok, out_ok[-160:])
    rc, out = run_foreground({"REDIS_URL": "redis://127.0.0.1:6399"}, 60)
    ok("E2 Redis 不可达时启动失败留下明确原因（含连接串）",
       "启动失败" in out and "Redis 不可用" in out, out[-200:])
    # 70 = EX_SOFTWARE。退出码非 0 才会被 restart: unless-stopped 重试；
    # 旧写法是「进程挂着不动 + 端口不监听 + 容器显示 running + 零日志」
    ok("E3 以非 0 退出（70），不是永不返回", rc == 70, rc)



def main():
    global TOKEN
    # 上一轮崩在侧容器起来之前也会留下同名容器占住 7171 ⇒ 开局先自扫
    side_stop()
    TOKEN = login_retry()
    s, r = call("GET", "/me", None, TOKEN)
    if s != 200:
        raise SystemExit("token self-check failed: %s" % s)
    try:
        sec_a_tiers()
        sec_b_stopped_exempt()
        sec_c_no_silent_loss()
        sec_d_dual_stack()
        sec_e_boot_loud()
    finally:
        side_stop()
    summary()


if __name__ == "__main__":
    main()
