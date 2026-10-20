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

判据（活体三条 + 静态两条 + 承重一条 + 台账棘轮一条）：
  HZ1 正控硬判据：有存档的过审种，RSS 里它的 enclosure 必须 200 且首字节是 'd'
      （bencode 字典）——证明「RSS→下载」这条路本身是通的，问题只在无存档种；
  HZ2 在册：RSS 仍为无存档种发 enclosure（按 present 断言；排除掉即转红，
      届时翻成「必须 0 条」的硬判据）；
  HZ3 活体读数：Torznab 侧同样枚举与否——现网 229 枚 api_tokens 全是哈希、
      没有明文可用，故不新建凭证（那是写操作），本条只记 WARN 读数；
  HZ4 静态同尺：RSS 取数谓词与 Torznab 取数谓词必须对「无存档」取同一口径
      （两边都排 or 两边都不排 ⇒ 绿；一边排一边不排 ⇒ 红，同 HJ3 手法）；
  HZ5 承重：把「200 + bencode」的正确形状喂给 HZ2 的违例探测器必须不报，
      把「400 + JSON 信封」的形状必须报（证明探测器不是恒真/恒假）；
  HZ6 台账棘轮：现网「过审 + 无存档」种数 ≤ 7，只许降不许升。

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


# —— HZ4 静态同尺：两个出口的取数谓词对「无存档」必须一致
rss_txt = strip(rd(RSS_RS))
tz_txt = strip(rd(TORZNAB_RS))
rss_excl = "torrent_files" in rss_txt
tz_excl = "torrent_files" in tz_txt
ok("HZ4 静态同尺：RSS 与 Torznab 对「无存档」必须取同一口径",
   rss_excl == tz_excl,
   "RSS 引用 torrent_files=%s；Torznab=%s（两边都不排=统一在册，"
   "一边排一边不排=口径分叉，会员在两个工具里看到的不一样）"
   % (rss_excl, tz_excl))

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

if LIVE:
    login("root", "password123")  # 顺带验鉴权链活着（不打印 token）
    pk = psql("SELECT passkey FROM users WHERE username='root';")
    mask = pk[:4] + "…" + pk[-4:]
    st, body = get("%s/rss/%s?limit=200" % (BASE, pk))
    xml = (body or b"").decode("utf-8", "replace") if isinstance(body, bytes)\
        else str(body)
    enc = enclosures(xml)
    ok("HZ2 在册：RSS 仍在给无存档种发 enclosure（排除掉即转红）",
       st == 200 and bool(set(i for i, _u in enc) & set(no_raw_ids)),
       "feed http=%s item=%d，其中无存档 ids=%s；命中即红说明已修，"
       "届时把本条翻成「必须 0 条」硬判据（passkey=%s 已打码）"
       % (st, len(enc), sorted(set(i for i, _u in enc) & set(no_raw_ids))[:6],
          mask))
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
        print("   [在册] 无存档 enclosure 实测 id=%s http=%s 响应=%s"
              % (bad_id[0], s2, txt.replace(pk, mask)))
        print("   [在册] 消费者拿到的是错误信封而不是 .torrent："
              "qBittorrent/FlexGet/autobrr 按 guid 轮询会无限重试同一枚种，"
              "而「请联系管理组补档」这句中文只有本站界面看得到")
    print("   [WARN] HZ3 Torznab 侧未做活体：现网 api_tokens=%s 枚全部是哈希，"
          "没有明文可用；新建凭证属写操作，本闸门不擅自做（静态口径见 HZ4）"
          % psql("SELECT count(*) FROM api_tokens;"))
else:
    print("[HZ1/HZ2/HZ3] HZ_LIVE=0 跳过活体")

summary()
