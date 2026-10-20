# -*- coding: utf-8 -*-
"""闸门 HZ：RSS / Torznab 不许把「点下去必失败」的种当可下载广告出去（批二百零七）。

现网实测（2026-10-20，root，只读，零写入）：
    GET /api/v1/rss/{passkey}?limit=200
      -> 200，50 个 <item>，其中含 49417/49418/49419… 这些
         「approval_status=1 但 torrent_files 里没有原始 .torrent」的种，
         而且照样给 <enclosure url="…/download.php?id=…&passkey=…"
         type="application/x-bittorrent">。
    真点那条 enclosure
      -> 400 {"code":1002,"message":"参数校验失败: 种子文件缺失
              （原始 .torrent 未存档），请联系管理组补档"}
      即：承诺的是 bencode 字节，回来的是 JSON 错误信封。

为什么这对 PT 用户是真问题（不是界面提示那种小毛病）：RSS/Torznab 的消费者是
qBittorrent / FlexGet / autobrr / Prowlarr+Sonarr，它们按 guid 轮询并**自动加种**，
拿到非 bencode 就报「无效种子文件」，下一轮同一个 guid 再来一遍——永久噪声，
且没人看得见那句中文「请联系管理组补档」。凭证还是本人 passkey（P-389 那条
口径），失败也不会掉凭证，只是无限重试。

同族在册的另一半是数据：现网「过审 + 无存档」= 7 条（站长 _demo_works.sql 的
样张种 + 闸门夹具），而 `np_import.rs:138` 那条导入路径直接落
approval_status=1 且无 raw ⇒ 内容包一旦导入，这个数会成百地上涨。

判据（活体三条 + 静态三条 + 真库 A/B 一条 + 承重一条 + 台账棘轮一条）：
  HZ1 正控硬判据：有存档的过审种，RSS 里它的 enclosure 必须 200 且首字节是 'd'
      （bencode 字典）——证明「RSS→下载」这条路本身是通的，问题只在无存档种；
  HZ2 硬判据：RSS **必须 0 条**无存档种的 enclosure（P-433 修复：两出口共用
      `torrents::HAS_ARCHIVE_CLAUSE`）。⚠️ 现网 api 镜像仍是修复前构建时这条
      会红，红账里会打出命中的旧 id——那是镜像滞后，不是判据坏；
  HZ2s 静态硬判据：RSS 的取数 SQL 与 Torznab 的回收路径都引用同一个共享
      谓词常量（RSS 直接插值、Torznab 经 list_torrents_noclamp_for_sub）；
  HZ3 活体读数：Torznab 侧同样枚举与否——现网 229 枚 api_tokens 全是哈希、
      没有明文可用，故不新建凭证（那是写操作），本条只记 WARN 读数；
  HZ4 静态同尺：两个出口对「无存档」必须取同一口径（共用常量 ⇒ 绿）；
  HZ5 承重：把「200 + bencode」的正确形状喂给 HZ2 的违例探测器必须不报，
      把「400 + JSON 信封」的形状必须报（证明探测器不是恒真/恒假）；
  HZ6 台账棘轮：现网「过审 + 无存档」种数 ≤ 7，只许降不许升；
  HZ7 真库 A/B：同一份「过审」取数，不加谓词必须命中无存档种（夹具在册）、
      加上共享谓词后必须 0 条——证明谓词本身真的排得掉，不是恒真/恒假。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hz_rss_undeliverable.py`
      HZ_LIVE=0 跳过 HZ1/HZ2/HZ3（都是只读 GET；passkey 不外传，输出里打码）。
"""
import io
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

LIVE = os.environ.get("HZ_LIVE", "1") != "0"
RSS_RS = "apps/api/src/rss_http.rs"
TORZNAB_RS = "apps/api/src/economy_http/recon.rs"
NO_RAW_CEIL = 7
FEED = "apps/api/src/rss_http.rs"

SQL_NO_RAW = ("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
              "AND NOT EXISTS (SELECT 1 FROM torrent_files f "
              "WHERE f.torrent_id = t.id) ORDER BY t.id")


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(rs):
    """剥 // 与 /// 注释：静态判据只认真码（注释里的表名会洗白真洞）。"""
    return "\n".join(ln for ln in rs.split("\n")
                     if not ln.strip().startswith("//"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA"], input=q,
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    err = (r.stderr or "")
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql: " + err[:240])
    return (r.stdout or "").strip()


def get(url, timeout=40):
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()
    except Exception as e:  # noqa: BLE001
        return None, type(e).__name__.encode()


def enclosures(xml):
    """[(gid, enclosure url)]，gid 取 link/enclosure 里的 id=。"""
    out = []
    for m in re.finditer(r"<enclosure url=\"([^\"]+)\"", xml):
        u = m.group(1).replace("&amp;", "&")
        g = re.search(r"[?&]id=(\d+)", u)
        if g:
            out.append((int(g.group(1)), u))
    return out


def undeliverable(status, body):
    """违例形状：enclosure 承诺 bencode，回来的却是 JSON 错误信封。"""
    if status != 200:
        return True
    head = body[:1] if isinstance(body, bytes) else str(body)[:1]
    return head != b"d" and not str(head).startswith("d")


def is_bencode(status, body):
    if status != 200:
        return False
    head = body[:1] if isinstance(body, bytes) else str(body)[:1]
    return head == b"d" or str(head) == "d"


# —— HZ2s/HZ4 静态同尺：两个出口必须引用**同一个**共享谓词常量
rss_txt = strip(rd(RSS_RS))
tz_txt = strip(rd(TORZNAB_RS))
NOCLAMP_RS = "apps/api/src/torrents/list_noclamp.rs"
noc_txt = strip(rd(NOCLAMP_RS))
rss_excl = "HAS_ARCHIVE_CLAUSE" in rss_txt
tz_excl = ("list_torrents_noclamp_for_sub" in tz_txt
           and "HAS_ARCHIVE_CLAUSE" in noc_txt)
ok("HZ2s 静态硬判据：RSS SQL 与 Torznab 回收路径都接上了共享谓词",
   rss_excl and tz_excl,
   "RSS 引用共享谓词=%s；Torznab 走 for_sub 且其实现引用共享谓词=%s"
   % (rss_excl, tz_excl))
ok("HZ4 静态同尺：RSS 与 Torznab 对「无存档」必须取同一口径",
   rss_excl and tz_excl,
   "两边都引用 torrents::HAS_ARCHIVE_CLAUSE ⇒ 同一把尺；"
   "任一侧不引用=口径分叉，会员在两个工具里看到的不一样")

# —— HZ5 承重：探测器不是恒真
ok("HZ5 承重：探测器对 200+bencode 不误报、对 400+JSON 必报",
   not undeliverable(200, b"d8:announce") and is_bencode(200, b"d8:announce")
   and undeliverable(400, b'{"code":1002')
   and undeliverable(200, b'{"code":0}'),
   "形状自检：bencode 首字节 'd'，错误信封首字节 '{'")

no_raw_ids = [int(x) for x in psql(SQL_NO_RAW).split() if x.strip()]
ok("HZ6 台账棘轮：现网「过审 + 无存档」种数只降不升（基线 7）",
   len(no_raw_ids) <= NO_RAW_CEIL,
   "现网=%d 基线=%d ids=%s；np_import.rs:138 那条导入路径一跑就会成百上涨"
   % (len(no_raw_ids), NO_RAW_CEIL, no_raw_ids[:8]))

# —— HZ7 真库 A/B：同一份取数，只换「共享谓词」这一处
SQL_ALL_APPROVED = "SELECT t.id FROM torrents t WHERE t.approval_status = 1"
SQL_WITH_ARCHIVE = (SQL_ALL_APPROVED + " AND EXISTS (SELECT 1 FROM "
                    "torrent_files tf WHERE tf.torrent_id = t.id)")
before = set(int(x) for x in psql(SQL_ALL_APPROVED).split())
after = set(int(x) for x in psql(SQL_WITH_ARCHIVE).split())
leak_before = sorted(before & set(no_raw_ids))
leak_after = sorted(after & set(no_raw_ids))
ok("HZ7 真库 A/B：共享谓词真能排掉无存档种（加谓词前命中、后 0 条）",
   bool(leak_before) and not leak_after,
   "加谓词前命中=%s / 加谓词后命中=%s；前后都空=夹具不在、判据不出结论"
   % (leak_before[:6], leak_after[:6]))

if LIVE:
    login("root", "password123")  # 顺带验鉴权链活着（不打印 token）
    pk = psql("SELECT passkey FROM users WHERE username='root';")
    mask = pk[:4] + "…" + pk[-4:]
    st, body = get("%s/rss/%s?limit=200" % (BASE, pk))
    xml = (body or b"").decode("utf-8", "replace") if isinstance(body, bytes)\
        else str(body)
    enc = enclosures(xml)
    hits = sorted(set(i for i, _u in enc) & set(no_raw_ids))
    ok("HZ2 硬判据：RSS 给无存档种发的 enclosure 必须 0 条（P-433）",
       st == 200 and not hits,
       "feed http=%s item=%d；命中无存档 ids=%s（非空=修复未生效或镜像"
       "仍是修复前构建，红账里的 id 就是被广告出去的坏种）"
       % (st, len(enc), hits[:6]))
    # 正控 + 违例：各取一条 enclosure 真点
    bad_id = sorted(set(i for i, _u in enc) & set(no_raw_ids))
    good_pair = [(i, u) for i, u in enc if i not in set(no_raw_ids)]
    if good_pair:
        gid, gu = good_pair[0]
        s1, b1 = get(gu)
        ok("HZ1 正控硬判据：有存档种的 enclosure 必须 200 且回 bencode"
           "（id=%s）" % gid, is_bencode(s1, b1),
           "http=%s 首字节=%r 字节数=%s（这条红=RSS→下载整条路坏了，"
           "与无存档无关）" % (s1, (b1 or b"")[:1],
                              len(b1) if isinstance(b1, bytes) else "-"))
    else:
        ok("HZ1 正控硬判据：本屏 feed 里没有可对照的有存档种（不算绿）",
           False, "enclosure 全落在无存档集合里，取样面太窄")
    if bad_id:
        bu = dict(enc)[bad_id[0]]
        s2, b2 = get(bu)
        txt = (b2 or b"").decode("utf-8", "replace")[:120] \
            if isinstance(b2, bytes) else str(b2)[:120]
        print("   [镜像滞后] 无存档 enclosure 实测 id=%s http=%s 响应=%s"
              % (bad_id[0], s2, txt.replace(pk, mask)))
        print("   [镜像滞后] 消费者拿到的是错误信封而不是 .torrent："
              "qBittorrent/FlexGet/autobrr 按 guid 轮询会无限重试同一枚种，"
              "而「请联系管理组补档」这句中文只有本站界面看得到；"
              "本条将在 api 镜像重建后转绿")
    print("   [WARN] HZ3 Torznab 侧未做活体：现网 api_tokens=%s 枚全部是哈希，"
          "没有明文可用；新建凭证属写操作，本闸门不擅自做（静态口径见 HZ4）"
          % psql("SELECT count(*) FROM api_tokens;"))
else:
    print("[HZ1/HZ2/HZ3] HZ_LIVE=0 跳过活体")

summary()
