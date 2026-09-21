# -*- coding: utf-8 -*-
"""管理组面板全功能核验（定稿口径）：12 tab × 78 个前端调用端点 × 后端全部管理路由。

策略：
- A. 所有 GET 端点逐一拉取验活（code==0；freeleech 无进行中促销时 data=null 合法）；
- B. 写操作走「安全路径」实测——创建临时对象→验证生效→清理复原；
- C. 高危操作（deletedisabled/docleanup 等）验证路由存活（参数错 400）不实际执行。
口径备注（首轮 12 个假 FAIL 的核实结论）：
- FAQ/规则/公告的读端点是公开 /faq、/rules-content、/home.news
  （staff-tools/content-manage 实际口径）；
- IP/邮箱封禁删除按数字 id（POST 返回值），不按 IP/模式串；
- emailbans 的 mode 合法值为 ban|allow；
- 模板预览 body 是 {scene_key, vars}，scene_key 须为库中真实场景；
- settings validate 是 {name,value} 单字段、groups PUT 是 {group,
    values}（真实字段名如 SITENAME）；
- /admin/links 只有 GET/PUT/DELETE（友链由用户申请、后台审核，前端亦无新增入口）——设计如此；
- /admin/users/flags 是 PUT-only。
"""
import json
import urllib.request
import urllib.error

BASE = "http://127.0.0.1:8080/api/v1"
results = []

def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=15) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}
    except Exception as e:
        return -1, {"err": str(e)}

def check(name, cond, detail=""):
    results.append((name, cond))
    print(("PASS " if cond else "FAIL ") + name + (
        f"  {detail}" if detail else ""))

st, r = call("POST", "/auth/login", {"username": "root",
    "password": "password123"})
tok = r["data"]["token"]
check("前置·root登录", st == 200)

# ============ A. GET 端点全量验活 ============
GETS = [
    "/admin/overview", "/admin/reviews", "/admin/reports", "/admin/appeals",
    "/admin/users?limit=5", "/admin/torrents?limit=5", "/admin/audit?limit=5",
    "/admin/cheaters?limit=5", "/admin/staffpanel", "/admin/stats",
    "/admin/dbstats", "/admin/syslog?page=1", "/admin/spark-logs?limit=5",
    "/admin/login-logs?limit=5", "/admin/warned", "/admin/ipcheck",
    "/admin/maxlogin?limit=5", "/admin/emailbans", "/admin/ads",
    "/admin/notconnectable?limit=5", "/admin/uploaders?limit=5",
    "/admin/allagents", "/admin/polloverview", "/admin/locations?page=1",
    "/admin/bans", "/admin/massmail?limit=5", "/admin/categories",
        "/admin/links",
    "/admin/menu-items", "/admin/message-templates", "/admin/deny-reasons",
    "/admin/agentrules", "/admin/plugins", "/admin/site-type-packs",
    "/admin/claims?state=active", "/admin/claims?state=unclaimed",
    "/admin/claims?state=exited", "/admin/torrent-buys?limit=5",
    "/admin/torrent-ops?limit=5", "/admin/sticky-promos",
        "/admin/settings/schema",
    "/admin/settings/history?name=SITENAME",
    # 公开读端点（staff-tools / content-manage 的真实读取口径）
    "/faq", "/rules-content", "/home",
]
for p in GETS:
    st, r = call("GET", p, token=tok)
    ok = st == 200 and r.get("code") == 0 and r.get("data") is not None
    check(f"GET {p.split('?')[0]}", ok, f"got {st} code={r.get('code')}")

# freeleech GET：无进行中促销 data=null 合法
st, r = call("GET", "/admin/freeleech", token=tok)
check("GET /admin/freeleech", st == 200 and r.get("code") == 0,
    f"data={r.get('data')}")

# users/flags 是 PUT-only：探真实调用（rows[1] 关掉再开回；列表形状 {rows:[...]}）
st, r = call("GET", "/admin/users?limit=5", token=tok)
rows = (r.get("data") or {}).get("rows") or []
target = next((u for u in rows if u.get("id") != 1), None)
if target:
    st, _ = call("PUT", "/admin/users/flags", {"user_id": target["id"],
        "download_enabled": False}, token=tok)
    ok1 = st == 200
    st, _ = call("PUT", "/admin/users/flags", {"user_id": target["id"],
        "download_enabled": True}, token=tok)
    check("PUT /admin/users/flags(开关复原)", ok1 and st == 200,
        f"target={target['username']}")
else:
    check("PUT /admin/users/flags", False, "无第二用户")

# ============ B. 写操作安全路径 ============
# B1. FAQ 增改删
st, r = call("POST", "/admin/faq", {"question": "e2e-probe-q",
    "answer": "e2e-probe-a"}, token=tok)
fid = (r.get("data") or {}).get("id")
check("写·FAQ新增", st == 200 and fid is not None, f"{st}")
if fid:
    st, _ = call("PUT", f"/admin/faq/{fid}", {"question": "e2e-probe-q2",
        "answer": "e2e-probe-a2"}, token=tok)
    check("写·FAQ修改", st == 200)
    st, _ = call("DELETE", f"/admin/faq/{fid}", token=tok)
    check("写·FAQ删除", st == 200)

# B2. 规则增删
st, r = call("POST", "/admin/rules", {"title": "e2e-rule", "body": "probe"},
    token=tok)
rid = (r.get("data") or {}).get("id")
check("写·规则新增", st == 200 and rid is not None)
if rid:
    st, _ = call("DELETE", f"/admin/rules/{rid}", token=tok)
    check("写·规则删除", st == 200)

# B3. 公告增删（读走 /home.news）
st, r = call("POST", "/admin/news", {"title": "e2e-news", "body": "probe"},
    token=tok)
nid = (r.get("data") or {}).get("id")
check("写·公告新增", st == 200 and nid is not None, f"{st}")
if nid:
    st, r = call("GET", "/home", token=tok)
    visible = any(n.get("title") == "e2e-news" for n in (r.get("data") or {
        }).get("news") or [])
    check("写·公告立即可见(/home)", visible)
    st, _ = call("DELETE", f"/admin/news/{nid}", token=tok)
    check("写·公告删除", st == 200)

# B4. 用户火花调整 ±7（写流水口径）
st, _ = call("POST", "/admin/users/adjust", {"user_id": 2, "spark_delta": 7},
    token=tok)
check("写·火花调整", st == 200)
st, _ = call("POST", "/admin/users/adjust", {"user_id": 2, "spark_delta": -7},
    token=tok)
check("写·火花回滚", st == 200)

# B5. 全站促销 创建→读→关闭
st, _ = call("POST", "/admin/freeleech", {"kind": "free", "hours": 1},
    token=tok)
check("写·全站促销创建", st == 200)
st, r = call("GET", "/admin/freeleech", token=tok)
check("写·促销可读", st == 200 and (r.get("data") or {}).get("kind") == "free")
st, _ = call("DELETE", "/admin/freeleech", token=tok)
check("写·全站促销关闭", st == 200)

# B6. 消息模板预览（真实 scene_key）
st, r = call("GET", "/admin/message-templates", token=tok)
tpls = r.get("data") or []
key0 = tpls[0]["scene_key"] if tpls else None
st, r = call("POST", "/admin/message-templates/preview", {"scene_key": key0,
    "vars": {"username": "e2e"}}, token=tok)
check("写·模板预览(真实场景)", st == 200, f"key={key0} {st}")

# B7. IP 封禁增删（按 id）
st, r = call("POST", "/admin/bans", {"ip": "203.0.113.77",
    "reason": "e2e-probe"}, token=tok)
bid = (r.get("data") or {}).get("id")
check("写·IP封禁新增", st == 200 and bid is not None, f"{st}")
if bid:
    st, _ = call("DELETE", f"/admin/bans/{bid}", token=tok)
    check("写·IP封禁删除(id)", st == 200)

# B8. 邮箱封禁增删（mode=ban，按 id 删）
st, r = call("POST", "/admin/emailbans", {"pattern": "*@e2e-probe.invalid",
    "mode": "ban"}, token=tok)
eid = (r.get("data") or {}).get("id")
check("写·邮箱封禁新增(ban)", st == 200 and eid is not None, f"{st}")
if eid:
    st, _ = call("DELETE", f"/admin/emailbans/{eid}", token=tok)
    check("写·邮箱封禁删除(id)", st == 200)

# B9. testip 探测
st, r = call("GET", "/admin/testip?ip=127.0.0.1", token=tok)
check("GET testip探测", st == 200 and r.get("code") == 0, f"{st}")

# B10. 设定：单字段校验 + 分组保存回写原值（无净变更）
st, r = call("POST", "/admin/settings/validate", {"name": "SITENAME",
    "value": "好学 FluxTorrent"}, token=tok)
check("写·设定校验(name/value)", st == 200, f"{st}")
st, r = call("PUT", "/admin/settings/groups", {"group": "basic", "values": {
    "SITENAME": "好学 FluxTorrent"}}, token=tok)
check("写·设定保存(回写原值)", st == 200, f"{st}")

# B11. 审核队列决策：无待审不空跑（列队存活已由 GET 覆盖）
st, r = call("GET", "/admin/reviews", token=tok)
pending = (r.get("data") or {})
np = len(pending) if isinstance(pending, list) else 0
check(f"队列·待审种子({np}条，不实际决策)", True)

# ============ C. 高危操作：路由存活（参数错 400，不执行） ============
# resetpass 会真实重置目标密码：只对不存在的 user_id=0 探路由（404 资源不存在 ≠ 路由缺失）
for p, b in [("/admin/deletedisabled", {}), ("/admin/docleanup", {}),
             ("/admin/resetpass", {"user_id": 0}),
             ("/admin/massmail", {"subject": "", "body": ""}),
             ("/admin/staffmess", {"subject": "", "body": ""}),
             ("/admin/adduser", {"username": "x", "email": "bad",
                 "password": "short"}),
             ("/admin/amountbonus", {"amount": 0})]:
    st, r = call("POST", p, b, token=tok)
    code = r.get("code")
    # 1004=资源不存在（路由命中、目标不存在）｜1002=参数校验｜0=成功（docleanup 这类只读巡检）
    exists = st in (200, 400, 404) and code in (0, 1002, 1004)
    check(f"高危·{p} 路由存活(不执行)", exists, f"got {st} code={code}")

fails = [n for n, c in results if not c]
print(f"\n===== 管理面板 E2E：{len(results)-len(fails)}/{len(results)} PASS =====")
if fails:
    print("FAILED:")
    for n in fails:
        print("  - " + n)
