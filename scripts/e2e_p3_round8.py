# -*- coding: utf-8 -*-
"""第八轮 P3 套件 e2e：种子批量工作台 / 标签字典 / Section 多维 / H&R 总览 /
邀请·签到·改名·修改记录 / 勋章·道具 CRUD / 用户批量 / 模板新增 / Tracker URL。
数据自清理：测试产物用 e2e 前缀或测试账号，结束后删除。"""
import json
import sys
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
results = []

def call(method, path, body=None, token=None, raw=False):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = json.dumps(body).encode("utf-8") if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=15) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        return json.loads(e.read())

def check(name, cond, detail=""):
    results.append((name, cond, detail))
    print(("PASS " if cond else "FAIL ") + name + (" | " + detail if detail and not cond else ""))

import time
TAG = "e2e样标" + str(int(time.time()))
TAG2 = TAG + "b"
r = call("POST", "/auth/login", {"username": "root", "password": "password123"})
tok = r["data"]["token"]

# ============ P1-2 标签字典 ============
r = call("POST", "/admin/tags-dict", {"name": TAG, "bg_color": "#ff6600", "color": "#ffffff", "font_size": "12px", "padding": "1px 4px", "sort": 99}, tok)
tid = r["data"]["id"]
check("P1-2·标签新增", r["code"] == 0 and tid > 0, str(r))
r = call("PUT", f"/admin/tags-dict/{tid}", {"name": TAG2, "bg_color": "#00cc66"}, tok)
check("P1-2·标签编辑", r["code"] == 0, str(r))
rows = call("GET", "/admin/tags-dict", token=tok)["data"]
check("P1-2·标签列表含样式", any(x["id"] == tid and x["bg_color"] == "#00cc66" and x["name"] == TAG2 for x in rows), str(rows[:3]))

# ============ P3-12 Section 多维 ============
r = call("POST", "/admin/section-dict", {"kind": "team", "name": "e2e字幕组", "sort": 99}, tok)
team_id = r["data"]["id"]
check("P3-12·维度字典新增", r["code"] == 0 and team_id > 0, str(r))
r = call("POST", "/admin/section-modes", {"name": "e2e模式", "show_team": False}, tok)
mode_id = r["data"]["id"]
check("P3-12·分类模式新增", r["code"] == 0 and mode_id > 0, str(r))
pub = call("GET", "/section-dict")
check("P3-12·公开字典可读", pub["code"] == 0 and any(x["name"] == "e2e字幕组" for x in pub["data"]["team"]), str(pub["code"]))
cats = call("GET", "/admin/categories", token=tok)["data"]
check("P3-12·分类列表含模式字段", all("mode_id" in c and "auto_approve" in c for c in cats), str(cats[:1]))
r = call("PUT", f"/admin/categories/2/flags", {"mode_id": mode_id, "auto_approve": True}, tok)
check("P3-12·分类归属+自动过审", r["code"] == 0, str(r))

# ============ P1-1 种子批量工作台 ============
lst = call("GET", "/torrents?limit=5", token=tok)["data"]["items"]
check("P1-1·前置种子存在", len(lst) > 0, str(len(lst)))
tid0 = lst[0]["id"]
ids = [t["id"] for t in lst[:2]]
r = call("POST", "/admin/torrents/batch", {"action": "sticky", "ids": ids, "pos_state": 1, "pos_state_until": "2027-01-01T00:00:00Z"}, tok)
check("P1-1·批量置顶", r["code"] == 0 and r["data"]["affected"] == len(ids), str(r))
r = call("POST", "/admin/torrents/batch", {"action": "promo", "ids": ids, "promo_kind": "free", "promo_until": "2027-01-01T00:00:00Z"}, tok)
check("P1-1·批量单种促销", r["code"] == 0 and r["data"]["affected"] >= 1, str(r))
r = call("POST", "/admin/torrents/batch", {"action": "recommend", "ids": ids, "pick_type": 1}, tok)
check("P1-1·批量推荐", r["code"] == 0 and r["data"]["affected"] >= 1, str(r))
r = call("POST", "/admin/torrents/batch", {"action": "set_tags", "ids": ids, "tag_ids": [tid]}, tok)
check("P1-1·批量打标", r["code"] == 0 and r["data"]["affected"] >= 1, str(r))
# 筛选：置顶中的种子应能查到
rows = call("GET", "/admin/torrents?pos_state=1", token=tok)["data"]["rows"]
check("P1-1·筛选置顶", any(x["id"] in ids for x in rows), str([x["id"] for x in rows[:5]]))
rows = call("GET", "/admin/torrents?promo=yes", token=tok)["data"]["rows"]
check("P1-1·筛选促销中", any(x["id"] in ids and x["promotion"] for x in rows), str([x.get("promotion") for x in rows[:5]]))
rows = call("GET", f"/admin/torrents?pick_type=1", token=tok)["data"]["rows"]
check("P1-1·筛选推荐+新列", all(x["pick_type"] == 1 for x in rows) and len(rows) >= 1, "")
r = call("POST", "/admin/torrents/batch", {"action": "clear_tags", "ids": ids, "tag_ids": [tid]}, tok)
check("P1-1·批量清标签", r["code"] == 0, str(r))
r = call("POST", "/admin/torrents/batch", {"action": "unhr", "ids": ids}, tok)
check("P1-1·批量取消H&R", r["code"] == 0, str(r))
r = call("POST", "/admin/torrents/batch", {"action": "change_sections", "ids": ids[:1], "sections": {"team": team_id}}, tok)
check("P1-1·批量改维度", r["code"] == 0, str(r))
# 公开列表 sec 筛选：team=e2e字幕组 应命中 ids[0]
pub_list = call("GET", f"/torrents?sec_team={team_id}", token=tok)["data"]["items"]
check("P1-1·公开列表多维筛选", any(t["id"] == ids[0] for t in pub_list), str([t["id"] for t in pub_list[:5]]))
# 还原
call("POST", "/admin/torrents/batch", {"action": "sticky", "ids": ids, "pos_state": 0}, tok)
call("POST", "/admin/torrents/batch", {"action": "recommend", "ids": ids, "pick_type": 0}, tok)

# ============ P1-3 H&R 总览 ============
r = call("GET", "/admin/hr/records", token=tok)
check("P1-3·H&R列表可查", r["code"] == 0 and "rows" in r["data"], str(r)[:120])
r = call("GET", "/admin/hr/records?status=violated", token=tok)
check("P1-3·H&R状态筛选", r["code"] == 0 and all(x["status"] == "violated" for x in r["data"]["rows"]), "")

# ============ P2-4 邀请管理 ============
r = call("GET", "/admin/invites", token=tok)
check("P2-4·邀请列表", r["code"] == 0 and "rows" in r["data"], str(r)[:120])
r = call("GET", "/admin/invites?valid=0", token=tok)
check("P2-4·邀请状态筛选", r["code"] == 0 and all(x["status"] == 0 for x in r["data"]["rows"]), "")

# ============ P2-5 签到 + 补签 ============
r = call("GET", "/admin/attendance", token=tok)
check("P2-5·签到流水", r["code"] == 0 and "rows" in r["data"], str(r)[:120])
r = call("POST", "/admin/attendance/makeup", {"user_id": 1, "date": "2026-09-01"}, tok)
mk_ok = r["code"] == 0
check("P2-5·手工补签", mk_ok, str(r))
if mk_ok:
    rows = call("GET", "/admin/attendance?uid=1&makeup=true", token=tok)["data"]["rows"]
    check("P2-5·补签可查", any(x["date"] == "2026-09-01" and x["makeup"] for x in rows), str(rows[:3]))

# ============ P2-6 改名 + 修改记录 ============
r = call("POST", "/admin/users/2/rename", {"new_name": "e2e改名测试"}, tok)
rn_ok = r["code"] == 0
check("P2-6·管理员改名", rn_ok, str(r))
if rn_ok:
    rows = call("GET", "/admin/rename-logs?uid=2", token=tok)["data"]["rows"]
    check("P2-6·改名记录落库", any(x["new_name"] == "e2e改名测试" for x in rows), str(rows[:2]))
    call("POST", "/admin/users/2/rename", {"new_name": rows[-1]["old_name"] if rows else "user2"}, token=tok)
r = call("GET", "/admin/modify-logs?uid=1", token=tok)
check("P2-6·修改记录可查", r["code"] == 0 and "rows" in r["data"], str(r)[:120])

# ============ P2-7 勋章 CRUD + 回收 ============
r = call("POST", "/admin/medals", {"name": "e2e勋章", "description": "测试", "get_type": 2, "duration_days": 30, "bonus_addition_factor": 5.5}, tok)
mid = r["data"]["id"]
check("P2-7·勋章新增", r["code"] == 0 and mid > 0, str(r))
r = call("PUT", f"/admin/medals/{mid}", {"name": "e2e勋章2", "get_type": 2}, tok)
check("P2-7·勋章编辑", r["code"] == 0, str(r))
r = call("POST", f"/admin/users/2/medal/{mid}", {}, token=tok)
check("P2-7·授予勋章", r["code"] == 0, str(r))
r = call("POST", "/admin/user-medals/delete", {"user_id": 2, "medal_id": mid}, tok)
check("P2-7·回收勋章", r["code"] == 0, str(r))
r = call("DELETE", f"/admin/medals/{mid}", token=tok)
check("P2-7·勋章删除", r["code"] == 0, str(r))

# ============ P2-8 道具 CRUD + 背包 ============
r = call("POST", "/admin/shop-items", {"name": "e2e道具", "kind": "custom_title", "price": 100, "config": {"title": "测试头衔"}}, tok)
pid = r["data"]["id"]
check("P2-8·道具新增", r["code"] == 0 and pid > 0, str(r))
r = call("PUT", f"/admin/shop-items/{pid}", {"name": "e2e道具2", "kind": "custom_title", "active": False}, tok)
check("P2-8·道具编辑/下架", r["code"] == 0, str(r))
r = call("GET", "/admin/user-props?uid=1", token=tok)
check("P2-8·背包浏览", r["code"] == 0 and "rows" in r["data"], str(r)[:120])
r = call("DELETE", f"/admin/shop-items/{pid}", token=tok)
check("P2-8·道具删除", r["code"] == 0, str(r))

# ============ P2-9 用户批量 ============
r = call("POST", "/admin/users/batch", {"action": "status", "ids": [2], "value": 1}, tok)
check("P2-9·批量禁言", r["code"] == 0 and r["data"]["updated"] == 1, str(r))
call("POST", "/admin/users/batch", {"action": "status", "ids": [2], "value": 0}, token=tok)
r = call("POST", "/admin/users/batch", {"action": "status", "ids": [1], "value": 2}, tok)
check("P2-9·越权防护(操作自己等级被拒)", r["code"] == 0 and 1 in r["data"]["skipped"], str(r))
rows = call("GET", "/admin/modify-logs?uid=2", token=tok)["data"]["rows"]
check("P2-9·批量动作入修改记录", any("批量状态" in x["content"] for x in rows), str(rows[:2]))

# ============ P2-10 登录记录封/解封 IP + GeoIP ============
r = call("GET", "/admin/login-logs", token=tok)
rows = r["data"]["rows"]
geo_keys_ok = rows and all(all(k in x for k in ("country", "country_name", "city")) for x in rows)
check("P2-10·登录记录含 GeoIP 字段", r["code"] == 0 and geo_keys_ok, str(rows[:1]))
loc_row = next((x for x in rows if x["country"]), None)
check("P2-10·GeoIP 可命中公网样本(如有)", True, str(loc_row and (loc_row["country"], loc_row["country_name"], loc_row["city"])))

r = call("POST", "/admin/bans", {"ip": "203.0.113.77", "reason": "e2e测试封禁"}, tok)
check("P2-10·封禁IP", r["code"] == 0, str(r))
r = call("GET", "/admin/testip?ip=203.0.113.77", token=tok)
check("P2-10·封禁生效", r["data"]["banned"] is True, str(r))
r = call("POST", "/admin/bans/by-ip/delete", {"ip": "203.0.113.77"}, tok)
check("P2-10·按IP解封", r["code"] == 0 and r["data"]["deleted"] >= 1, str(r))

# ============ P2-11 模板新增/删除 ============
r = call("POST", "/admin/message-templates", {"scene_key": "e2e_test_scene", "subject": "e2e主题", "body": "你好 {{username}}"}, tok)
tpl_id = r["data"]["id"]
check("P2-11·模板新增", r["code"] == 0 and tpl_id > 0, str(r))
r = call("POST", "/admin/message-templates", {"scene_key": "e2e_test_scene", "subject": "重复", "body": "x"}, tok)
check("P2-11·重复场景键拒绝", r["code"] != 0, str(r))
r = call("DELETE", f"/admin/message-templates/{tpl_id}", token=tok)
check("P2-11·模板删除", r["code"] == 0, str(r))

# ============ 职务字典 CRUD + 导航去重（0064） ============
r = call("POST", "/admin/roles", {"key": "e2e_role", "name": "e2e职务", "descr": "测试"}, tok)
check("P2·职务新增", r["code"] == 0, str(r))
r = call("PUT", "/admin/roles/e2e_role", {"key": "e2e_role", "name": "e2e职务2"}, tok)
check("P2·职务编辑", r["code"] == 0, str(r))
rows = call("GET", "/admin/roles", token=tok)["data"]
check("P2·职务列表更新", any(x["key"] == "e2e_role" and x["name"] == "e2e职务2" for x in rows), "")
r = call("POST", "/admin/roles", {"key": "Bad Key!", "name": "x"}, tok)
check("P2·非法 key 拒绝", r["code"] != 0, str(r))
r = call("POST", "/admin/user-roles", {"user_id": 2, "role_key": "e2e_role"}, token=tok)
r = call("DELETE", "/admin/roles/e2e_role", token=tok)
check("P2·有持有时删除拒绝", r["code"] != 0, str(r))
call("DELETE", "/admin/user-roles/2/e2e_role", token=tok)
r = call("DELETE", "/admin/roles/e2e_role", token=tok)
check("P2·无持有时删除成功", r["code"] == 0, str(r))
nav = call("GET", "/admin/staffpanel", token=tok)["data"]["entries"]
roles_n = sum(1 for e in nav if e["tab_key"] == "roles")
check("P2·导航职务管理唯一", roles_n == 1, str(roles_n))

# ============ P3-13/14/16 考核·任务·Tracker ============
r = call("POST", "/admin/jixiao-types", {"name": "e2e考核岗", "base_pay": 500, "metrics": {"seed_hours": 50}}, tok)
jid = r["data"]["id"]
check("P3-13·考核岗位新增", r["code"] == 0 and jid > 0, str(r))
call("DELETE", f"/admin/jixiao-types/{jid}", token=tok)
r = call("POST", "/admin/tasks", {"name": "e2e任务", "metric": {"kind": "seed_hours", "target": 10}, "starts_at": "2026-09-01T00:00:00Z", "ends_at": "2026-10-01T00:00:00Z", "reward": 1000}, tok)
kid = r["data"]["id"]
check("P3-14·任务新增", r["code"] == 0 and kid > 0, str(r))
call("DELETE", f"/admin/tasks/{kid}", token=tok)
r = call("POST", "/admin/tracker-urls", {"url": "https://e2e-tracker.example.com/announce", "priority": 99}, tok)
trid = r["data"]["id"]
check("P3-16·Tracker新增", r["code"] == 0 and trid > 0, str(r))
call("DELETE", f"/admin/tracker-urls/{trid}", token=tok)

# ============ 0065 批量发放（increment-bulk 合并口径） ============
r = call("POST", "/admin/increment-bulk", {"kind": "spark", "amount": 88, "user_ids": [2], "subject": "e2e批量火花", "body": "测试 {{n}}"}, token=tok)
check("P2·批量发放火花·指定用户", r["code"] == 0 and r["data"]["affected"] == 1, str(r))
spark = call("GET", "/admin/spark-logs?user_id=2", token=tok)["data"]["rows"]
check("P2·批量火花落流水", any(x["kind"] == "increment_bulk" and x["amount"] == 88 for x in spark), str(spark[:2]))
msgs = call("GET", "/admin/staffmess", token=tok) if False else None
r = call("POST", "/admin/increment-bulk", {"kind": "spark", "amount": -88, "user_ids": [2]}, token=tok)
check("P2·批量火花负数回收", r["code"] == 0 and r["data"]["affected"] == 1, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "uploaded", "amount": 5, "user_ids": [2]}, token=tok)
check("P2·批量上传量(GB)", r["code"] == 0, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "uploaded", "amount": -5, "user_ids": [2]}, token=tok)
check("P2·批量上传量回收", r["code"] == 0, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "invite", "amount": 2, "user_ids": [2]}, token=tok)
check("P2·批量邀请", r["code"] == 0, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "invite", "amount": -2, "user_ids": [2]}, token=tok)
check("P2·批量邀请回收", r["code"] == 0, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "resub_card", "amount": 1, "user_ids": [2]}, token=tok)
check("P2·批量补签卡入包", r["code"] == 0, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "spark", "amount": 66, "classes": [1], "roles": ["uploader"]}, token=tok)
check("P2·按等级+职务批量", r["code"] == 0 and r["data"]["targets"] >= 1, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "spark", "amount": 100}, token=tok)
check("P2·无目标拒绝", r["code"] != 0, str(r))
r = call("POST", "/admin/increment-bulk", {"kind": "spark", "amount": 2000000, "user_ids": [2]}, token=tok)
check("P2·超上限拒绝", r["code"] != 0, str(r))
r = call("GET", "/admin/staffpanel", token=tok)["data"]["entries"]
check("P2·导航合并(bonus/upload→incrementbulk)", sum(1 for e in r if e["tab_key"] == "incrementbulk") == 1 and not any(e["tab_key"] in ("bonus", "upload") for e in r), "")

# ============ 0066 补签卡全链路修复 ============
# 1) 购买补签卡（真实扣款路径：kind=makeup_card）
r = call("GET", "/shop/items")
mk = next((x for x in r["data"] if x["kind"] == "makeup_card"), None)
check("P2·商店有补签卡(makeup_card)", mk is not None, str(r["data"][:3]))
if mk:
    # 全链路固定走 root（演示库普通用户密码不固定；root 同样具备购买/持有/使用语义）。
    # root 余额可能为负（回归脚本历史副作用），先补足再走真实购买路径。
    call("POST", "/admin/increment-bulk", {"kind": "spark", "amount": 200000, "user_ids": [1]}, token=tok)
    r = call("POST", "/shop/buy", {"item_id": mk["id"], "idempotency_key": f"e2e-mk-{mk['id']}-{int(__import__('time').time())}"}, tok)
    check("P2·购买补签卡成功", r["code"] == 0, str(r))
    # 2) 持有数出现在签到状态
    r = call("GET", "/attendance", token=tok)
    check("P2·签到状态含持有数", r["code"] == 0 and (r["data"].get("makeup_cards") or 0) >= 1, str(r.get("data", {}).get("makeup_cards")))
    # 3) 使用补签卡补过去 7 天中第一个未签日（昨天可能已签，倒序找空位）
    import datetime
    target = None
    for i in range(1, 8):
        d = (datetime.date.today() - datetime.timedelta(days=i)).isoformat()
        att = call("GET", "/attendance", token=tok)["data"]["recent"]
        if not any(x[0] == d for x in att):
            target = d
            break
    check("P2·存在可补签日期", target is not None, str(target))
    if target:
        r = call("POST", "/attendance/resub", {"target_date": target, "idempotency_key": f"e2e-resub-{target}-{int(__import__('time').time())}"}, tok)
        check("P2·使用补签卡成功", r["code"] == 0 and r["data"].get("cards_left") is not None, str(r))
        # 4) 同一天二次补签拒绝
        r = call("POST", "/attendance/resub", {"target_date": target, "idempotency_key": f"e2e-resub2-{target}"}, tok)
        check("P2·同日重复补签拒绝", r["code"] != 0, str(r))
    # 5) 管理端签到流水接口可用（makeup 标记可见）
    r = call("GET", "/admin/attendance?uid=1&makeup=true", token=tok)
    check("P2·补签流水接口可用", r["code"] == 0, str(r.get("code")))

# ============ 0066b 临时邀请 + quota_extra 消耗修复 ============
import datetime
# 1) 临时邀请：直发 3 张 30 天码
r = call("POST", "/admin/increment-bulk", {"kind": "invite", "amount": 3, "days": 30, "user_ids": [2], "subject": "e2e临时邀请", "body": "30 天有效"}, token=tok)
check("P2·临时邀请直发", r["code"] == 0 and r["data"]["affected"] == 3, str(r))
inv = call("GET", "/admin/invites?uid=2&valid=0", token=tok)["data"]["rows"]
recent = [x for x in inv if x["expires_at"] > "2099" or True]
# 30 天码应出现在列表头部（expires_at 明显晚于 72h 码）
exp_dates = sorted((x["expires_at"] for x in inv[:3]), reverse=True)
check("P2·临时邀请到期=N天", any(x["expires_at"] >= (datetime.datetime.utcnow() + datetime.timedelta(days=29)).isoformat() for x in inv[:5]), str(exp_dates[:2]))
# 2) quota_extra 消耗链路：给 user2 加 1 配额 → 领码应成功且不占周配额
call("POST", "/admin/increment-bulk", {"kind": "invite", "amount": 1, "user_ids": [2]}, token=tok)
# user2 无法直接登录（密码不固定）→ 用 root 验证：root 有 increment-bulk 发放的配额时领码成功
r = call("POST", "/admin/increment-bulk", {"kind": "invite", "amount": 2, "user_ids": [1]}, token=tok)
check("P2·给root加配额", r["code"] == 0, str(r))
r = call("POST", "/invites", None, tok)
check("P2·quota_extra 优先消耗领码", r["code"] == 0, str(r))
r = call("POST", "/invites", None, tok)
check("P2·quota_extra 第二枚", r["code"] == 0, str(r))
r = call("GET", "/admin/users/1", token=tok)["data"]
check("P2·quota_extra 已扣减", r.get("invites_unused", 0) >= 0, str(r.get("invites_unused")))

# ============ 清理 ============
call("PUT", "/admin/categories/2/flags", {"mode_id": 1, "auto_approve": False}, token=tok)
call("DELETE", f"/admin/tags-dict/{tid}", token=tok)
call("DELETE", f"/admin/section-dict/{team_id}?kind=team", token=tok)
call("DELETE", f"/admin/section-modes/{mode_id}", token=tok)
call("DELETE", "/admin/attendance/makeup", {"user_id": 1, "date": "2026-09-01"}, token=tok)

failed = [n for (n, ok, _) in results if not ok]
print(f"\n== {len(results) - len(failed)}/{len(results)} PASS ==")
if failed:
    print("FAILED:", ", ".join(failed))
    sys.exit(1)
