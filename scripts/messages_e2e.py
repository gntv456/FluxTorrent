# -*- coding: utf-8 -*-
"""信箱全功能 E2E（对齐 NexusPHP messages.php + staffbox.php 口径）。

覆盖：接收限制矩阵 / 防刷 / 回复引用 / 转发 / 已读 / 双删逻辑 / 文件夹 CRUD+移动+连带清信 /
咨询提交-列表-答复回写-私信触达-批量标记-删除。
"""
import json
import time
import urllib.parse
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
    print(("PASS " if cond else "FAIL ") + name + (f"  {detail}" if detail else ""))

# 前置：root + 普通用户 A/B
st, r = call("POST", "/auth/login", {"username": "root", "password": "password123"})
root = r["data"]["token"]
check("前置·root登录", st == 200)
import time as _t
def ensure_user(name, email, password):
    """幂等建号：新建走 adduser；已存在（重跑）时改密路径走 /me/change-password
    无法用（不知道旧密码），改用 SQL 侧由 root 直接重置（演示环境约定）——
    这里通过 admin/resetpass 拿临时密码 → 登录 → 必改流程改回目标密码。"""
    st1, _ = call("POST", "/admin/adduser", {"username": name, "email": email, "password": password}, token=root)
    if st1 != 200:
        # 已存在：查 uid → admin/resetpass 生成临时密码（会置 must_reset）→ 改回
        uid = None
        st2, rows = call("GET", f"//admin/users?kw={name}", token=root)
        _ = st2
        # 用临时密码登录后走 change-password 改回目标密码
        import subprocess
        q = subprocess.run(
            ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent",
             "-tAc", f"SELECT id FROM users WHERE username='{name}'"],
            capture_output=True, text=True)
        uid = (q.stdout.strip() or "").split("\n")[0]
        if not uid:
            return None
        st3, r3 = call("POST", "/admin/resetpass", {"user_id": int(uid)}, token=root)
        if st3 != 200:
            return None
        temp = r3["data"]["temp_password"]
        for _ in range(5):
            st4, r4 = call("POST", "/auth/login", {"username": name, "password": temp})
            if st4 == 200:
                break
            _t.sleep(10)
        else:
            return None
        tok_tmp = r4["data"]["token"]
        call("POST", "/me/password/change",
             {"old_password": temp, "new_password": password}, token=tok_tmp)
    for _ in range(5):
        st5, r5 = call("POST", "/auth/login", {"username": name, "password": password})
        if st5 == 200:
            return r5["data"]["token"]
        _t.sleep(10)  # 登录限流退避
    return None
A = ensure_user("pma", "pma@t.local", "PmaPass123!")
check("前置·用户A", A is not None)
B = ensure_user("pmb", "pmb@t.local", "PmbPass123!")
check("前置·用户B", B is not None)

if A and B:
    # 1. 接收限制矩阵：B 设 accept_pm=no → A 发信被拒；root(staff) 发信放行
    st, _ = call("PUT", "/me/settings", {"accept_pm": "no"}, token=B)
    st, r = call("POST", "/messages", {"to": "pmb", "subject": "hi", "body": "x"}, token=A)
    check("限制·no拒收普通用户", st == 400, f"{st}")
    st, r = call("POST", "/messages", {"to": "pmb", "subject": "staff直发", "body": "x"}, token=root)
    check("限制·staff无视no", st == 200, f"{st}")
    # friends-only
    st, _ = call("PUT", "/me/settings", {"accept_pm": "friends"}, token=B)
    st, r = call("POST", "/messages", {"to": "pmb", "subject": "hi", "body": "x"}, token=A)
    check("限制·friends拒非好友", st == 400, f"{st}")
    st, _ = call("PUT", "/me/settings", {"accept_pm": "yes"}, token=B)

    # 2. 防刷：A 连发两条（60s 内第二条被拒）；root 不受限
    time.sleep(61)
    st, r = call("POST", "/messages", {"to": "pmb", "subject": "first", "body": "1st"}, token=A)
    check("防刷·第一条可发", st == 200, f"{st}")
    st, r = call("POST", "/messages", {"to": "pmb", "subject": "second", "body": "2nd"}, token=A)
    check("防刷·60s内第二条被拒", st == 400, f"{st}")
    st, r = call("POST", "/messages", {"to": "pmb", "subject": "s1", "body": "s"}, token=root)
    st2, r2 = call("POST", "/messages", {"to": "pmb", "subject": "s2", "body": "s"}, token=root)
    check("防刷·staff连发不受限", st == 200 and st2 == 200, f"{st},{st2}")

    # 3. 回复引用：B 回复 root 的信 → Re: 前缀 + 原文引用
    time.sleep(61)
    st, r = call("GET", "/messages/inbox", token=B)
    first = (r.get("data") or [{}])[0]
    st, r = call("POST", "/messages", {"to": "root", "subject": "ignored", "body": "my reply", "reply_to": first["id"]}, token=B)
    check("回复·成功", st == 200, f"{st}")
    st, r = call("GET", "/messages/inbox", token=root)
    rep = next((m for m in r["data"] if "Re:" in m["subject"]), None)
    check("回复·Re前缀+引用原文", rep is not None and "原信" in rep["body"], f"{rep and rep['subject']}")

    # 4. 转发：A 转发给 B
    time.sleep(61)
    st, r = call("POST", "/messages", {"to": "root", "subject": "给A的信", "body": "fwsrc"}, token=root)
    fw_id = (r.get("data") or {}).get("id") if isinstance(r.get("data"), dict) else None
    st, r = call("POST", "/messages", {"to": "pmb", "subject": "fw", "body": "", "forward_of": fw_id}, token=root)
    check("转发·成功", st == 200, f"{st}")

    # 5. 已读：B 收件箱有未读 → markread → unread=false
    st, r = call("GET", "/messages/inbox", token=B)
    unread_ids = [m["id"] for m in r["data"] if m.get("unread")]
    check("已读·存在未读", len(unread_ids) > 0, f"n={len(unread_ids)}")
    st, r = call("POST", "/messages/markread", {"ids": unread_ids}, token=B)
    check("已读·批量标记", st == 200, f"{st}")
    st, r = call("GET", "/messages/inbox?unread=true", token=B)
    check("已读·未读筛选为空", not (r.get("data") or []), f"n={len(r.get('data') or [])}")

    # 6. 搜索
    st, r = call("GET", "/messages/inbox?search=" + urllib.parse.quote("staff直发"), token=B)
    check("搜索·关键词命中", len(r.get("data") or []) >= 1, f"n={len(r.get('data') or [])}")

    # 7. 双删语义：B 删一封（收件方）→ 收件箱不见、root 发件箱仍在；root 再删 → 物理删
    st, r = call("GET", "/messages/inbox", token=B)
    victim = r["data"][0]
    st, _ = call("POST", "/messages/delete", {"ids": [victim["id"]]}, token=B)
    st, r = call("GET", "/messages/inbox", token=B)
    check("双删·收件方删后不可见", all(m["id"] != victim["id"] for m in r["data"]))
    st, r = call("GET", "/messages/sent", token=root)
    check("双删·发件方仍可见", any(m["id"] == victim["id"] for m in r["data"]))
    st, _ = call("POST", "/messages/delete", {"ids": [victim["id"]]}, token=root)
    st, r = call("GET", "/messages/sent", token=root)
    check("双删·双方删后物理消失", all(m["id"] != victim["id"] for m in r["data"]))

    # 8. 文件夹：先清残留（重复跑幂等）→ 建(≤3/名≤14) → 移动 → box 视图 → 改名 → 删除连带清信
    st, r = call("GET", "/messages/boxes", token=B)
    for b in r.get("data") or []:
        call("POST", "/messages/boxes", {"id": b["id"], "name": ""}, token=B)
    st, r = call("POST", "/messages/boxes", {"name": "重要"}, token=B)
    box_id = (r.get("data") or {}).get("id") if isinstance(r.get("data"), dict) else None
    check("文件夹·新建", st == 200 and box_id is not None, f"{st}")
    st, r = call("GET", "/messages/inbox", token=B)
    mv = r["data"][0]["id"] if r["data"] else None
    if mv:
        st, _ = call("POST", "/messages/move", {"ids": [mv], "folder": box_id}, token=B)
        check("文件夹·移动", st == 200, f"{st}")
        st, r = call("GET", f"/messages/inbox?box_id={box_id}", token=B)
        check("文件夹·box视图", any(m["id"] == mv for m in (r.get("data") or [])))
    st, r = call("POST", "/messages/boxes", {"id": box_id, "name": "改名了"}, token=B)
    check("文件夹·改名", st == 200, f"{st}")
    st, r = call("POST", "/messages/boxes", {"name": ""}, token=B) if False else (None, None)
    st, r = call("POST", "/messages/boxes", {"id": box_id, "name": ""}, token=B)
    check("文件夹·删除(清空名)", st == 200, f"{st}")
    st, r = call("GET", f"/messages/inbox?box_id={box_id}", token=B)
    check("文件夹·连带清信", not (r.get("data") or []), f"n={len(r.get('data') or [])}")
    # 上限 3 个
    ids = []
    for n in ["甲", "乙", "丙", "丁"]:
        st, r = call("POST", "/messages/boxes", {"name": n}, token=B)
        d = r.get("data")
        ids.append(d.get("id") if isinstance(d, dict) else None)
    check("文件夹·上限3个(第4被拒)", ids[3] is None, f"{ids}")

    # 9. 咨询工作台：A 提交 → root 列表 → 答复（回写+私信触达）→ 批量标记 → 删除
    time.sleep(61)
    st, r = call("POST", "/contactstaff", {"subject": "咨询标题", "body": "咨询正文"}, token=A)
    sm_id = (r.get("data") or {}).get("id") if isinstance(r.get("data"), dict) else None
    check("咨询·提交", st == 200 and sm_id is not None, f"{st}")
    st, r = call("GET", "/staffmessages?answered=0", token=root)
    found = any(m["id"] == sm_id for m in r.get("data") or [])
    check("咨询·staff列表可见", found)
    st, r = call("GET", "/staffmessages", token=A)
    check("咨询·普通用户不可见", st == 403, f"{st}")
    st, r = call("POST", "/staffmessages/answer", {"id": sm_id, "answer": "这是答复"}, token=root)
    check("咨询·答复成功", st == 200, f"{st}")
    st, r = call("GET", "/staffmessages?answered=1", token=root)
    done = next((m for m in r["data"] if m["id"] == sm_id), None)
    check("咨询·答复回写+答复人", done and done["answer"] == "这是答复" and done["answered_by"] == "root", f"{done and done['answered_by']}")
    st, r = call("GET", "/messages/inbox", token=A)
    hit = any(m["subject"].startswith("Re: 咨询标题") for m in (r.get("data") or []))
    check("咨询·答复私信触达", hit, f"subjects={[m['subject'] for m in (r.get('data') or [])][:4]}")
    # 批量标 + 删
    st, r = call("POST", "/contactstaff", {"subject": "批量1", "body": "b"}, token=root)  # staff 不防刷
    id2 = (r.get("data") or {}).get("id") if isinstance(r.get("data"), dict) else None
    st, r = call("POST", "/staffmessages/mark", {"ids": [id2]}, token=root)
    check("咨询·批量标记", st == 200, f"{st}")
    st, r = call("POST", "/staffmessages/delete", {"ids": [sm_id, id2]}, token=root)
    check("咨询·删除", st == 200, f"{st}")

fails = [n for n, c in results if not c]
print(f"\n===== 信箱 E2E：{len(results)-len(fails)}/{len(results)} PASS =====")
if fails:
    print("FAILED:")
    for n in fails:
        print("  - " + n)
