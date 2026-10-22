# -*- coding: utf-8 -*-
"""常驻闸门：版块每页主题数必须听控制面板那一列（P-449）。

会员视角：控制面板 → 追踪偏好 → 「每页主题数」，我填 2、保存、回显也是 2，
可版块页照样一次几十条。这一列（`users.topics_per_page`，0024）此前全站
只有两处读它：设置写口 `me_settings_kv.rs` 与设置页自己——列表端点
`/forums/{id}/topics` 的每页条数是硬常量 `TOPIC_PAGE_SIZE = 50`。

修法沿用仓里既有惯例（P-275 在种子列表上就是这套），分三层，不另起机制：
  · 折算写成纯函数 `page_size_of(本人偏好, 站点默认)`——
    偏好 > 站点默认 `topics_per_page_default` > 硬常量 50；
    0/负数 = 未设置（建表默认就是 0），正数生效、越界夹到
    `TOPIC_PAGE_MAX`。上限不自己拍：0039 给这一键登记的元数据就是
    min 1 / max 500，后台那格控件按它校验，代码夹到同一个界才不会
    「管理员填 300、程序给 200」两套口径；
  · 点查下沉 H 层 `repo::user_prefs::board_per_page`（FRAME §2：H 不
    import sqlx 的反面是 SQL 只住在 repo），站点那一键走
    `repo::settings::read`（按 name 读 site_settings 的唯一居所）；
  · 读失败按「无偏好」处理，偏好不该把列表主路径拖倒。

P-450 是本批的续集：接线时才发现后台有个「主题列表默认每页」（现网值 20），
它躺在 `fake_switch_baseline.json` 的存量登记里没人读——站长改完保存成功、
回显正常、版块页照旧 50 条。**行为可见变化**：站方重建后，没设过偏好的
会员看到的版块页从 50 条/页变成站点值 20 条/页（这才是后台那一格一直说
的数）；要维持 50，一条 `UPDATE site_settings SET value='50' WHERE
name='topics_per_page_default';` 即可，不必改代码。

判据分层（每层各管一段，别拿其中一层冒充另一层）：
  G0 正控   修前的 topic.rs：`let per = TOPIC_PAGE_SIZE;` 且函数体不出现
            那一列 ⇒ 尺抓得到旧形状
  G1        现网接线：per 走 per_page_from_pref，折函数取 repo 点查，
            repo 里那句 SQL 真读 topics_per_page
  G2        纯函数行为：cargo test --bin flux-api per_page_folds_pref
            （未设置/负数/2/50/100000/1 六档；这是行为不是文本）
  G3        SQL 层：同一段 WHERE 只换 LIMIT，现网版块 1 的行数随之变
            ⇒ 「这一个数」确实决定这一页给几条
  G4        前端成环：forums/[id]/page.tsx 画 Pager 用的是回显的 data.per
            （后端改了、前端还按 50 画页码＝只修了一半）
  G5        台账联动：topics_per_page 已移出死开关台账，剥注释后 reader
            扫描命中 topic.rs 与 repo/user_prefs.rs
  G6        界面层：默认**只报读数**——现网二进制还没有本批修复，
            站方重建后加 `--live` 把这条升硬

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_forum_per_page.py
      站方重建 api 之后：同上加 --live
"""
import io
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import BASE, ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOPIC = "apps/api/src/community_http/topic.rs"
PREFS = "apps/api/src/repo/user_prefs.rs"
WEBPAGE = os.path.join("apps", "web", "app", "(main)", "forums", "[id]",
                       "page.tsx")
FIX = "0c87e1ce"           # 本批修复的提交（G0 从它的父提交取旧形状）
UID = int(os.environ.get("PROBE_UID", "6273"))
LIVE = "--live" in sys.argv
FORM = re.compile(r"(usercp[\w.-]*\.tsx?|me_settings"
                  r"|/i18n/(zh-CN|en|zh-TW|ja)\.ts)")
SCAN_DIRS = ["apps/api/src", "apps/worker/src", "apps/tracker/src", "crates",
             "apps/web"]


def read(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip_comments(t):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", t)
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-v", "ON_ERROR_STOP=1", "-f", "-"],
        input=sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace", cwd=ROOT)
    if p.returncode != 0:
        raise RuntimeError("psql 失败：%s" % ((p.stderr or "")[:200]))
    return (p.stdout or "").strip()


def tok(uid):
    out = subprocess.run(
        [sys.executable, os.path.join(ROOT, "_mint_jwt.py"),
         "--uid", str(uid), "--class", "1"],
        capture_output=True, text=True, encoding="utf-8",
        errors="replace").stdout
    m = re.search(r"[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+", out)
    return m.group(0) if m else None


def call(method, path, token, body=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=25) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:100].decode("utf-8", "replace")}
    except Exception as ex:
        return 0, {"err": str(ex)[:100]}


print("== G0/G1 接线形状 ==")
old = subprocess.run(["git", "show", "%s:%s" % (FIX + "^", TOPIC)],
                     capture_output=True, text=True, encoding="utf-8",
                     errors="replace", cwd=ROOT)
if old.returncode != 0:
    raise SystemExit("取文失败 %s^:%s / %s" % (FIX, TOPIC,
                                               (old.stderr or "")[:150]))
old_t, new_t, prefs_t = old.stdout or "", read(TOPIC), read(PREFS)
ok("G0 正控：修前 per 取硬常量且函数体不出现那一列（尺抓得到旧形状）",
   re.search(r"let per = TOPIC_PAGE_SIZE;", old_t) is not None
   and "topics_per_page" not in old_t)
ok("G1a 现网 per 走三层折叠（偏好 > 站点默认 > 常量）",
   re.search(r"let per = page_size_of\(", new_t) is not None
   and "let per = TOPIC_PAGE_SIZE;" not in new_t
   and "per_page_from_pref" not in new_t)
ok("G1b 折函数取的是 repo 点查（SQL 不留在 H 层）",
   "user_prefs::board_per_page" in new_t
   and re.search(r"SELECT topics_per_page FROM users WHERE id = \$1",
                 prefs_t) is not None)
ok("G1c 读失败按「无偏好」兜，不拿偏好把列表主路径拖倒",
   re.search(r"pub async fn board_per_page[\s\S]{0,400}"
             r"unwrap_or\(0\)", prefs_t) is not None)
ok("G1d 站点默认那层也接上了（P-450，走 repo::settings 的单键读居所）",
   "topics_per_page_default" in new_t
   and "crate::repo::settings::read" in new_t)

print("== G2 纯函数行为（真跑单元测试）==")
t2 = subprocess.run(["cargo", "test", "--bin", "flux-api",
                     "per_page_folds_pref", "--", "--nocapture"],
                    capture_output=True, text=True, encoding="utf-8",
                    errors="replace", cwd=ROOT)
tail2 = (t2.stdout or "") + (t2.stderr or "")
ok("G2 cargo test per_page_folds_pref 通过（rc=%d）" % t2.returncode,
   t2.returncode == 0 and "test result: ok" in tail2,
   "红账=%s" % [ln for ln in tail2.splitlines()
                if "FAILED" in ln or "panicked" in ln][:3])

print("== G3 SQL 层：LIMIT 就是那个数 ==")
seg = ("SELECT count(*) FROM (SELECT t.id FROM topics t"
       " WHERE t.forum_id = 1 ORDER BY t.id LIMIT %s) x")
n50 = int(psql(seg % 50))
n2 = int(psql(seg % 2))
ok("G3 同一段 WHERE 只换 LIMIT：50→%d 行、2→%d 行" % (n50, n2),
   n2 == min(2, n50) and n50 >= n2 and n2 > 0,
   "版块 1 至少要有一条主题，否则这条判据是空的")

print("== G4 前端成环 ==")
web = read(WEBPAGE)
ok("G4 版块页画 Pager 用的是回显的 data.per（不是自己写 50）",
   re.search(r"data\.per\s*\?\?", web) is not None,
   "后端改了、前端按老数画页码＝只修一半")

print("== G5 台账联动 ==")
ds = read(os.path.join("scripts", "pt_audit_dead_settings.py"))
mm = re.search(r"DEAD_LEDGER = \{([^}]*)\}", ds or "")
ledger = set(re.findall(r"[a-z_]+", mm.group(1))) if mm else set()
ok("G5a topics_per_page 已移出死开关台账（%s）"
   % ("仍在册＝桩没撤" if "topics_per_page" in ledger else "已撤"),
   "topics_per_page" not in ledger)
hits = []
for d in SCAN_DIRS:
    for dp, _, files in os.walk(os.path.join(ROOT, d)):
        if "node_modules" in dp or ".next" in dp:
            continue
        for fn in files:
            if not fn.endswith((".rs", ".ts", ".tsx")):
                continue
            rel = os.path.relpath(os.path.join(dp, fn), ROOT)
            rel = rel.replace("\\", "/")
            if FORM.search(rel):
                continue
            if re.search(r"\btopics_per_page\b", strip_comments(read(rel))):
                hits.append(rel)
ok("G5b 剥注释后扫到读者：%s" % sorted(hits), len(hits) > 0,
   "空＝读侧接线又被摘掉")

print("== G6 界面层（%s）==" % ("升硬" if LIVE else "默认只报读数"))
token = tok(UID)
if not token:
    print("   [跳过 G6] 探针令牌取不到（_mint_jwt.py 需要容器在跑）")
else:
    call("PUT", "/me/settings", token, {"topics_per_page": 2})
    st, r = call("GET", "/forums/1/topics", token)
    d = r.get("data") or {}
    per = d.get("per")
    rows = d.get("topics")
    n = len(rows) if isinstance(rows, list) else -1
    call("PUT", "/me/settings", token, {"topics_per_page": 0})
    back = (call("GET", "/me/settings", token)[1].get("data") or {}) \
        .get("topics_per_page")
    ok("G6b 探针偏好已还原（回读 %s）" % back, back in (0, None),
       "还原失败会把这个号的列表页长期留在 2 条")
    msg = ("GET /forums/1/topics（%d）per=%s 行数=%s" % (st, per, n))
    if LIVE:
        ok("G6a 偏好=2 ⇒ 这一页就给 2 条、信封回显 2：%s" % msg,
           per == 2 and n <= 2,
           "现网二进制还没带上本批修复时这条会红——那是部署账不是产品账")
    else:
        print("   [WARN 不判红] %s｜现网镜像若无本批修复，per 仍是 50。"
              % msg)
        print("   站方重建 api 后用 --live 升硬。")

print("== G7 站点设定台账（假开关门禁）联动 ==")
SITE_KEY = "topics_per_page_default"
SIBLINGS = ["posts_per_page_default", "pm_per_page_default",
            "torrents_per_page_default"]
bs = json.loads(read(os.path.join("scripts", "fake_switch_baseline.json"))
                 or "{}")
inb = set(str(x) for x in (bs.get("dead") or []))
site_val = psql("SELECT coalesce(value,'<无行>') FROM site_settings"
                " WHERE name = '%s';" % SITE_KEY)
ok("G7a %s 已从假开关存量登记里移出（现网值 %s 条/页）"
   % (SITE_KEY, site_val or "?"), SITE_KEY not in inb,
   "接了却留登记＝台账说谎；站方重建后用 "
   "node scripts/fake_switch_guard.mjs --update 收缩基线")
ok("G7b 同族三键仍在册（本批没接它们，基线不许顺手删）：%s"
   % [k for k in SIBLINGS if k in inb],
   all(k in inb for k in SIBLINGS),
   "少一条＝有人删了没接线的登记（posts/pm 端点不分页，torrents 走"
   " P-275 那条 clamp 缺省，都不读这个键）")
print("   [待裁] 站点级「每页默认」四键里本批只接了主题这一键；其余三键"
      " 接它需要先加分页/改缺省口径，属功能改动。")

summary()
