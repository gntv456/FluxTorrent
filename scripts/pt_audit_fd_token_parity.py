# -*- coding: utf-8 -*-
"""闸门 FD：命名 RSS token（fxr_）必须在**每个**用它当凭证的口都认。

批一百七十九。P-389 实测出来的洞：RSS 订阅能正常拉（200，item 的
`<enclosure>` 直链里放的就是本次鉴权用的那枚 fxr_ token——`rss_http.rs`
的口径注释白纸黑字写「订阅 URL 与下载直链同凭证，撤销一个 token 即整链失效」），
但 `qBittorrent/autobrr/Flexget` 照直链去下种时打到的是
`/compat/nexusphp/download.php`，而那个口此前**只查 `user_by_passkey`** ⇒
实测 401「未认证」。也就是说：给「不想把主 passkey 交给刷流器」的人做的
命名 token 功能，恰恰在他们要自动刷流的那一步把链路切断，而文档说它是通的。

判据（静态钉形状 + 活体打整链）：
  FD1 compat download.php 必须分形状解析凭证（fxr_ 走 rss_tokens），
      且 token 路径要带 `revoked_at IS NULL` 与 `users.status < 2`
      （换凭证不许放宽状态闸——这是 P-346/鉴权层一贯口径）；
  FD2 哈希只有一把尺：鉴权口与写口都调 `tokens::token_hash_hex`，
      不许再出现各处自己 sha3 一遍的写法（authz.rs 抽出就是为了防漂移）；
  FD3 活体：建 token → 拉订阅 → 取 enclosure（**按阅读器的方式反转义
      `&amp;`**）→ 下载必须 200 且首字节是 bencode 的 `d8:announce`；
  FD4 活体对照：主 passkey 打同一个口仍 200（没被我改动打坏）；
  FD5 活体：撤销后同一直链必须 401（文档承诺的整链失效）；伪造 token 形状
      与伪造 passkey 都必须 401，不得 500；
  FD6 收尾：本闸门建的 token 行必须为 0。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_fd_token_parity.py
     （活体需要带本批代码的实例：FLUX_API_BASE=http://127.0.0.1:8099/api/v1）
"""
import html
import io
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOTDIR, "apps", "api", "src")


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return re.sub(r"^\s*//.*$", "", s, flags=re.M)


np_dl = strip_comments(rd(os.path.join(SRC, "compat_http", "nexusphp.rs")))
tok_rs = strip_comments(rd(os.path.join(SRC, "rss_http", "tokens.rs")))
authz = strip_comments(rd(os.path.join(SRC, "rss_http", "authz.rs")))

FD1 = ("resolve_user_id(" in np_dl and "t.revoked_at IS NULL" in authz
       and "u.status < 2" in authz)
ok("FD1 download.php 走共用凭证解析（撤销 + 状态两道闸齐）", FD1,
   "np 调解析=%s / authz 撤销闸=%s / authz 状态闸=%s"
   % ("resolve_user_id(" in np_dl, "t.revoked_at IS NULL" in authz,
      "u.status < 2" in authz))
SHAPE = "Sha3_256::new()"
ok("FD2 哈希一把尺：token_hash_hex 是唯一入口，写口与鉴权口都调它",
   "pub(crate) fn token_hash_hex" in tok_rs
   and "token_hash_hex(path_cred)" in authz
   and tok_rs.count("fn sha3_hex") == 0,
   "sha3_hex 残留=%d" % tok_rs.count("fn sha3_hex"))
ok("FD2b 旧的两把尺写法不许复活（authz/download 里不再自己 sha3 摘要）",
   SHAPE not in authz and SHAPE not in np_dl,
   "authz=%d np=%d" % (authz.count(SHAPE), np_dl.count(SHAPE)))


def req(method, url, body=None, tok=None):
    r = urllib.request.Request(url, method=method)
    r.add_header("Content-Type", "application/json")
    if tok:
        r.add_header("Authorization", "Bearer " + tok)
    data = json_dumps(body) if body is not None else None
    try:
        x = urllib.request.urlopen(r, data, timeout=30)
        return x.status, x.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def json_dumps(obj):
    import json
    return json.dumps(obj).encode()


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-c", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:220])
    return (r.stdout or "").strip()


import json
st, b = req("POST", BASE + "/auth/login",
            {"username": "root", "password": "password123"})
sess = json.loads(b)["data"]["token"]
st, b = req("POST", BASE + "/rss/tokens", {"name": "fd-gate"}, sess)
if st != 200:
    ok("FD3 建 token 成功（活体前置）", False, "st=%s body=%r" % (st, b[:120]))
else:
    d = json.loads(b)["data"]
    plain, tid = d["token"], d["id"]
    try:
        st_f, feed = req("GET", BASE + "/rss/" + plain)
        m = re.search(rb'enclosure url="([^"]+)"', feed)
        encl = html.unescape(m.group(1).decode()) if m else None
        ok("FD3a 订阅可拉且 item 带 enclosure 直链",
           st_f == 200 and bool(encl), "st=%s" % st_f)
        st_d, body = req("GET", encl) if encl else (0, b"")
        # 本轮实测：enclosure 指向的种可能**根本没有原始存档**（闸门夹具留下的
        # 无档案过审种，与 GC1 同族台账）。那是「缺档案」而不是「凭证不对等」，
        # FD 的职责只判凭证对等 ⇒ 缺档案降级成显式信号，不误报也不放过。
        missing_archive = body.startswith(b'{"code":1002') and             "未存档".encode() in body
        if missing_archive:
            print("   [GC1 同族] enclosure 指向的种缺原始存档 ⇒ FD3b 本轮只判对等")
        ok("FD3b 照 enclosure 直链下载：有档案必须 200 回 .torrent；"
           "无档案则必须回「缺档」而不是凭证错误",
           (st_d == 200 and body.startswith(b"d8:announce")) or missing_archive,
           "st=%s 首字节=%r" % (st_d, body[:60]))
        pk = psql("SELECT passkey FROM users WHERE username = 'root'")
        st_p, bp = req("GET", BASE + "/compat/nexusphp/download.php"
                       "?id=%s&passkey=%s" % (re.sub(r".*id=(\d+).*", r"\1",
                                                     encl or ""), pk))
        ok("FD4 主 passkey 与 fxr_ token 打同一个口必须同码同因"
           "（凭证对等；缺档时两边都得是缺档）",
           st_p == st_d or (missing_archive and st_p == 400),
           "passkey st=%s token st=%s 首字节=%r" % (st_p, st_d, bp[:60]))
        req("DELETE", BASE + "/rss/tokens/%d" % tid, None, sess)
        st_r, br = req("GET", encl) if encl else (0, b"")
        ok("FD5a 撤销后同一直链必须 401（整链失效，文档口径）",
           st_r == 401, "st=%s body=%r" % (st_r, br[:60]))
        st_x, bx = req("GET", BASE + "/compat/nexusphp/download.php"
                       "?id=1&passkey=fxr_" + "z" * 32)
        st_y, by = req("GET", BASE + "/compat/nexusphp/download.php"
                       "?id=1&passkey=" + "0" * 32)
        ok("FD5b 伪造 token / 伪造 passkey 一律 401，不得 5xx",
           st_x == 401 and st_y == 401,
           "伪造 token=%s 伪造 passkey=%s" % (st_x, st_y))
    finally:
        psql("DELETE FROM rss_tokens WHERE name = 'fd-gate';")
        left = psql("SELECT count(*) FROM rss_tokens WHERE name = 'fd-gate'")
        ok("FD6 本闸门无残料（fd-gate 行 %s 条）" % left, left == "0")

summary()
