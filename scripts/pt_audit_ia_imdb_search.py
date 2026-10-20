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
  IA4 活体读数（默认关）：IA_LIVE=1 时打一次现网 search_area=4 命中数，
      只记读数不硬断言（镜像更新后应等于 IA3 的新谓词命中数，届时升硬判据）。

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

LIVE = os.environ.get("IA_LIVE", "0") == "1"
SYN = "apps/api/src/torrents/syn_search.rs"
NEEDLE = "33051301"


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


arm = arm4(strip(rd(SYN)))
has_mi = "media_info->>'imdb'" in arm
has_col = "t.imdb_id" in arm
ok("IA1 静态：area=4 谓词必须同时读 media_info.imdb 与 imdb_id 列",
   has_mi and has_col,
   "臂内出现的键=%s（缺 imdb_id＝编辑填过 IMDb 的种永久隐身）"
   % [k for k, v in (("media_info", has_mi), ("imdb_id", has_col)) if v])

# IA2 承重：旧臂形状（只有 media_info）必须被同一对条件判成不合格
OLD_ARM = "t.media_info->>'imdb' ILIKE $6{esc} OR t.descr ILIKE"
ok("IA2 承重：只读 media_info 的旧臂被 IA1 那对条件判违例",
   not ("media_info->>'imdb'" in OLD_ARM and "t.imdb_id" in OLD_ARM),
   "旧臂若仍算合格＝判据恒真，不算承重")

PAT = "'%" + NEEDLE + "%'"
BASE_W = "SELECT count(*) FROM torrents t WHERE t.approval_status=1 AND ("
OLD_SQL = BASE_W + "t.media_info->>'imdb' ILIKE " + PAT + ")"
NEW_SQL = (BASE_W + "t.media_info->>'imdb' ILIKE " + PAT
           + " OR t.imdb_id ILIKE " + PAT + ")")
old_n = int(psql(OLD_SQL) or 0)
new_n = int(psql(NEW_SQL) or 0)
ok("IA3 真库 A/B：补读 imdb_id 后该关键字必须多命中且 >0",
   new_n > old_n and new_n > 0,
   "旧谓词命中=%d 新谓词命中=%d；差额＝只有 imdb_id 能命中的种数。"
   "这条是数据库级红证，与现网镜像是否含本笔无关" % (old_n, new_n))

if LIVE:
    try:
        tok = login("root", "password123")
        url = ("%s/torrents?limit=50&alive=0&search_area=4&search=%s"
               % (BASE, urllib.parse.quote("TT" + NEEDLE)))
        with urllib.request.urlopen(urllib.request.Request(
                url, headers={"Authorization": "Bearer " + tok})) as x:
            import json
            d = json.loads(x.read()).get("data") or {}
        got = len(d.get("items") or [])
        print("   [读数] 现网 search_area=4 命中=%d（新镜像落地后应=%d，"
              "届时把 IA3 的期望搬到活体做硬判据）" % (got, new_n))
    except urllib.error.HTTPError as e:
        print("   [WARN] 活体取数 http=%s：%s"
              % (e.code, e.read()[:80].decode("utf-8", "replace")))
    except Exception as e:  # noqa: BLE001
        print("   [WARN] 活体取数未完成（多半是 flux-api 正在重启）：%s"
              % type(e).__name__)
else:
    print("[IA4] IA_LIVE=0 跳过活体读数（默认关：镜像重启窗口会误读）")

summary()
