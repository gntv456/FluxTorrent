# -*- coding: utf-8 -*-
"""常驻台账：个人设置里「有控件、存进库、全站没人读」的假开关（集合级棘轮）。

会员视角：设置页写着「每页主题数（0=默认）」，我填 2、保存、回显也是 2，
可论坛版块页照样 20 条。这类字段的共同形状是——
`PUT /me/settings` 46 列全收，UI 也画了控件，
但 api / worker / tracker / web 里没有任何一处读它。

活体取证（2026-10-21，探针号 6273，收尾把偏好改回 0 并回读）：
  topics_per_page：设 2 → 库里 2 → 设置页回显 2 → GET /forums/1/topics 仍 20 条
  posts_per_page：设 1 → 库里 1 → 主题 472（库里 3 层）仍回 2 条
⇒ 不是「没存进去」，也不是「存了但回显坏」，是**没有读者**。

尺怎么算（三条都被现网打过脸，逐条自证过才敢用）：
  · 读者扫描范围＝apps/api + apps/worker + apps/tracker + crates + apps/web，
    排除设置表单本身与写/回显口（那只能证明「有控件」，不证明「有人读」）；
  · **必须减掉人工核实过的活口**：`site_language` 纯 grep 会被误判成死开关，
    实际 usercp.tsx:175/217 拿到它后调 syncLocaleCookie(want) + router.refresh()
    ⇒ 语言是真生效的（函数名不含 cookie/document 之类关键字，机器认不出）。
    同类还有 `tooltip`（torrent-table-badges.tsx 读它决定悬停详情）。
  · 因此台账里的每一项都只算**候选**：要么补上读者，要么把控件撤掉/标「未生效」，
    定性归站方；本闸门只保证「不再悄悄变多」和「修好了会来催删桩」。

判据：
  D0 尺自证：写口列数 >40、且抽样的字段确实在设置面板里有控件
  D1 台账集合相等：算出的候选死开关 == 登记值（多=新死开关；少=已接通，删桩）
  D2 活口名单：ALIVE 里每个字段都能静态找到消费点（防把真生效的字段记成死的）
  D3 活体钉桩（EH5 形状）：topics_per_page=2 时版块列表**仍**回 20 条——
     这条在接通之后会转红，红即提醒删桩并把该字段移出 D1 台账
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dead_settings.py
      FLUX_API_BASE 默认 http://127.0.0.1:8080/api/v1；PROBE_UID 默认 6273
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
KV = os.path.join(ROOT, "apps", "api", "src", "auth_http", "me_settings_kv.rs")
SCAN_DIRS = ["apps/api/src", "apps/worker/src", "apps/tracker/src", "crates",
             "apps/web"]
# 只证明「有控件」或「能写能回显」的地方，不算读者。
# 注意写口文件名是 me_settings_kv.rs，写 `me_settings\.rs` 匹配不到它，
# 于是 46 个列名全被算成「有人读」——台账会永远空、D1 永远绿（10-21 实踩过）。
FORM = re.compile(
    r"(usercp[\w.-]*\.tsx?|me_settings"
    r"|/i18n/(zh-CN|en|zh-TW|ja)\.ts)")
UID = int(os.environ.get("PROBE_UID", "6273"))
# 人工逐个核实过的「真生效」字段（消费点写在注释里，D2 逐条验）
ALIVE = {
    "site_language": "apps/web/components/usercp.tsx",
    "tooltip": "apps/web/components/torrent-table-badges.tsx",
}
# 登记在册的候选死开关（只许缩小）。来源：写口 46 列全量扫 + 人工排除误判
DEAD_LEDGER = {
    "append_new", "append_picked", "append_promotion", "append_sticky",
    "bm_icon", "browsecat", "click_topic", "comment_pm", "delete_pm",
    "dl_icon", "fontsize", "notify_hr", "pm_per_page", "posts_per_page",
    "save_pm", "show_ad", "show_com_num", "show_comment", "show_description",
    "show_imdb", "show_last_com", "stylesheet", "time_type",
    "topics_per_page", "tt_last_post",
}


def write_cols():
    src = io.open(KV, encoding="utf-8").read()
    m = re.search(r"UPDATE users SET([\s\S]*?)WHERE", src)
    return sorted(set(re.findall(r"(\w+)\s*=\s*COALESCE", m.group(1))))


def readers_of(col):
    hits = []
    for d in SCAN_DIRS:
        base = os.path.join(ROOT, d)
        for root, _, files in os.walk(base):
            if "node_modules" in root or ".next" in root:
                continue
            for fn in files:
                if not fn.endswith((".rs", ".ts", ".tsx")):
                    continue
                p = os.path.join(root, fn)
                rel = os.path.relpath(p, ROOT).replace("\\", "/")
                if FORM.search(rel):
                    continue
                t = io.open(p, encoding="utf-8", errors="replace").read()
                if re.search(r"\b%s\b" % re.escape(col), t):
                    hits.append(rel)
    return hits


def panel_has_control(col):
    for root, _, files in os.walk(os.path.join(ROOT, "apps", "web")):
        if "node_modules" in root or ".next" in root:
            continue
        for fn in files:
            if not fn.startswith("usercp") or not fn.endswith(".tsx"):
                continue
            t = io.open(os.path.join(root, fn), encoding="utf-8",
                        errors="replace").read()
            if re.search(r"\b%s\b" % re.escape(col), t):
                return True
    return False


def tok():
    out = subprocess.run(
        [sys.executable, os.path.join(ROOT, "_mint_jwt.py"),
         "--uid", str(UID), "--class", "1"],
        capture_output=True, text=True, encoding="utf-8").stdout
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
            return e.code, {"raw": raw[:120].decode("utf-8", "replace")}
    except Exception as ex:
        return 0, {"err": str(ex)[:120]}


def main():
    cols = write_cols()
    # ---------- D0 尺自证 ----------
    ok("D0 尺自证：写口 %d 列（>40）且抽样字段在设置面板确有控件" % len(cols),
       len(cols) > 40 and all(panel_has_control(c) for c in
                             ["topics_per_page", "posts_per_page", "fontsize"]),
       "写口列数或控件不见了，说明这条尺看的已经不是设置面板")
    # ---------- D2 活口名单（防误判） ----------
    bad_alive = []
    for col, where in ALIVE.items():
        t = io.open(os.path.join(ROOT, where), encoding="utf-8",
                    errors="replace").read()
        if not (re.search(r"\b%s\b" % col, t)
                and re.search(r"syncLocaleCookie|title|Tooltip|tooltip", t)):
            bad_alive.append((col, where))
    ok("D2 已核实活口 %d 个都找得到消费点（异常 %s）" % (len(ALIVE), bad_alive or "无"),
       not bad_alive, "消费点没了＝它可能已退化成死开关，要么移进 D1 台账")
    # ---------- D1 台账集合 ----------
    computed = {c for c in cols
                if c not in ALIVE
                and not readers_of(c)
                and panel_has_control(c)}
    extra = sorted(computed - DEAD_LEDGER)
    gone = sorted(DEAD_LEDGER - computed)
    ok("D1 候选死开关集合必须等于台账（多 %s / 少 %s）"
       % (extra or "无", gone or "无"), computed == set(DEAD_LEDGER),
       "多＝又添了不生效的控件；少＝有字段接通了，请把它从台账里删掉并补活口名单")
    print("   [在册 %d] %s" % (len(DEAD_LEDGER), ", ".join(sorted(DEAD_LEDGER))))
    # ---------- D3 活体钉桩（接通后会转红） ----------
    token = tok()
    if not token:
        print("   [跳过 D3] 探针令牌取不到（_mint_jwt.py 需要容器在跑）")
    else:
        call("PUT", "/me/settings", token, {"topics_per_page": 2})
        st, r = call("GET", "/forums/1/topics", token)
        d = r.get("data")
        n = len(d if isinstance(d, list) else
                 ((d or {}).get("items") or (d or {}).get("topics") or []))
        echo = (call("GET", "/me/settings", token)[1].get("data") or {}) \
            .get("topics_per_page")
        call("PUT", "/me/settings", token, {"topics_per_page": 0})
        back = (call("GET", "/me/settings", token)[1].get("data") or {}) \
            .get("topics_per_page")
        ok("D3 钉桩：偏好=2（回显 %s）时版块列表仍回 %d 条（默认 20）——"
           "接通后这条会红，红即删桩" % (echo, n),
           echo == 2 and n > 2,
           "已经生效了？把 topics_per_page 移出 D1 台账并删掉本条钉桩")
        ok("D3b 探针偏好已还原（回读 %s）" % back, back in (0, None),
           "还原失败会把这个号的列表页长期留在 2 条")
    summary()


if __name__ == "__main__":
    main()
