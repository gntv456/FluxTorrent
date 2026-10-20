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
（topics_per_page 已于 P-449 接通读侧，行为判据搬到
`pt_audit_forum_per_page.py`；posts_per_page 仍在册——本主题楼层列表
端点根本不分页（一次 `LIMIT 200`），接它等于新加分页，属加功能不是接线。）

尺怎么算（三条都被现网打过脸，逐条自证过才敢用）：
  · 读者扫描范围＝apps/api + apps/worker + apps/tracker + crates + apps/web，
    排除设置表单本身与写/回显口（那只能证明「有控件」，不证明「有人读」）；
  · **先剥 `//` 与 `/* */` 注释再扫**：注释里提一句字段名不算读者。
    这条是 P-447 的负控抓出来的——把谓词翻掉之后「还有没有读者」仍判绿，
    因为日志字符串与文档注释里写着那个列名；
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
  D3 写口回环：topics_per_page=2 存得下也回显（P-449 之前这里是
     「列表仍回几十条」的活体钉桩，接通之后按本文件自己的口径撤桩；
     行为判据住在 pt_audit_forum_per_page.py）
  D3b 探针偏好已还原（活体闸门最怕留残料）
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
# 人工逐个核实过的「真生效」字段：值是 (文件, 生效谓词)，谓词必须打到那个
# 字段**真正起作用的那行代码**，不是文件里出现过这个单词。
# 反面教材（10-21 实踩）：tooltip 原本被豁免，消费点写的是
# apps/web/components/torrent-table-badges.tsx——可那处只是注释里
# 「徽标类名与 tooltip 拼装」几个字。剥掉注释后 readers_of 返回空，而面板
# 三档值（minorimdb/medianimdb/off）除写口枚举校验外全仓零消费：它一直是
# 死开关，被旧 D2 的「单词共现」判据洗白。
ALIVE = {
    "site_language": (
        "apps/web/components/usercp.tsx", r"siteLangToLocale\("),
}
# 登记在册的候选死开关（只许缩小）。来源：写口 46 列全量扫 + 人工排除误判
# notify_hr 已接通（P-447，2026-10-21）：hr_resolve.rs 的达标消除信按
# `users.notify_hr` 执法，常驻闸门 scripts/pt_audit_hr_fulfilled_pref.py。
# save_pm 已接通（P-448，2026-10-21）：message_send.rs 按本人 save_pm 决定
# 发件副本，常驻闸门 scripts/pt_audit_pm_outbox_copy.py。
# topics_per_page 已接通（P-449，2026-10-21）：topic.rs 按本人那一列
# 折算每页条数（0=未设置、越界夹 200），常驻闸门
# scripts/pt_audit_forum_per_page.py；本文件原 D3 活体钉桩随之撤掉。
DEAD_LEDGER = {
    "append_new", "append_picked", "append_promotion", "append_sticky",
    "bm_icon", "browsecat", "click_topic", "comment_pm", "delete_pm",
    "dl_icon", "fontsize", "pm_per_page", "posts_per_page",
    "show_ad", "show_com_num", "show_comment", "show_description",
    "show_imdb", "show_last_com", "stylesheet", "time_type",
    "tooltip", "tt_last_post",
}


def write_cols():
    src = io.open(KV, encoding="utf-8").read()
    m = re.search(r"UPDATE users SET([\s\S]*?)WHERE", src)
    return sorted(set(re.findall(r"(\w+)\s*=\s*COALESCE", m.group(1))))


def strip_comments(text):
    """剥 `//` 行注释与 `/* */` 块注释：注释里提一句字段名不算读者。
    `https://` 的那个 `//` 不剥（(?<!:) 挡住），否则 URL 后半行消失，
    会把真读者误判成死开关。"""
    t = re.sub(r"/\*[\s\S]*?\*/", " ", text)
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


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
                t = strip_comments(io.open(p, encoding="utf-8",
                                           errors="replace").read())
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
    # ---------- D2 活口名单（谓词必须打到生效那行） ----------
    def _stripped(rel):
        return strip_comments(io.open(os.path.join(ROOT, rel),
                                      encoding="utf-8",
                                      errors="replace").read())

    bad_alive = []
    for col, (where, pat) in ALIVE.items():
        if not re.search(pat, _stripped(where)):
            bad_alive.append(col)
    ok("D2 已核实活口 %d 个都找得到生效代码（异常 %s）"
       % (len(ALIVE), bad_alive or "无"), not bad_alive,
       "消费点没了＝它可能已退化成死开关，要么移进 D1 台账")
    # ---------- D2b 尺的分辨力（A/B 同一把谓词） ----------
    # 旧 D2 只要求「文件里同时出现字段名和 Tooltip 字样」，注释就能满足，
    # 于是把死开关洗成活口（tooltip 实案）。新判据：同一把尺下，真生效的
    # 字段必须命中、被撤销豁免的必须不命中，缺一边都说明这把尺量的是单词
    # 而不是行为。
    hit_alive = bool(re.search(ALIVE["site_language"][1],
                               _stripped(ALIVE["site_language"][0])))
    badges = _stripped(
        "apps/web/components/torrent-table-badges.tsx")
    hit_dead = any(v in badges for v in ("minorimdb", "medianimdb",
                                         "users.tooltip"))
    ok("D2b 同一把尺能分辨生效与提及"
       "（site_language 命中=%s / badges 提 tooltip 档位=%s）"
       % (hit_alive, hit_dead), hit_alive and not hit_dead,
       "两头反了说明判据又在量单词共现")
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
    # ---------- D3 写口回环（原「列表仍回几十条」活体钉桩已撤）----------
    # P-449 把 topics_per_page 接上了读侧，钉桩的使命到此结束；
    # 行为层判据搬去 scripts/pt_audit_forum_per_page.py。
    token = tok()
    if not token:
        print("   [跳过 D3] 探针令牌取不到（_mint_jwt.py 需要容器在跑）")
    else:
        call("PUT", "/me/settings", token, {"topics_per_page": 2})
        echo = (call("GET", "/me/settings", token)[1].get("data") or {}) \
            .get("topics_per_page")
        call("PUT", "/me/settings", token, {"topics_per_page": 0})
        back = (call("GET", "/me/settings", token)[1].get("data") or {}) \
            .get("topics_per_page")
        ok("D3 写口回环：topics_per_page=2 存得下也回显（回读 %s）" % echo,
           echo == 2,
           "接线只动读侧——这条红说明写口或回显被碰坏了")
        ok("D3b 探针偏好已还原（回读 %s）" % back, back in (0, None),
           "还原失败会把这个号的列表页长期留在 2 条")
    summary()


if __name__ == "__main__":
    main()
