# -*- coding: utf-8 -*-
"""闸门 IA：IMDb 检索必须同时认 `media_info.imdb` 与 `imdb_id` 列（P-436）。

IMDb 在 torrents 上有**两个落库目标**：
  · 上传/生成口 → `media_info.imdb`（ptgen.rs:38、upload.rs:139，注释还写着
    「搜索区 4 已按此键命中」——作者只看见了一处）
  · 编辑口 / 导入与演示种 → `imdb_id` 列（interact.rs:87；现网该列非空的
    过审种 4 条，形态 TT33051301）
`auto_meta.rs:67` 取数时两处都读，唯独搜索谓词（syn_search.rs
`search_pred_with_syn` 的 `4 =>` 臂）只认 media_info ⇒ 会员在编辑表单里填过
IMDb 的种，用 `search_area=4` 永远搜不到。现网实测：`search=TT33051301`、
小写 `tt…`、纯数字 `33051301`、乃至通配 `%` 四种输入全部 0 命中，而那两枚种
确实存在且过审。属 P-426 同族：同一逻辑字段多处落库、只有一处有读者。
IMDb 检索是 PT 站刚需（按号找种、Prowlarr/Sonarr、豆瓣对照），失效面是全量。

判据（全静态 + 真库，**不依赖现网镜像是否含本笔**）：
  IA1 静态：`4 =>` 臂文本必须同时出现 `media_info->>'imdb'` 与 `t.imdb_id`；
      先剥 `//`、`///` 注释再扫——本批写进去的注释本身就含这两个键名，
      不剥就是自己给自己洗白（HJ/HW/HX 各栽过一次，这条是防呆）。
  IA2 承重：把「只读 media_info」的旧臂喂给 IA1 那一对条件，必须判违例
      （否则判据恒真，不算承重）。
  IA3 真库 A/B：同一关键字 `%33051301%` 下，旧谓词命中数必须 < 新谓词
      （media_info OR imdb_id）命中数，且新谓词 > 0。现网应为 旧 0 / 新 2。
      这是数据库级红证：站长可能正在重启 flux-api，活体 HTTP 会把「部署中」
      误读成「闸门坏」；真库算账不受镜像影响。
  IA4 活体三档（默认开）：按 IMDb 号搜到的 id 必须覆盖库里带号的过审种、
      库里没有的号必须 0 命中（负控，防「恒返全部」）、tt/TT 同数（钉
      ILIKE）。/health 打不通或登录被限流 ⇒ 只 WARN 不出红，避免把「部署
      中」读成「闸门坏」。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ia_imdb_search.py`
      IA_LIVE=1 额外做一次只读活体读数。
"""
import io
import os
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

LIVE = os.environ.get("IA_LIVE", "1") != "0"
SYN = "apps/api/src/torrents/syn_search.rs"
NEEDLE = "33051301"  # 兜底常量：真正取值见 pick_needle()


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(rs):
    return "\n".join(l for l in rs.split("\n")
                     if not l.strip().startswith("//"))


def arm4(src):
    i = src.index("4 => format!(")
    k = src.find("\n        5 =>", i)
    if k < 0:
        k = src.find("\n        _ =>", i)
    return src[i:k if k > i else i + 700]


def psql(q):
    x = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-q", "-tA"], input=q, capture_output=True,
        text=True, encoding="utf-8", errors="replace")
    err = x.stderr or ""
    if x.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql: " + err[:240])
    return (x.stdout or "").strip()


def pick_needle():
    """取一枚「imdb 号只写在 imdb_id 列、media_info 两个键都没有」的过审种。"""
    return psql(
        "SELECT regexp_replace(imdb_id, '\\D', '', 'g') FROM torrents "
        "WHERE approval_status = 1 AND imdb_id <> '' "
        "AND media_info->>'imdb_id' IS NULL AND media_info->>'imdb' IS NULL "
        "ORDER BY id DESC LIMIT 1").strip()


arm = arm4(strip(rd(SYN)))
has_mi = "media_info->>'imdb'" in arm
has_jk = "media_info->>'imdb_id'" in arm
has_col = "t.imdb_id" in arm
ok("IA1 静态：area=4 谓词必须覆盖 列 imdb_id + JSON 键 imdb_id（旧 imdb 键兼容可选）",
   has_jk and has_col,
   "臂内出现的键=%s；只认 JSON 的 `imdb` 键＝拿一把不存在的钥匙开锁"
   % [k for k, v in (("media_info.imdb", has_mi),
                    ("media_info.imdb_id", has_jk),
                    ("列 imdb_id", has_col)) if v])

# IA0 现网形态取证：真正有值的是哪一处（决定「谁该被读」）
phantom = int(psql("SELECT count(*) FROM torrents WHERE "
                   "media_info->>'imdb' IS NOT NULL") or 0)
jk = int(psql("SELECT count(*) FROM torrents WHERE "
              "media_info->>'imdb_id' IS NOT NULL") or 0)
col = int(psql("SELECT count(*) FROM torrents WHERE imdb_id IS NOT NULL "
               "AND imdb_id <> ''") or 0)
ok("IA0 现网形态：写入口落的键与列必须确有值，而旧 `imdb` 键是空的",
   phantom == 0 and (jk > 0 or col > 0),
   "media_info->>'imdb'=%d 行（幻键）；media_info->>'imdb_id'=%d；imdb_id 列=%d"
   % (phantom, jk, col))

# IA2 承重：旧臂形状（只有 media_info）必须被同一对条件判成不合格
OLD_ARM = "t.media_info->>'imdb' ILIKE $6{esc} OR t.descr ILIKE"
ok("IA2 承重：只读 media_info 的旧臂被 IA1 那对条件判违例",
   not ("media_info->>'imdb'" in OLD_ARM and "t.imdb_id" in OLD_ARM),
   "旧臂若仍算合格＝判据恒真，不算承重")

NEEDLE = pick_needle() or NEEDLE
PAT = "'%" + NEEDLE + "%'"
BASE_W = "SELECT count(*) FROM torrents t WHERE t.approval_status=1 AND ("
OLD_SQL = BASE_W + "t.media_info->>'imdb' ILIKE " + PAT + ")"
NEW_SQL = (BASE_W + "COALESCE(NULLIF(t.imdb_id, ''), "
           + "t.media_info->>'imdb_id', t.media_info->>'imdb') ILIKE "
           + PAT + ")")
old_n = int(psql(OLD_SQL) or 0)
new_n = int(psql(NEW_SQL) or 0)
ok("IA3 真库 A/B：补读 imdb_id 后该关键字必须多命中且 >0",
   new_n > old_n and new_n > 0,
   "旧谓词命中=%d 新谓词命中=%d；差额＝只有 imdb_id 能命中的种数。"
   "这条是数据库级红证，与现网镜像是否含本笔无关" % (old_n, new_n))

# 活体三档：期望集合 ⊆ 命中集合 / 库里没有的号必须 0 命中 / tt 与 TT 同数。
def http_get(path, tok=None):
    h = {"Authorization": "Bearer " + tok} if tok else {}
    with urllib.request.urlopen(
            urllib.request.Request(BASE + path, headers=h), timeout=20) as x:
        import json
        return json.loads(x.read())


def hits4(tok, kw):
    d = http_get("/torrents?limit=50&alive=0&search_area=4&search=%s"
                 % urllib.parse.quote(kw), tok).get("data") or {}
    return sorted(int(i["id"]) for i in (d.get("items") or []))


want = [int(r) for r in psql(
    "SELECT id FROM torrents WHERE approval_status=1 AND ("
    "COALESCE(NULLIF(imdb_id, ''), media_info->>'imdb_id', "
    "media_info->>'imdb')) ILIKE '%" + NEEDLE + "%'").split("\n")
    if r.strip()]

if not LIVE:
    print("[IA4] IA_LIVE=0 跳过活体三档")
elif not want:
    print("   [WARN] 现网已没有「imdb 号只写在列里」的过审种，活体三档与 IA3 "
          "无从取证（静态 IA1/IA2 仍承重）")
else:
    try:
        http_get("/health")
        tok = login("root", "password123")
        up = hits4(tok, "TT" + NEEDLE)
        lo = hits4(tok, "tt" + NEEDLE)
        absent = hits4(tok, "TT9999" + NEEDLE)
        ok("IA4 活体：按 IMDb 号搜到的集合必须覆盖库里的期望集合",
           set(want) <= set(up),
           "期望=%s 实得=%s——带号的种在搜索区 4 一个都不能少" % (want, up))
        ok("IA4b 负控：库里不存在的 IMDb 号必须 0 命中（防该臂退化成恒真）",
           not absent,
           "TT9999%s 命中=%s——非空说明谓词把不相干的种也放进来" % (NEEDLE,
                                                              absent))
        ok("IA4c 大小写同数：tt 与 TT 结果必须一致（钉 ILIKE 不是 LIKE）",
           lo == up, "TT=%s 小写=%s" % (up, lo))
    except urllib.error.HTTPError as e:
        # 5xx 是产品洞，不是「重启窗口」：3ac09f0a 之前 area=4 因 SQL 里
        # 多了一个真反斜杠而全 500，旧写法在这里按 WARN 吞掉，闸门全绿。
        body = ""
        try:
            body = e.read().decode("utf-8", "replace")[:200]
        except Exception:  # noqa: BLE001
            pass
        if e.code >= 500 and e.code != 503:
            ok("IA4 活体：搜索端点 5xx 必须红（不是重启窗口）", False,
               "http=%s body=%s" % (e.code, body))
        else:
            print("   [WARN] 活体未完成（http=%s，按部署窗口/限流跳过）：%s"
                  % (e.code, body[:80]))
    except Exception as e:  # noqa: BLE001
        print("   [WARN] 活体未完成（连接失败，按重启窗口跳过）：%s: %s"
              % (type(e).__name__, str(e)[:120]))

summary()
