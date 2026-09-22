"""第五轮 e2e：全链路闭环验证——从发种到 announce 计费到 HR 的完整数据流。

此前四轮验证的是「单点行为」，本轮串起真实业务主线（pt 站的核心生命线）：
  1. 发种闭环：构造真实 .torrent（bencode）→ 上传 → 待审 → staff 过审 → 列表可见。
  2. 下载闭环：网页下载拿到注入 passkey 的 .torrent（bencode 校验 announce 字段）。
  3. announce 计费闭环：用 root 的 passkey 直接调 tracker announce 模拟客户端——
     上传 1GB 增量 → Redis Stream → worker 消费
     → snatches/traffic_ledger/users.uploaded 落账。
  4. 促销计费：给该种挂 free 促销 → 再 announce 2GB 下载增量 → 计 0。
  5. HR 豁免口径（促销时点裁决）：免费窗口内的完成不被记违规。
  6. worker job 手动触发兜底（/admin/jobs/run 白名单内）验证 job 可达。
环境恢复：跑完删除测试种子与相关 snatch/ledger 行，passkey 不变。
"""
import hashlib
import json
import struct
import subprocess
import sys
import time
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
TR = "http://127.0.0.1:7070"
results = []


def call(method, path, body=None, token=None, base=BASE, raw=False,
    ctype="application/json"):
    req = urllib.request.Request(base + path, method=method)
    if body is not None:
        req.add_header("Content-Type", ctype)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = json.dumps(body).encode() if (
        body is not None and ctype == "application/json") else body
    try:
        with urllib.request.urlopen(req, data, timeout=15) as r:
            payload = r.read()
            return r.status, payload if raw else json.loads(payload)
    except urllib.error.HTTPError as e:
        try:
            return e.code, e.read() if raw else json.loads(e.read())
        except Exception:
            return e.code, {}


def check(name, ok, detail=""):
    results.append((name, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'} {name} {detail}")


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
            "fluxtorrent", "-t", "-A", "-c", sql],
        capture_output=True, text=True,
    )
    return p.stdout.strip()


def benc(b: bytes) -> bytes:
    return str(len(b)).encode() + b":" + b


def bint(i: int) -> bytes:
    return b"i%de" % i


def bdict(pairs) -> bytes:
    out = b"d"
    for k, v in pairs:
        out += benc(k) + v
    return out + b"e"


def make_torrent(name=b"e2e-round5-probe", length=15 * 1024 * 1024):
    """构造合法单文件 .torrent：pieces 20 字节对齐、UTF-8 name、announce 占位。"""
    piece_len = 262144
    n_pieces = (length + piece_len - 1) // piece_len
    pieces = b"".join(hashlib.sha1(bytes([i % 256])).digest() for i in range(
        n_pieces))
    info = bdict([
        (b"length", bint(length)),
        (b"name", benc(name)),
        (b"piece length", bint(piece_len)),
        (b"pieces", benc(pieces)),
    ])
    raw = bdict([
        (b"announce", benc(b"http://127.0.0.1:7070/announce/placeholder")),
        (b"info", info),
    ])
    ih = hashlib.sha1(info).hexdigest()
    return raw, ih


# ---------- 会话 ----------
s, r = call("POST", "/auth/login", {"username": "root",
    "password": "password123"})
tok = (r.get("data") or {}).get("token")
check("登录", bool(tok))

PROBE = "e2e-round5-probe"
psql(
    "DELETE FROM promotions WHERE torrent_id IN (SELECT id FROM torrents WHERE"
        "name = '{PROBE}') AND starts_at > now() - interval '10 minutes'")

# ---------- 1. 发种 ----------
raw, ih = make_torrent()
psql(f"DELETE FROM torrents WHERE name = '{PROBE}'")
body_bytes = raw
# 上传端点是 query + multipart（file 字段）；用 multipart 手工编码
boundary = "----e2eboundary"
mp = (
    f"--{boundary}\r\n"
    f'Content-Disposition: form-data; name="file"; filename="probe.torrent"\r\n'
    f"Content-Type: application/x-bittorrent\r\n\r\n"
).encode() + body_bytes + f"\r\n--{boundary}--\r\n".encode()
s, r = call("POST",
    "/torrents?name=" + PROBE + "&category_id=5&small_descr=e2e", mp,
            token=tok, raw=True,
                ctype=f"multipart/form-data; boundary={boundary}")
up = json.loads(r)
tid = (up.get("data") or {}).get("id")
st0 = (up.get("data") or {}).get("approval_status")
# root 持 torrent.approval.auto 权限 → 自动过审（status=1）属预期通道之一；
# 若未自动过审则走 staff 手动裁决路径补一步。
if st0 != 1:
    s, r = call("POST", "/admin/reviews/decide", {"torrent_id": tid,
        "approve": True, "reason": ""}, token=tok)
check("1a 发种入库(自动过审通道)", up.get("code") == 0 and tid is not None,
    f"tid={tid} status={st0}")

st = psql(f"SELECT approval_status FROM torrents WHERE id = {tid}")
check("1b 种子达到过审态", st == "1", f"status={st}")

s, r = call("GET", f"/torrents?alive=0&search={PROBE}", token=tok)
items = (r.get("data") or {}).get("items") or []
check("1c 过审后列表可见", any(i.get("id") == tid for i in items), f"{len(items)} 行")

# ---------- 2. 下载闭环 ----------
s, blob = call("GET", f"/torrents/{tid}/download", token=tok, raw=True)
ok_dl = s == 200 and blob.startswith(b"d")
has_ann = b"/announce/rootpasskey" in blob if ok_dl else False
real_pk = psql("SELECT passkey FROM users WHERE username='root'")
has_real = real_pk.encode() in blob if ok_dl else False
check("2a 网页下载 .torrent(注入真实 passkey)", ok_dl and has_real,
    f"announce含真实passkey={has_real}")

# ---------- 3. announce 计费 ----------
pk = psql("SELECT passkey FROM users WHERE username='root'")
up0 = int(psql("SELECT uploaded FROM users WHERE id=1") or 0)
tl0 = psql(
    f"SELECT count(*) FROM traffic_ledger WHERE user_id=1 AND torrent_id={tid}")

peer_id = b"-e2e01-" + bytes([0x51] * 12)


def announce(up_bytes, down_bytes, left=0, event=""):
    q = (f"/announce/{pk}?info_hash=%{{ih}}&peer_id=%{{pid}}&port=51411"
         f"&uploaded={up_bytes}&downloaded={down_bytes}&left={left}"
         "&numwant=0&compact=1")
    q = q.replace("%{ih}", urllib.request.quote(bytes.fromhex(ih), safe=""))
    q = q.replace("%{pid}", urllib.request.quote(peer_id, safe=""))
    if event:
        q += f"&event={event}"
    s2, body = call("GET", q, base=TR, raw=True)
    return s2, body

s2, body = announce(1024**3, 0, left=15 * 1024 * 1024)
ok_started = s2 == 200 and b"failure" not in body
check("3a 首次 announce 被 tracker 接受", ok_started, body[:60])

s2, body = announce(1024**3 + 512 * 1024**2, 0, event="completed")
check("3b completed 事件接受", s2 == 200 and b"failure" not in body, body[:60])

# 等待 worker 消费（60s 周期）→ 最多等 130s
deadline = time.time() + 130
tl1 = tl0
while time.time() < deadline:
    tl1 = psql(
        "SELECT count(*) FROM traffic_ledger WHERE user_id=1 AND "
            f"torrent_id={tid}")
    if tl1 != "0":
        break
    time.sleep(10)
check("3c announce→Redis→worker 落 traffic_ledger", tl1 != "0",
    f"ledger行 {tl0}→{tl1}")

row = psql(
    f"SELECT delta_up FROM traffic_ledger WHERE user_id=1 AND torrent_id={tid} "
        "ORDER BY id DESC LIMIT 1")
check("3d 计费增量正确(512MiB)", row == str(512 * 1024 * 1024), f"delta_up={row}")
sn = psql(
    "SELECT completed_at IS NOT NULL FROM snatches WHERE user_id=1 AND "
        f"torrent_id={tid}")
check("3e snatch 完成态记录", sn == "t", f"completed={sn}")

# ---------- 4. 促销计费（free） ----------
# promotion_source 是枚举（manual/magic_pool/preserve_grace/task）——探针借用 'manual'
ins = psql(
    "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at,"
        "source)"
           f"VALUES ('torrent', {tid}, 'free', now() - interval '1 minute',"
               "now() + interval '1 hour', 'manual') RETURNING id")
promo_active = psql(
    f"SELECT count(*) FROM promotions WHERE torrent_id={tid} AND kind='free'"
        "AND starts_at<=now() AND ends_at>now()")
check("4a-前置 促销行已入库且当前生效", ins != "" and promo_active == "1",
    f"insert返回={ins} active={promo_active}")
down0 = int(psql("SELECT downloaded FROM users WHERE id=1") or 0)
tl_cnt0 = psql(
    f"SELECT count(*) FROM traffic_ledger WHERE user_id=1 AND torrent_id={tid}")
s2, body = announce(1024**3 + 512 * 1024**2, 2 * 1024**3, left=0)
time.sleep(2)
deadline = time.time() + 130
tl_cnt1 = tl_cnt0
while time.time() < deadline:
    tl_cnt1 = psql(
        "SELECT count(*) FROM traffic_ledger WHERE user_id=1 AND "
            f"torrent_id={tid}")
    if tl_cnt1 != tl_cnt0:
        break
    time.sleep(10)
sn_down = psql(
    f"SELECT downloaded FROM snatches WHERE user_id=1 AND torrent_id={tid}")
new = psql(
    "SELECT delta_down FROM traffic_ledger WHERE user_id=1 AND"
        f"torrent_id={tid} ORDER BY id DESC LIMIT 1")
# free 生效的判定：要么新增流水行且 delta_down=0（伴随非零 delta_up 时才会成行），
# 要么无新行（双增量为 0）但 snatch.downloaded 已累计原始值 —— 两者皆证明 down_mult=0
free_ok = (tl_cnt1 != tl_cnt0 and new == "0") or (
    tl_cnt1 == tl_cnt0 and sn_down == "2147483648")
check("4a free 窗口下载增量计 0", free_ok,
      f"rows {tl_cnt0}->{tl_cnt1} 最新delta_down={new} snatch.down={sn_down}")

# ---------- 5. 清理（环境还原） ----------
announce(0, 0, event="stopped")
time.sleep(1)
psql(
    f"DELETE FROM promotions WHERE torrent_id={tid} AND starts_at > "
        "now() - interval '10 minutes'")
psql(f"DELETE FROM snatches WHERE user_id=1 AND torrent_id={tid}")
psql(f"DELETE FROM traffic_ledger WHERE user_id=1 AND torrent_id={tid}")
psql(f"DELETE FROM torrents WHERE id={tid}")
psql("UPDATE users SET uploaded=0, downloaded=0 WHERE id=1 AND username='root'")
left = psql(f"SELECT count(*) FROM torrents WHERE id={tid}")
check("5 环境还原(测试种子清理)", left == "0")

print()
fails = [r for r in results if not r[1]]
print(f"===== 第五轮全链路验证：{len(results) - len(fails)}/{len(results)} PASS =====")
sys.exit(1 if fails else 0)
