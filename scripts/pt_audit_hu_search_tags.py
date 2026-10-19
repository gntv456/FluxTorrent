# -*- coding: utf-8 -*-
"""闸门 HU：种子搜索召回（中/英）与标签链的常驻判据（批一百九十四）。

为什么建这条：现网 `tags` 全表 0 行、`seedbox_claims` 也 0 行，我按「0 行的功能
最藏洞」去跑，实测下来两条链都是**好的**，但过程里踩了三个自己造的读数坑，
正好都是这条判据要防的：
  1. `pg_stat_user_tables.n_live_tup` 在**没 analyze 过的表上恒为 0**
     （本库 `last_analyze` 全空），我据此列出「235 张空表」几乎全假
     ——真数 attendance=9、announcements=4、posts=5、achievement_defs=30。
     ⇒ 判「表是不是空」必须 `count(*)`，不许读统计视图。
  2. 列表口的搜索参数是 **`search`**（配 `search_area`/`search_mode`），
     我先按 `q=` 打，9 个词全 MISS 且命中数一模一样 ⇒ 差点报「搜索完全失效」。
  3. 期望值要拿真名字：种号 40 叫 `e2e-regression-seed`（ASCII），
     我按中文「回归」去期望它命中 ⇒ 假 MISS。

所以本闸门把「召回」与「不召回」**两头**都钉住（只有正例的判据会把「返回全集」
也读成绿）：
  HU1 中文子串命中（`深测` → 含 Audit.Pack.深测 那一行）；
  HU2 ASCII 子串命中（`FluxTorrent` → 含对接自测种子那一行）；
  HU3 反例：一个库里确实不存在的词必须 0 命中（防「搜索被忽略、永远回全集」）；
  HU4 参数真生效：同一页大小下 `search` 命中数必须 < 不带 `search` 的命中数
      （如果搜索被忽略，两者相等 ⇒ 红）；
  HU5 标签链三段：打标签→列表按 tag_id 命中→取消后不命中（夹具自清理，
      收尾断言 `tags` 回到原行数）；
  HU6 存活三态是**产品口径**不是洞：默认列表 0 条时 `hidden_dead>0` 且
      `include_dead=1` 能拿到内容（这条只钉「口径没被改丢」，别当空列表报警）；
  HU7 正控：本判据的取数函数在搜索被拿掉时必须能报 MISS（用不存在的参数名 q= 试，
      应观察到「命中数等于不带过滤」的形状）。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hu_search_tags.py
      HU_LIVE=0 跳过全部现网与库侧只读操作。
"""
import os
import subprocess
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from pt_audit_lib import call, dig, login, ok, summary  # noqa: E402

LIVE = os.environ.get("HU_LIVE", "1") != "0"
# 夹具标签用 tag_dict.id=3（官种）；作用在 root 名下已过审的种上，跑完撤掉。
TAG_ID = 3
FAKE_TERM = "zz-不存在的关键词-qq"


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA", "-c", q],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


def main():
    if not LIVE:
        print("[HU] HU_LIVE=0 跳过全部活体条目")
        summary()
        return
    tok = login()
    # 现网词库：中英各挑一条**真存在**的名字片段。
    # 不能按 id DESC 随便取一条——若那条恰好全 ASCII，中文判据会静默不跑
    # （条件断言不显式点名＝我最恨的假绿）。
    row_cjk = psql("SELECT id || '|' || name FROM torrents "
                   "WHERE approval_status = 1 AND name ~ '[\\u4e00-\\u9fff]' "
                   "ORDER BY id DESC LIMIT 1")
    row_asc = psql("SELECT id FROM torrents WHERE approval_status = 1 "
                   "AND name ~ '[A-Za-z]{4}' ORDER BY id DESC LIMIT 1")
    tid = ""
    cjk = ""
    if row_cjk:
        tid, nm = row_cjk.split("|", 1)
        buf = ""
        for ch in nm:
            if ord(ch) > 0x2E80:
                buf += ch
                if len(buf) == 2:
                    break
        cjk = buf
    ascii_term = ""
    tid_asc = ""
    if row_asc:
        nm2 = psql("SELECT name FROM torrents WHERE id = " + row_asc)
        for w in nm2.replace("(", " ").replace(")", " ").split():
            if w.isascii() and len(w) > 3:
                ascii_term = w
                tid = tid or row_asc
                tid_asc = row_asc
                break
    ok("HU0 中英两条夹具都取得到（缺一边就点名）",
       bool(cjk) and bool(ascii_term),
       "中文=%r ASCII=%r" % (cjk, ascii_term))

    def search(term):
        path = ("/torrents?include_dead=1&limit=100&search="
                + urllib.parse.quote(term))
        st, b = call("GET", path, None, tok)
        items = dig(b, "data", "items") or []
        return st, b.get("code"), [i.get("id") for i in items]

    st, b = call("GET", "/torrents?include_dead=1&limit=100", None, tok)
    base_ids = [i.get("id") for i in (dig(b, "data", "items") or [])]
    ok("HU4a 列表口本身有内容（否则下面全是假绿）", len(base_ids) > 0,
       "不带过滤=%s 条" % len(base_ids))

    if len(cjk) >= 2:
        st, code, got = search(cjk)
        ok("HU1 中文子串 '%s' 命中该种" % cjk, tid and int(tid) in got,
           "http=%s code=%s 命中=%s 条" % (st, code, len(got)))
    if ascii_term:
        st, code, got = search(ascii_term)
        ok("HU2 ASCII 子串 '%s' 命中该种" % ascii_term,
           bool(tid_asc) and int(tid_asc) in got,
           "http=%s code=%s 命中=%s 条" % (st, code, len(got)))

    st, code, got = search(FAKE_TERM)
    ok("HU3 不存在的词必须 0 命中（防「搜索被忽略」冒充绿）", got == [],
       "命中=%s 条" % len(got))

    st, code, got = search(cjk or ascii_term)
    ok("HU4b 搜索确实缩小结果集（命中数 < 不带过滤）",
       len(got) < len(base_ids) or len(base_ids) >= 100,
       "过滤后=%s 不带=%s" % (len(got), len(base_ids)))

    # ---- HU5 标签链：打上→筛出→撤掉，夹具自清理 ----
    before = int(psql("SELECT count(*) FROM tags"))
    st, b = call("PUT", "/torrents/%s/tags" % tid,
                 {"tag_id": TAG_ID, "on": True}, tok)
    ok("HU5a 打标签口回 200", st == 200 and b.get("code") == 0,
       "http=%s msg=%s" % (st, (b.get("message") or "")[:20]))
    st, b = call("GET", "/torrents?include_dead=1&limit=100&tag_id=%s"
                 % TAG_ID, None, tok)
    ids = [i.get("id") for i in (dig(b, "data", "items") or [])]
    ok("HU5b 按 tag_id 列表能命中该种", bool(tid) and int(tid) in ids,
       "http=%s 命中=%s 条" % (st, len(ids)))
    call("PUT", "/torrents/%s/tags" % tid, {"tag_id": TAG_ID, "on": False}, tok)
    after = int(psql("SELECT count(*) FROM tags"))
    ok("HU5c 收尾：tags 行数回到起点", after == before,
       "前=%s 后=%s" % (before, after))

    # ---- HU6 存活三态（产品口径，不报警只钉「没被改丢」） ----
    st, b = call("GET", "/torrents?limit=5", None, tok)
    d = b.get("data") or {}
    hid = d.get("hidden_dead")
    st2, b2 = call("GET", "/torrents?limit=5&include_dead=1", None, tok)
    ok("HU6 默认隐藏存活外条目时 include_dead 能取回内容",
       (len(d.get("items") or []) > 0) or (int(hid or 0) > 0 and len(
           ((b2.get("data") or {}).get("items") or [])) > 0),
       "默认=%s 条 hidden_dead=%s include_dead=%s 条" % (
           len(d.get("items") or []), hid,
           len(((b2.get("data") or {}).get("items") or []))))

    # ---- HU7 正控：错误的参数名会让搜索被忽略 ⇒ 判据必须能看出这个形状 ----
    st, b = call("GET", "/torrents?include_dead=1&limit=100&q="
                 + urllib.parse.quote(FAKE_TERM), None, tok)
    n = len(dig(b, "data", "items") or [])
    ok("HU7 正控：未知参数被忽略时结果数等于不带过滤（证明 HU3/HU4 承重）",
       n == len(base_ids), "q= 命中=%s 不带过滤=%s" % (n, len(base_ids)))
    summary()


if __name__ == "__main__":
    main()
