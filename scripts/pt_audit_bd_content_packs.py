# -*- coding: utf-8 -*-
"""闸门 BD：内容包（生态商店）安装与回滚实测（批四十，此前零覆盖）。

包能改分类学/外观/规则/素材/内嵌白名单，装错了要能回滚。判据一律「真发
请求 → 真读库」，SQL 字面量全 ASCII：
  S1 目录可见 + 普通成员七个包端点全 403
  S2 taxonomy 往返；在用维度标注不得随整维重建蒸发（section_dict 换 id 而
     torrent_sections 是 ON DELETE CASCADE）；同类型必须后装先回
  S3 theme 白名单外键两态都拒 + 色键回滚不被自己锁死；S4 rules 键白名单 +
     changed 与 will_change 同口径 + rule-try 只读
  S5 embed 保存侧六闸包侧一道不能少、不得覆写 builtin、快照拍全、能回滚
  S6 assets 包没声明的表不碰（种子每张只带一张表，缺键当空集 = 一键清空）
  S7 收尾：六张表 + 设置键回到基线，探针号墓碑，审计只增不删

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_bd_content_packs.py
"""
import json, os, re, subprocess, sys, time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 10000
CLEANUP, MEMBER = [], ""

def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:60], (r.stderr or "").strip()[:90]))
    return (r.stdout or "").strip()

def cnt(t, w=None):
    return psql("SELECT count(*) FROM %s%s" % (
        t, "" if w is None else " WHERE " + w))

def sval(k):
    return psql("SELECT value FROM site_settings WHERE name = '%s'" % k)

def blob(r):
    return json.dumps(r, ensure_ascii=False) if r is not None else ""

def data(r):
    return (r or {}).get("data") or {}

def imp(pack, confirm):
    return call("POST", "/admin/content-packs/import",
                {"pack": pack, "confirm": confirm}, token=ROOT)

def install(pack_id, confirm=True):
    return call("POST", "/admin/content-packs/install",
                {"pack_id": pack_id, "confirm": confirm}, token=ROOT)

def roll(row_id):
    return call("POST", "/admin/content-packs/rollback",
                {"id": row_id}, token=ROOT)

def last_row_for(pack_id):
    v = psql("SELECT id FROM content_packs WHERE pack_id = '%s'" % pack_id)
    return int(v) if v.isdigit() else 0

def pack(kind, pack_id, payload, name="BD 探针包"):
    return {"format": "fluxtorrent.contentpack", "version": 1, "kind": kind,
            "pack_id": pack_id, "name": name, "core_compat": "*",
            "pack_version": "1.0.0", "payload": payload}


BASE = {"cats": cnt("categories"), "dict": cnt("section_dict"),
        "kinds": cnt("section_kinds"), "medals": cnt("medals"),
        "frames": cnt("avatar_frames"), "embed": cnt("video_embed_rules"),
        "embed_nb": cnt("video_embed_rules", "NOT builtin"),
        "packs": cnt("content_packs"), "tagline": sval("site_tagline"),
        "rule": sval("rule_bank_term_rate"),
        "announce": sval("announce_interval")}
print("基线：%s" % BASE)
# ============ S1 目录与权限 ============
s, r = call("GET", "/admin/content-packs/catalog", token=ROOT)
items = data(r).get("items") or []
by_kind = {}
for it in items:
    by_kind[it.get("kind")] = by_kind.get(it.get("kind"), 0) + 1
ok("S1a catalog 200 且内置目录非空（%d 条 / %d）" % (len(items), s),
   s == 200 and len(items) > 0, "%s %s" % (s, blob(r)[:80]))
ok("S1b 内置目录含 taxonomy 与 assets 两类（%s）" % by_kind,
   by_kind.get("taxonomy", 0) >= 1 and by_kind.get("assets", 0) == 2,
   str(by_kind))
ok("S1c 未装任何包时目录不带 installed_row_id",
   all("installed_row_id" not in it for it in items),
   str([it.get("pack_id") for it in items if "installed_row_id" in it][:3]))

# 普通成员：七个口全 403（不是 401，令牌是真的，只是没权）
clear_register_rl()
s, r = call("POST", "/admin/invites", {"user_id": 1, "count": 1}, token=ROOT)
codes = (data(r).get("codes") or [])
MEM, WHY = None, ""
if not codes:
    WHY = "发码 http=%s %s" % (s, blob(r)[:110])
else:
    cid = codes[0]["id"]
    CLEANUP.append("DELETE FROM invites WHERE id = %d" % cid)
    cap = data(call("GET", "/auth/captcha")[1])
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    MEMBER = uname = "bd%d%d" % (TAG, int(time.time()) % 100000)
    if len(nums) < 2:
        WHY = "验证码题面解析不出 %r" % cap
    else:
        s, r = call("POST", "/auth/register", {
            "username": uname, "email": "%s@bd.local" % uname,
            "password": "password123", "invite_code": codes[0]["code"],
            "captcha_id": cap.get("captcha_id"),
            "captcha_answer": int(nums[0]) + int(nums[1]),
            "captcha_token": "", "fields": {}})
        if not data(r).get("token"):
            s, r = call("POST", "/auth/login", {"username": uname,
                                                "password": "password123"})
        MEM = data(r).get("token")
        if MEM:
            CLEANUP.append("UPDATE users SET status = 2 WHERE username = '%s'"
                           % uname)
        else:
            WHY = "注册/登录 http=%s %s" % (s, blob(r)[:140])
ok("S1d 探针普通成员可用（注册+登录）", MEM is not None, WHY)
ENDPOINTS = [
    ("GET", "/admin/content-packs/catalog", None),
    ("GET", "/admin/content-packs", None),
    ("GET", "/admin/content-packs/export?kind=theme", None),
    ("POST", "/admin/content-packs/import", {"pack": {}, "confirm": False}),
    ("POST", "/admin/content-packs/install", {"pack_id": "x", "confirm": 1}),
    ("POST", "/admin/content-packs/rollback", {"id": 1}),
    ("POST", "/admin/content-packs/rule-try",
     {"key": "rule_bank_term_rate", "expr": "1", "vars": {}})]
denied = ["%s %s=%s" % (m, u, st) for m, u, b in ENDPOINTS if MEM
          and (st := call(m, u, b, token=MEM)[0]) != 403]
ok("S1e 普通成员 7 个包端点全部 403（越权即改站型/外观/内嵌白名单）",
   MEM is not None and not denied, blob(denied)[:200])
# ============ S2 taxonomy：往返 + 在用标注 + 后装先回 ============
s, r = call("GET", "/admin/content-packs/export?kind=taxonomy", token=ROOT)
exp = data(r)
cats = (exp.get("payload") or {}).get("categories") or []
NEWCAT = 90000 + TAG % 500
probe_tid = psql("SELECT id FROM torrents ORDER BY id LIMIT 1")
drow = psql("SELECT id, kind FROM section_dict ORDER BY id LIMIT 1")
d_id, d_kind = (drow.split("|") + [""])[:2] if drow else ("0", "")
ordinal = psql("SELECT coalesce(max(ordinal),0)+1 FROM torrent_sections "
               "WHERE torrent_id = %s" % (probe_tid or 0))
if probe_tid and d_id != "0":
    psql("INSERT INTO torrent_sections (torrent_id, kind, dict_id, ordinal) "
         "VALUES (%s, '%s', %s, %s)" % (probe_tid, d_kind, d_id, ordinal))
CLEANUP.append("DELETE FROM torrent_sections WHERE torrent_id = %s "
               "AND ordinal = %s" % (probe_tid, ordinal))
ok("S2a 导出现网分类学（%d 分类 / %s 维度）" %
   (len(cats), cnt("section_kinds")),
   s == 200 and len(cats) > 0, "%s %s" % (s, blob(r)[:80]))
ok("S2b 探针在用维度标注已就位（dict_id=%s kind=%s）" % (d_id, d_kind),
   cnt("torrent_sections", "dict_id = %s" % d_id) == "1", "插入失败")

sec = exp["payload"].get("sections") or {}
p1 = pack("taxonomy", "bd.tax1.%d" % TAG, {"sections": sec, "categories":
      cats + [{"id": NEWCAT, "name": "BD探针甲", "icon_key": ""}]})
s, r = imp(p1, False)
d = data(r)
ok("S2c 预检只读：will_change=%s / 现网 %s / 包内 %s" %
   (d.get("will_change"), d.get("current_categories"),
    d.get("pack_categories")),
   s == 200 and d.get("dry_run") is True
   and d.get("pack_categories") == len(cats) + 1
   and cnt("categories") == BASE["cats"], "%s %s" % (s, blob(r)[:120]))

s, r = imp(p1, True)
ok("S2d 确认导入：分类 66→67，登记 1 行 kind=taxonomy",
   s == 200 and cnt("categories") == str(int(BASE["cats"]) + 1)
   and cnt("content_packs", "kind = 'taxonomy' AND "
                             "pack_id = 'bd.tax1.%d'" % TAG),
   "%s %s" % (s, blob(r)[:120]))
ok("S2e 在用维度标注存活且 dict_id 未变（整维重建会连带级联清掉）",
   cnt("torrent_sections", "dict_id = %s" % d_id) == "1"
   and cnt("section_dict", "id = %s" % d_id) == "1",
   "标注=%s 行=%s" % (cnt("torrent_sections", "dict_id=%s" % d_id),
                      cnt("section_dict", "id=%s" % d_id)))

p2 = pack("taxonomy", "bd.tax2.%d" % TAG, {"sections": sec, "categories":
      cats + [{"id": NEWCAT, "name": "BD探针甲", "icon_key": ""},
              {"id": NEWCAT + 1, "name": "BD探针乙", "icon_key": ""}]})
s, r = imp(p2, True)
row2 = last_row_for("bd.tax2.%d" % TAG)
row1 = last_row_for("bd.tax1.%d" % TAG)
ok("S2f 再装第二个分类学包（分类 +2）", s == 200
   and cnt("categories") == str(int(BASE["cats"]) + 2)
   and row2 > 0 and row1 > 0 and row2 > row1,
   "%s %s rows=%s,%s" % (s, blob(r)[:80], row1, row2))

s, r = roll(row1)
msg = blob(r)
ok("S2g 回滚旧包被拒（后装先回，否则新包被吃掉却仍显示已装）"
   "：400 + 指向 bd.tax2", s == 400 and "bd.tax2" in msg,
   "%s %s" % (s, msg[:120]))
ok("S2h 被拒的旧包登记行仍在（失败不得删登记）",
   cnt("content_packs", "id = %d" % row1) == "1")
s, r = roll(row2)
ok("S2i 回滚最新包 200 → 分类回到 +1，乙类消失",
   s == 200 and cnt("categories") == str(int(BASE["cats"]) + 1)
   and cnt("categories", "id = %d" % (NEWCAT + 1)) == "0",
   "%s %s" % (s, blob(r)[:120]))
s, r = roll(row1)
ok("S2j 再回滚旧包 200 → 分类/登记回到基线，甲类消失",
   s == 200 and cnt("categories") == BASE["cats"]
   and cnt("categories", "id = %d" % NEWCAT) == "0"
   and cnt("content_packs") == BASE["packs"],
   "%s %s cats=%s" % (s, blob(r)[:120], cnt("categories")))
ok("S2k 全程在用标注仍在（两次导入 + 两次回滚都不动它）",
   cnt("torrent_sections", "dict_id = %s" % d_id) == "1")
ok("S2l 分类学不碰素材表", cnt("medals") == BASE["medals"]
   and cnt("avatar_frames") == BASE["frames"],
   "%s/%s" % (cnt("medals"), cnt("avatar_frames")))
# ============ S3 theme：两态同闸 + 回滚复原 ============
BADK = "announce_interval"
tp_bad = pack("theme", "bd.theme.bad.%d" % TAG,
              {"settings": {"site_tagline": "BD 探针标语", BADK: "1"}})
s, r = imp(tp_bad, False)
errs = blob(r)
ok("S3a 预检报白名单外键（%s）" % BADK,
   s == 200 and "theme" in errs and BADK in errs, "%s %s" % (s, errs[:140]))
s, r = imp(tp_bad, True)
ok("S3b 确认侧同一道闸：400 + 键名，绝不落库",
   s == 400 and BADK in blob(r) and sval(BADK) == BASE["announce"]
   and sval("site_tagline") == BASE["tagline"],
   "%s %s" % (s, blob(r)[:140]))

tp_ok = pack("theme", "bd.theme.ok.%d" % TAG,
             {"settings": {"site_tagline": "BD 探针标语"}})
s, r = imp(tp_ok, True)
rowt = last_row_for("bd.theme.ok.%d" % TAG)
ok("S3c 合法 theme 包装上：site_tagline 已改，登记 1 行",
   s == 200 and sval("site_tagline") == "BD 探针标语" and rowt > 0,
   "%s %s" % (s, blob(r)[:120]))
s, r = roll(rowt)
ok("S3d theme 回滚回到导入前值（且登记行清除）",
   s == 200 and sval("site_tagline") == BASE["tagline"]
   and cnt("content_packs", "id = %d" % rowt) == "0",
   "%s now=%r %s" % (s, sval("site_tagline"), blob(r)[:150]))
tp2 = pack("theme", "bd.theme.color.%d" % TAG,
           {"settings": {"theme_token_sky": "#123456"}})
s, r = imp(tp2, True)
rowc2 = last_row_for("bd.theme.color.%d" % TAG)
ok("S3e 品牌色单独装包可用（theme_token_sky=#123456）",
   s == 200 and sval("theme_token_sky") == "#123456",
   "%s %s" % (s, blob(r)[:140]))
s, r = roll(rowc2)
ok("S3f 回滚到「未设置」的空值：不得被 color 校验反锁（装得上滚不回）",
   s == 200 and sval("theme_token_sky") == ""
   and not cnt("content_packs", "id = %d" % rowc2),
   "%s now=%r %s" % (s, sval("theme_token_sky"), blob(r)[:170]))
# ============ S4 rules：白名单 / changed 口径 / rule-try 只读 ============
rp_bad = pack("rules", "bd.rules.bad.%d" % TAG,
              {"rules": {"rule_bank_term_rate": "term_days * 0.002",
                         "rule_bd_evil": "1"}})
s1, r1 = imp(rp_bad, False)
s2, r2 = imp(rp_bad, True)
ok("S4a 非白名单规则键：预检与确认两态都 400 并点名该键",
   s1 == 400 and s2 == 400 and "rule_bd_evil" in blob(r1)
   and "rule_bd_evil" in blob(r2),
   "%s %s / %s %s" % (s1, blob(r1)[:90], s2, blob(r2)[:90]))
ok("S4b 被拒的键没有落库", sval("rule_bd_evil") == "")

rp_same = pack("rules", "bd.rules.same.%d" % TAG,
               {"rules": {"rule_bank_term_rate": BASE["rule"]}})
s, r = imp(rp_same, False)
d = data(r)
s2, r2 = imp(rp_same, True)
d2 = data(r2)
rowsame = last_row_for("bd.rules.same.%d" % TAG)
ok("S4c 值不变的包：will_change=%s 与 changed=%s 同口径（不虚报）"
   % (d.get("will_change"), d2.get("changed")),
   s == 200 and d.get("will_change") == 0 and s2 == 200
   and (d2.get("changed") or []) == []
   and psql("SELECT value FROM site_settings "
            "WHERE name = 'rule_bank_term_rate'") == BASE["rule"],
   "%s %s / %s %s" % (s, blob(r)[:90], s2, blob(r2)[:90]))
s, r = roll(rowsame)
ok("S4d 空包也登记、也能滚（滚完键值不变、登记行清除）",
   s == 200 and sval("rule_bank_term_rate") == BASE["rule"]
   and cnt("content_packs", "id = %d" % rowsame) == "0",
   "%s %s" % (s, blob(r)[:120]))
NEWEXPR = "term_days * 0.0031"
rp_set = pack("rules", "bd.rules.set.%d" % TAG,
              {"rules": {"rule_bank_term_rate": NEWEXPR}})
s, r = imp(rp_set, True)
d = data(r)
rowr = last_row_for("bd.rules.set.%d" % TAG)
ok("S4e 规则包装上：site_settings 值改变且 changed 点名该键",
   s == 200 and d.get("changed") == ["rule_bank_term_rate"]
   and sval("rule_bank_term_rate") == NEWEXPR,
   "%s %s" % (s, blob(r)[:140]))
s, r = call("POST", "/admin/content-packs/rule-try",
            {"key": "rule_bank_term_rate", "expr": "term_days * 0.0005",
             "vars": {"term_days": 90}}, token=ROOT)
d = data(r)
ok("S4f rule-try 求值（90 天 × 0.0005 = 0.045）与域区间回显 %s"
   % d.get("domain"),
   s == 200 and abs((d.get("value") or 0) - 0.045) < 1e-9
   and d.get("vars") == ["term_days"], "%s %s" % (s, blob(r)[:140]))
s, r = call("POST", "/admin/content-packs/rule-try",
            {"key": "rule_bank_term_rate", "expr": "term_days",
             "vars": {"term_days": 90}}, token=ROOT)
ok("S4f2 越界表达式被夹到域上限（90 → 1.0），不返回荒谬利率",
   s == 200 and abs((data(r).get("value") or 0) - 1.0) < 1e-9,
   "%s %s" % (s, blob(r)[:140]))
ok("S4g rule-try 只读：没把表达式写进 site_settings",
   sval("rule_bank_term_rate") == NEWEXPR)
s, r = call("POST", "/admin/content-packs/rule-try",
            {"key": "rule_bd_evil", "expr": "1", "vars": {}}, token=ROOT)
ok("S4h rule-try 也吃白名单：未知键 400", s == 400, "%s %s" % (s, blob(r)[:90]))
s, r = roll(rowr)
ok("S4i rules 回滚：键值回到导入前（%r）" % BASE["rule"],
   s == 200 and sval("rule_bank_term_rate") == BASE["rule"]
   and cnt("content_packs") == BASE["packs"],
   "%s now=%r %s" % (s, sval("rule_bank_term_rate"), blob(r)[:120]))

EB = BASE["embed_nb"]
# 收尾兜底：包侧回滚若坏了（本轮就在测它），探针规则按「跑前 id 集」清干净
EB_IDS = psql("SELECT coalesce(string_agg(id::text, ','), '0') "
              "FROM video_embed_rules WHERE NOT builtin")
CLEANUP.append("DELETE FROM video_embed_rules WHERE NOT builtin "
               "AND id NOT IN (%s)" % EB_IDS)
EID = 90000 + TAG % 900
GOOD = {"id": EID, "provider": "bdprobe", "name_zh": "BD 探针内嵌",
        "url_pattern": "^https://bdprobe.example.com/v/([0-9]{6,20})$",
        "embed_template": "https://bdprobe.example.com/embed/$1",
        "embed_origin": "https://bdprobe.example.com/",
        "render_kind": "iframe", "aspect": "16:9", "extra_params": None,
        "enabled": True, "sort": 901}

def epack(rows, pid):
    return pack("embed", pid, {"rules": rows})  # 内嵌包只有一张表


MUT = [
    # (标签, 改哪个键, 改成什么（"-" = 删掉该键）, 期望报错里含)
    ("未锚定开放正则", "url_pattern", ".*", "锚定"),
    ("开放捕获", "url_pattern", "^https://x.example.com/(.*)$", "开放捕获"),
    ("双反斜杠（P-55 形态）", "url_pattern",
     "^https://bdprobe\\\\.example\\\\.com/v/([0-9]{6,20})$", "反斜杠"),
    ("render_kind 越界", "render_kind", "object", "render_kind"),
    ("aspect 越界", "aspect", "21:9", "aspect"),
    ("模板落在 embed_origin 之外", "embed_template",
     "https://evil.example.com/$1", "embed_origin"),
    ("缺 url_pattern（旧写法铸空串）", "url_pattern", "-", "url_pattern"),
    ("缺有效 id（旧写法静默跳行）", "id", "-", "有效 id"),
    ("覆写 builtin 规则", "id", 1, "内置"),
]
bad = []
for label, key, val, kw in MUT:
    row = dict(GOOD)
    if val == "-":
        row.pop(key, None)
    else:
        row[key] = val
    pk = epack([row], "bd.embed.bad.%d" % TAG)
    two = (("预检", imp(pk, False)), ("确认", imp(pk, True)))
    for state, res in two:
        if res[0] != 400 or kw not in blob(res[1]):
            bad.append("%s %s=%s %s"
                       % (label, state, res[0], blob(res[1])[:48]))
ok("S5a 九道形状/安全闸：预检与确认两态都 400 并点名原因（%d 组）"
   % len(MUT), not bad, blob(bad)[:260])
ok("S5b 被拒的包一条规则也没写进去（非 builtin 仍 %s 行、内置未动）" % EB,
   cnt("video_embed_rules", "NOT builtin") == EB
   and cnt("video_embed_rules", "id = 1 AND provider = 'bilibili'") == "1"
   and cnt("video_embed_rules", "builtin") == "5")
s, r = imp(epack([GOOD], "bd.embed.ok.%d" % TAG), True)
rowe = last_row_for("bd.embed.ok.%d" % TAG)
d = data(r)
ok("S5c 合法 embed 包装上：applied=%s、行入库、登记 kind=embed"
   % (d.get("applied")),
   s == 200 and (d.get("applied") or {}).get("video_embed_rules") == 1
   and cnt("video_embed_rules", "id = %d" % EID) == "1" and rowe > 0
   and psql("SELECT kind FROM content_packs WHERE id = %d" % rowe) == "embed",
   "%s %s" % (s, blob(r)[:160]))
pub = json.dumps(call("GET", "/forums/embed-rules", token=ROOT)[1],
                 ensure_ascii=False)
ok("S5d 渲染层真的看到这条规则（/forums/embed-rules 含 bdprobe）",
   "bdprobe" in pub, pub[:140])
# 快照拍全：改名改序后装第二个包，回滚不得抹平在用行
psql_body = dict(GOOD)
psql_body.update(name_zh="BD 改名后的框", sort=9, aspect="4:3")
s, r = call("PUT", "/admin/embed-rules/%d" % EID, psql_body, token=ROOT)
ok("S5e 界面侧改名改序可用（PUT %s）" % s, s == 200, blob(r)[:100])
row2 = dict(GOOD)
row2["id"] = EID + 1
s, r = imp(epack([psql_body, row2], "bd.embed.ok2.%d" % TAG), True)
rowe2 = last_row_for("bd.embed.ok2.%d" % TAG)
ok("S5f 再装含两条的 embed 包（90051 改名值 + 新行）",
   s == 200 and (data(r).get("applied") or {}).get("video_embed_rules") == 2
   and cnt("video_embed_rules", "id = %d" % (EID + 1)) == "1",
   "%s %s" % (s, blob(r)[:120]))
s, r = roll(rowe2)
cols = psql("SELECT name_zh, sort, aspect, render_kind FROM video_embed_rules "
            "WHERE id = %d" % EID).split("|")
ok("S5g 回滚后 90051 的展示名/排序/比例原样存活（快照漏列会被抹平）：%s"
   % cols,
   s == 200 and cols[0] == "BD 改名后的框" and cols[1] == "9"
   and cols[2] == "4:3"
   and cnt("video_embed_rules", "id = %d" % (EID + 1)) == "0",
   "%s %s" % (s, blob(r)[:120]))
s, r = roll(rowe)
ok("S5h embed 包可回滚（原先没有 embed 分支，报「未知包类别」）"
   "：非 builtin 回到 %s 行" % EB,
   s == 200 and cnt("video_embed_rules", "NOT builtin") == EB
   and not cnt("video_embed_rules", "id = %d" % EID) == "1"
   and cnt("content_packs") == BASE["packs"], "%s %s" % (s, blob(r)[:120]))
# ============ S6 assets：包没声明的表不碰 + 净化 + 范围校验 ============
s, r = install("builtin.assets.season-wafu", True)
rowa = last_row_for("builtin.assets.season-wafu")
ok("S6a 商店一键装「四季·和风头像框」（只带 avatar_frames 的包）"
   "：勋章表 14 件一件不少（旧写法整表清空）",
   s == 200 and cnt("medals") == BASE["medals"] and rowa > 0,
   "%s medals=%s base=%s %s" % (s, cnt("medals"), BASE["medals"],
                                blob(r)[:100]))
C201 = psql("SELECT css FROM avatar_frames WHERE id = 201")
ok("S6b 包内四款框入库，且框样式过了后台白名单（越界声明被滤掉）",
   cnt("avatar_frames", "id BETWEEN 201 AND 204") == "4"
   and "border-color" in C201, C201[:80])
s, r = roll(rowa)
ok("S6c assets 回滚把包外被删的框全部还原（frames 回到 %s）" % BASE["frames"],
   s == 200 and cnt("avatar_frames") == BASE["frames"]
   and cnt("avatar_frames", "id = 201") == "0"
   and cnt("medals") == BASE["medals"], "%s" % blob(r)[:100])

s, r = call("GET", "/admin/content-packs/export?kind=assets", token=ROOT)
ax = data(r)
tables = (ax.get("payload") or {}).get("tables") or {}
ok("S6d 导出素材包（两张表都在）：%s 勋章 / %s 框"
   % (len(tables.get("medals") or []), len(tables.get("avatar_frames") or [])),
   s == 200 and len(tables.get("medals") or []) == int(BASE["medals"])
   and len(tables.get("avatar_frames") or []) == int(BASE["frames"]),
   "%s %s" % (s, blob(r)[:100]))
EVIL = {"id": 90061, "name": "BD 探针框", "price": 10, "sort": 1,
        "css": "position:fixed;top:0;width:100vw;background:"
               "url(javascript:alert(1)); border-color: #ff0011",
        "image_url": "javascript:alert(2)"}
fr = list(tables.get("avatar_frames") or []) + [EVIL]
ax["pack_id"] = "bd.assets.css.%d" % TAG
ax["payload"]["tables"]["avatar_frames"] = fr
s, r = imp(ax, True)
rowc = last_row_for("bd.assets.css.%d" % TAG)
css = psql("SELECT css FROM avatar_frames WHERE id = 90061")
img = psql("SELECT coalesce(image_url,'NULL') FROM avatar_frames "
           "WHERE id = 90061")
ok("S6e 框样式只留白名单声明、图片链接非 http(s) 归一为 NULL：%r / %s"
   % (css[:40], img),
   s == 200 and css == "border-color: #ff0011;" and img == "NULL"
   and cnt("medals") == BASE["medals"], "%s css=%r" % (s, css[:40]))
s, r = roll(rowc)
ok("S6f 净化包回滚：探针框清除、原七框未被连带改动",
   s == 200 and cnt("avatar_frames", "id = 90061") == "0"
   and cnt("avatar_frames") == BASE["frames"], "%s" % blob(r)[:100])

md = list(tables.get("medals") or [])
md[0] = dict(md[0])
md[0]["price"] = -5
neg = pack("assets", "bd.assets.neg.%d" % TAG,
           {"tables": {"medals": md, "avatar_frames":
                       tables.get("avatar_frames") or []}})
a = imp(neg, False)
b = imp(neg, True)
ok("S6g 勋章负价格：预检与确认两态都 400 且点名原因",
   a[0] == 400 and "价格" in blob(a[1]) and b[0] == 400
   and "价格" in blob(b[1]), blob(b[1])[:90])
ok("S6h 被拒的素材包没有动任何一行（medals/frames 均基线）",
   cnt("medals") == BASE["medals"] and cnt("avatar_frames") == BASE["frames"]
   and cnt("content_packs") == BASE["packs"],
   "%s/%s/%s" % (cnt("medals"), cnt("avatar_frames"), cnt("content_packs")))
# ============ S7 收尾：残留与审计 ============
for stmt in CLEANUP:
    psql(stmt)
now_tag = psql("SELECT value FROM site_settings WHERE name = 'site_tagline'")
now_rule = psql("SELECT value FROM site_settings "
                "WHERE name = 'rule_bank_term_rate'")
NOWCNT = dict((k, cnt(t)) for k, t in [
    ("cats", "categories"), ("dict", "section_dict"), ("medals", "medals"),
    ("frames", "avatar_frames"), ("embed", "video_embed_rules"),
    ("packs", "content_packs")])
ok("S7a 六张表与三个设置键全部回到基线 %s" % NOWCNT,
   all(NOWCNT[k] == BASE[k] for k in NOWCNT)
   and now_tag == BASE["tagline"] and now_rule == BASE["rule"]
   and sval("announce_interval") == BASE["announce"], str(NOWCNT))
ok("S7b 探针标注已删、探针号已墓碑（%s）" % MEMBER,
   cnt("torrent_sections", "dict_id = %s" % d_id) == "0"
   and (not MEMBER or psql("SELECT status FROM users WHERE username = '%s'"
                           % MEMBER) == "2"))
aud = psql("SELECT count(*) FROM audit_log WHERE action LIKE "
           "'content_pack:import' OR action LIKE 'content_pack:rollback'")
ok("S7c 导入/回滚审计留痕（只增不删，本轮 %s 条）" % aud,
   int(aud or 0) >= 12, "aud=%s" % aud)
summary()
