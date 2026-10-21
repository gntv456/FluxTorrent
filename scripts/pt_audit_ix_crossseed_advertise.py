# -*- coding: utf-8 -*-
"""闸门 IX：辅种（crossseed）出口只许广告「可交付」的种（P-482）。

病形（批二百四十实测）：`/crossseed/lookup` 与 `/crossseed/iyuu` 共用内核
`repo::xseed::find_by_fingerprints`，它原来只判 `approval_status = 1`。
现网实测 7 个「已过审但没有 .torrent 存档」的种能被指纹命中，返回体里带着
`download_url`；客户端真去兑那条链接，`GET /crossseed/dl/{id}` 直接 400
（凭证签发口 `issue_download_key` 判定存档）——链接兑不了现，还按
10 次/分钟吃掉辅种下载额度。

同一个口径在 RSS/Torznab 上已经修过一次（P-433：对外只广告可交付的种，
共用谓词 `torrents::HAS_ARCHIVE_CLAUSE`）。辅种是**第三个对外出口**，
比 RSS 宽就是分叉——所以第一条判据是「内核必须引用那个共用常量」：
自己再抄一份 EXISTS，下次改常量这出口就漏。

判据：
  IX1 内核引用 `crate::torrents::HAS_ARCHIVE_CLAUSE`，且没有自抄的存档子查询；
  IX2 两个外壳（`mod.rs` 的 lookup / `iyuu.rs`）都不许自己写 `FROM torrents`
      取数——两份外壳一份内核；
  IX3 SQL 执行级（现网真库）：带存档条件挡掉「无存档过审种」、放进
      「有存档过审种」；正控＝摘掉条件后无存档种必须回来；
  IX4 活体：探针成员自签一枚 crossseed Token 打真出口，返回的每个 torrent_id
      都必须有存档（旧镜像这条红＝缺陷本体，镜像重建后转绿）；
  IX5 活体对照：匿名 401、未过审种不被广告、非 hex40 拒 1002、
      一次塞 501 条指纹拒 1002；
  IX6 收尾：本轮 Token 撤销、探针成员停用（先撤 Token 再停用——
      `require_token` 会 JOIN `users.status < 2`，停用后凭证即失效）。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_ix_crossseed_advertise.py
"""
import inspect
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (BASE, call, login, ok, psql_stdin, summary)  # noqa

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUN = int(time.time())
MARK = "ixg%d" % RUN
RPW = inspect.signature(login).parameters["pw"].default


def sval(q):
    return psql_stdin(q).strip()


def flat(rel):
    """读 Rust 源码并剥注释：判据说的是「代码」，不是文档里的字。"""
    t = open(os.path.join(ROOTDIR, rel), encoding="utf-8",
             errors="replace").read()
    t = "\n".join(re.sub(r"(?<!:)//.*", "", ln) for ln in t.split("\n"))
    return re.sub(r"/\*.*?\*/", "", t, flags=re.S)


XSEED = flat(os.path.join("apps", "api", "src", "repo", "xseed.rs"))
ok("IX1 辅种内核引用共用存档谓词（=%s），且没有自抄的存档子查询"
   % ("crate::torrents::HAS_ARCHIVE_CLAUSE" in XSEED),
   "crate::torrents::HAS_ARCHIVE_CLAUSE" in XSEED
   and "torrent_files" not in XSEED,
   "自抄一份＝下次改常量这个出口就漏（P-433 已为此修过 RSS/Torznab）")

own_sql = []
for n in ["mod.rs", "iyuu.rs"]:        # 只看两个外壳；单测文件不是出口
    t = flat(os.path.join("apps", "api", "src", "openapi_http",
                          "crossseed", n))
    if re.search(r"SELECT[^;]{0,200}\bFROM\s+torrents\b", t, re.I):
        own_sql.append(n)
ok("IX2 两个外壳里都没有第二条 `FROM torrents` 取数（违例=%s）" % own_sql,
   not own_sql, "出现第二条＝辅种的两个出口会漂成两套口径")

print("== IX3 谓词在现网真库上算得对不对 ==")
no_arch = sval("SELECT string_agg(id::text, ',') FROM (SELECT t.id FROM "
               "torrents t WHERE t.approval_status = 1 AND t.info_hash <> '' "
               "AND NOT EXISTS (SELECT 1 FROM torrent_files tf WHERE "
               "tf.torrent_id = t.id) LIMIT 3) x")
has_arch = sval("SELECT string_agg(id::text, ',') FROM (SELECT t.id FROM "
                "torrents t WHERE t.approval_status = 1 AND t.info_hash <> '' "
                "AND EXISTS (SELECT 1 FROM torrent_files tf WHERE "
                "tf.torrent_id = t.id) ORDER BY t.id DESC LIMIT 3) x")
ids = ",".join([x for x in (no_arch + "," + has_arch).split(",") if x]) or "0"
KEEP = ("SELECT string_agg(t.id::text, ',') FROM torrents t WHERE "
        "t.approval_status = 1 AND EXISTS (SELECT 1 FROM torrent_files tf "
        "WHERE tf.torrent_id = t.id) AND t.id IN (" + ids + ")")
DROP = KEEP.replace("AND EXISTS (SELECT 1 FROM torrent_files tf "
                   "WHERE tf.torrent_id = t.id) ", "")
kept = {x for x in sval(KEEP).split(",") if x}
free = {x for x in sval(DROP).split(",") if x}
na = {x for x in no_arch.split(",") if x}
ha = {x for x in has_arch.split(",") if x}
ok("IX3 带存档条件=%s（无存档种混进 %d 个，须 0）；摘掉条件后放回 %d 个"
   "（正控须 %d）" % (sorted(kept), len(kept & na), len(free & na), len(na)),
   bool(na) and bool(ha) and not (kept & na) and (kept & ha) == ha
   and (free & na) == na,
   "现网无存档过审种取样=%s" % sorted(na))

print("== IX4/IX5 活体：真出口只广告可交付的种 ==")
ROOT = login()
user = "ixu%d" % RUN
st_add, r_add = call("POST", "/admin/adduser",
                     {"username": user, "password": RPW,
                      "email": "%s@ix.local" % user}, token=ROOT)
row = sval("SELECT id FROM users WHERE username = '%s'" % user)
if not row:
    raise RuntimeError("adduser 失败 http=%s resp=%s"
                       % (st_add, str(r_add)[:160]))
uid = int(row)
t0 = login(user)
call("POST", "/me/password/change", {"old_password": RPW,
                                      "new_password": RPW + "x"}, token=t0)
mtok = login(user, RPW + "x")
st_t, r_t = call("POST", "/me/tokens", {"name": MARK,
                                        "scopes": ["crossseed"],
                                        "rate_per_min": 60,
                                        "password": RPW + "x"}, token=mtok)
td = (r_t or {}).get("data") or {}
key, token_id = td.get("token"), td.get("id")
ok("IX0 探针成员自签 crossseed Token 成功（uid=%d token_id=%s）"
   % (uid, token_id), bool(key) and bool(token_id),
   "用探针成员而不是 root：root 名下 Token 上限 3 枚，历史探针占满后所有活体"
   "判据会 cascade 成「未认证」假红（本轮实测踩过）")


def api(method, path, body=None, tok=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("content-type", "application/json")
    if tok:
        req.add_header("authorization", "Token " + tok)
    try:
        with urllib.request.urlopen(req, timeout=25) as x:
            raw = x.read()
            return x.status, (json.loads(raw) if raw[:1] == b"{" else raw)
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, raw


hash_list = [h for h in sval("SELECT string_agg(info_hash, ',') FROM torrents "
                             "WHERE id IN (" + ids + ")").split(",") if h]
s4, b4 = api("POST", "/crossseed/lookup",
             {"info_hash_v1": hash_list, "pieces_sha1": []}, tok=key)
got = {str(r.get("torrent_id")) for r in
       (((b4 or {}).get("data") or {}).get("results") or [])}
s5, b5 = api("POST", "/crossseed/lookup", {"info_hash_v1": hash_list[:1]})
pend_row = (sval("SELECT id || ' ' || info_hash FROM torrents WHERE "
                 "approval_status <> 1 AND info_hash <> '' ORDER BY id "
                 "LIMIT 1").split() + ["", ""])[:2]
pend_id, pend_h = pend_row
s6, b6 = api("POST", "/crossseed/lookup", {"info_hash_v1": [pend_h]}, tok=key)
got6 = {str(r.get("torrent_id")) for r in
        (((b6 or {}).get("data") or {}).get("results") or [])}
s7, b7 = api("POST", "/crossseed/lookup",
             {"info_hash_v1": ["zz" + "0" * 38]}, tok=key)
# 上限要按**去重后**的条数测：`norm_hashes` 先合并同值（代码里明写
# 「去重后合计」），所以塞 501 份同一个 hash 是合法的 1 条——上一版我拿重复
# 数组判上限，红的是判据自己。这里造 501 个互不相同的合法 hex40。
many = ["%040x" % i for i in range(1, 502)]
s8, b8 = api("POST", "/crossseed/lookup",
             {"info_hash_v1": many, "pieces_sha1": []}, tok=key)
ok("IX4 真出口返回的命中里不许有「无存档种」（实返回=%s）" % sorted(got),
   s4 == 200 and bool(key) and not (got & na),
   "旧镜像这条红＝缺陷本体（内核还没引存档谓词）；重建后应只剩 %s"
   % sorted(got & ha))
ok("IX5 匿名 401(%s)、未过审种 %s 不被广告(返回=%s)、非 hex40 拒(%s)、"
   "501 条指纹拒(%s)" % ((b5 or {}).get("code"), pend_id, sorted(got6),
                          (b7 or {}).get("code"), (b8 or {}).get("code")),
   (b5 or {}).get("code") == 2001 and (b7 or {}).get("code") == 1002
   and (b8 or {}).get("code") == 1002 and pend_id not in got6,
   str(b8)[:80])

print("== IX6 收尾 ==")
if token_id:
    call("POST", "/me/tokens/revoke", {"id": int(token_id)}, token=mtok)
left = sval("SELECT count(*) FROM api_tokens WHERE user_id = %d AND "
            "revoked_at IS NULL" % uid)
sval("UPDATE users SET status = 2 WHERE id = %d" % uid)
ok("IX6 探针 Token 已撤销（该成员名下未撤销=%s 须 0）、成员已停用" % left,
   left == "0",
   "先撤 Token 再停用：require_token 会 JOIN users.status<2，停用即凭证失效；"
   "探针成员 id=%s（已停用号，与其它闸门探针一起待清账）" % uid)

summary()
