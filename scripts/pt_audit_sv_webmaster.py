# -*- coding: utf-8 -*-
"""闸门 SV：站长画像·治理配置面（2026-10-25 七轮）。

覆盖 terms 治理链（键/状态机/版本/回灌）、staffpanel-entries CRUD、
custom-pages、user-fields、roles、reg-quiz、webhook egress ops。

  P-16（P1 已修）：POST /admin/staffpanel-entries 的 INSERT 不写
        NOT NULL 列 panel（0052 迁移后保留做兼容），新建面板条目
        整口 500——管理界面「面板条目编辑器」的新建按钮全灭。
        本闸门 SV3 建-查-改-删四步会把回归钉死。

  冷启动注意：site_terms 为空表时，早期 term_versions 的快照全是
  `{"terms":[]}`——restore 到这些版本会把表清空（快照语义本身如此，
  非缺陷）；判据只用本轮自建的版本。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_sv_webmaster.py
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

ROOT = login()
TAG = "sv%d" % (int(time.time()) % 100000)

# ============ S1 terms 治理链 ============
K = "%s.term" % TAG
s, r = call("POST", "/admin/terms/keys", token=ROOT,
            body={"key": K, "display_zh": "闸门键", "category": "system"})
ok("SV1 terms 键登记", s == 200 and r.get("code") == 0, str(r)[:100])

s, r = call("POST", "/admin/terms/keys/status", token=ROOT,
            body={"key": K, "status": "retired"})
ok("SV2 非法迁移 active→retired 被拦", s == 400, "s=%s" % s)

s, r = call("POST", "/admin/terms/keys/status", token=ROOT,
            body={"key": K, "status": "deprecated"})
ok("SV3 合法迁移 active→deprecated", s == 200, "s=%s" % s)

s, r = call("POST", "/admin/terms/keys/status", token=ROOT,
            body={"key": K, "status": "retired"})
ok("SV4 deprecated→retired（清理）", s == 200, "s=%s" % s)

s, r = call("POST", "/admin/terms/audit", token=ROOT)
ok("SV5 巡检落报告", s == 200 and "report_id" in str(r), "s=%s" % s)

# 版本链：词条 CRUD 触发快照；restore 回灌后词条应回到改动前形态
C, R1, R2 = "%s.a" % TAG, "闸门甲", "闸门乙"
call("POST", "/admin/terms", token=ROOT,
     body={"canonical": C, "replacement": R1, "sort": 9990})
call("POST", "/admin/terms", token=ROOT,
     body={"canonical": C, "replacement": R2, "sort": 9990})
s, r = call("GET", "/admin/terms/versions", token=ROOT)
cur = r["data"]["current_version"]
ok("SV6 词条改动产生版本", isinstance(cur, int) and cur >= 1, "cur=%s" % cur)
s, r = call("POST", "/admin/terms/versions/%s/restore" % cur, token=ROOT)
ok("SV7 restore 留档+回灌", s == 200 and "saved_current_as" in str(r),
   str(r)[:120])
s, r = call("GET", "/admin/terms", token=ROOT)
rows = {x["canonical"]: x["replacement"] for x in r["data"]}
ok("SV8 回灌后词条=改动前形态", rows.get(C) == R1, str(rows))
call("POST", "/admin/terms/%s/delete" % C, token=ROOT, body={"confirm": True})
s, r = call("GET", "/admin/terms", token=ROOT)
ok("SV9 词条清理", not any(x["canonical"] == C for x in r["data"]), "")

# ============ S2 staffpanel-entries（P-16 回归棘轮） ============
s, r = call("POST", "/admin/staffpanel-entries", token=ROOT,
            body={"section": "ops", "name": "闸门条目%s" % TAG,
                  "url": "/admin?tool=%s" % TAG, "info": "sv",
                  "sort": 9990, "min_class": 99})
ok("SV10 建面板条目（P-16：panel NOT NULL）", s == 200 and r.get("code") == 0,
   str(r)[:120])
eid = r.get("data", {}).get("id") if s == 200 else None
if eid:
    s, r = call("PUT", "/admin/staffpanel-entries/%s" % eid, token=ROOT,
                body={"section": "ops", "name": "闸门条目改",
                      "url": "/admin?tool=%s" % TAG, "info": "sv",
                      "sort": 9990, "min_class": 99,
                      "perm_key": "settings.manage"})
    ok("SV11 更新面板条目", s == 200, str(r)[:100])
    s, r = call("POST", "/admin/staffpanel-entries/delete", token=ROOT,
                body={"id": eid})
    ok("SV12 删面板条目", s == 200, str(r)[:100])
    s, r = call("POST", "/admin/staffpanel-entries/delete", token=ROOT,
                body={"id": eid})
    ok("SV13 再删 404 不假成功", s == 404, "s=%s" % s)
else:
    ok("SV10b 拿到 id", False, "建口未返回 id")

# ============ S3 custom-pages / user-fields / roles ============
s, r = call("POST", "/admin/custom-pages", token=ROOT,
            body={"slug": "%s-page" % TAG, "title": "闸门页",
                  "body": "# sv", "visible": True, "sort": 9990})
ok("SV14 建自定义页", s == 200, str(r)[:100])
pid = r.get("data", {}).get("id") if s == 200 else None
s, r = call("GET", "/pages/%s-page" % TAG)
ok("SV15 前台匿名可读", s == 200, "s=%s" % s)
s, r = call("POST", "/admin/custom-pages/%s/delete" % pid, token=ROOT,
            body={"confirm": True})
ok("SV16 删页", s == 200, str(r)[:100])

fld = {"label": "闸门字段", "type": "text", "required": False,
       "visibility": "private", "show_on_register": False,
       "options": [], "sort": 9990, "enabled": True}
s, r = call("POST", "/admin/user-fields/%s_f" % TAG, token=ROOT, body=fld)
ok("SV17 建自定义字段", s == 200, str(r)[:100])
s, r = call("POST", "/admin/user-fields/%s_f/delete" % TAG, token=ROOT,
            body={})
ok("SV18 删自定义字段", s == 200, str(r)[:100])

s, r = call("POST", "/admin/roles", token=ROOT,
            body={"key": "%s_r" % TAG, "name": "闸门职务"})
ok("SV19 建职务", s == 200, str(r)[:100])
s, r = call("DELETE", "/admin/roles/%s_r" % TAG, token=ROOT)
ok("SV20 删职务", s == 200, str(r)[:100])

# ============ S4 reg-quiz / webhook egress ============
s, r = call("POST", "/admin/reg-quiz", token=ROOT,
            body={"category": "account", "question": "闸门题干：探针是否落库？",
                  "options": ["是", "否"], "answer_index": 0,
                  "enabled": False})
ok("SV21 建注册题", s == 200, str(r)[:100])
qid = r.get("data", {}).get("id") if s == 200 else None
if qid:
    s, r = call("POST", "/admin/reg-quiz/delete", token=ROOT,
                body={"ids": [qid]})
    ok("SV22 删注册题", s == 200 and r.get("data", {}).get("deleted") == 1,
       str(r)[:100])
else:
    ok("SV21b 拿到题 id", False, str(r)[:100])

s, r = call("POST", "/admin/webhooks/egress", token=ROOT,
            body={"cidr": "203.0.113.0/24", "note": "sv gate"})
ok("SV23 加 webhook 出网段", s == 200, str(r)[:100])
s, r = call("POST", "/admin/webhooks/egress/delete", token=ROOT,
            body={"cidr": "203.0.113.0/24"})
ok("SV24 删 webhook 出网段", s == 200 and r.get("data", {}).get("deleted") == 1,
   str(r)[:100])

summary()
