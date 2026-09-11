# -*- coding: utf-8 -*-
"""第五轮 P2 e2e：置顶促销 / 自定义菜单 / 消息模板（UTF-8 body，规避 curl 在 GBK 控制台下的编码问题）"""
import json
import sys
import urllib.request

BASE = "http://127.0.0.1:8080/api/v1"
results = []

def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    data = json.dumps(body).encode("utf-8") if body is not None else None
    with urllib.request.urlopen(req, data, timeout=10) as r:
        return json.loads(r.read())

def check(name, cond, detail=""):
    results.append((name, cond, detail))
    print(("PASS " if cond else "FAIL ") + name + (" " + detail if detail and not cond else ""))

r = call("POST", "/auth/login", {"username": "root", "password": "password123"})
tok = r["data"]["token"]

# 1. 新增置顶促销
r = call("POST", "/admin/sticky-promos", {"title": "e2e开站促销", "url": "/torrents?official=1", "badge": "活动"}, tok)
promo_id = r["data"]["id"]
check("P2·置顶促销新增", r["code"] == 0 and promo_id > 0, str(r))

# 2. 前台生效列表（时间窗内）
r = call("GET", "/sticky-promos")
titles = [x["title"] for x in r["data"]]
check("P2·置顶促销前台可见", "e2e开站促销" in titles, str(titles))

# 3. 停用后前台不可见
r = call("PUT", f"/admin/sticky-promos/{promo_id}", {"title": "e2e开站促销", "enabled": False}, tok)
r = call("GET", "/sticky-promos")
titles2 = [x["title"] for x in r["data"]]
check("P2·置顶促销停用生效", "e2e开站促销" not in titles2, str(titles2))

# 4. 新增菜单项
r = call("POST", "/admin/menu-items", {"location": "sidebar", "label": "E2E外部链接", "url": "https://example.com"}, tok)
menu_id = r["data"]["id"]
check("P2·自定义菜单新增", r["code"] == 0 and menu_id > 0, str(r))

# 5. 前台菜单可见 + 位置过滤
r = call("GET", "/menu-items?location=sidebar")
labels = [x["label"] for x in r["data"]]
check("P2·菜单前台可见", "E2E外部链接" in labels, str(labels))
r = call("GET", "/menu-items?location=footer")
check("P2·菜单位置过滤", all(x["location"] == "footer" for x in r["data"]))

# 6. 非法位置拒绝
try:
    call("GET", "/menu-items?location=hack")
    check("P2·菜单非法位置拒绝", False)
except urllib.error.HTTPError:
    check("P2·菜单非法位置拒绝", True)

# 7. 消息模板列表（4 条预置）
r = call("GET", "/admin/message-templates", None, tok)
keys = [x["scene_key"] for x in r["data"]]
check("P2·消息模板预置4条", set(keys) >= {"review_reject", "review_approve", "hr_warn", "invite_grant"}, str(keys))

# 8. 模板预览变量替换
r = call("POST", "/admin/message-templates/preview",
            {"scene_key": "review_reject", "vars": {"username": "张三", "torrent_name": "测试种子", "reason": "重复发布"}}, tok)
d = r["data"]
ok_sub = "张三" in d["subject"] or "种子" in d["subject"]
ok_body = "张三" in d["body"] and "测试种子" in d["body"] and "重复发布" in d["body"] and "{{" not in d["body"]
check("P2·模板预览占位符替换", r["code"] == 0 and ok_body, str(d))

# 9. 模板编辑保存
r = call("PUT", "/admin/message-templates/1", {"subject": "【改】审核未通过"}, tok)
r = call("GET", "/admin/message-templates", None, tok)
t1 = [x for x in r["data"] if x["id"] == 1][0]
check("P2·模板编辑保存", t1["subject"] == "【改】审核未通过", t1["subject"])
# 还原
call("PUT", "/admin/message-templates/1", {"subject": "您的种子未通过审核"}, tok)

# 10. 权限：无 token 访问被拒
try:
    call("GET", "/admin/sticky-promos")
    check("P2·管理端鉴权", False)
except urllib.error.HTTPError as e:
    check("P2·管理端鉴权", e.code in (401, 403))

# 清理测试数据
call("DELETE", f"/admin/sticky-promos/{promo_id}", None, tok)
call("DELETE", f"/admin/menu-items/{menu_id}", None, tok)
r = call("GET", "/sticky-promos")
check("P2·促销清理复原", len(r["data"]) == 0)
r = call("GET", "/menu-items?location=sidebar")
check("P2·菜单清理复原", "E2E外部链接" not in [x["label"] for x in r["data"]])

n_pass = sum(1 for _, c, _ in results if c)
print(f"\n===== P2 e2e 总结：{n_pass}/{len(results)} PASS =====")
sys.exit(0 if n_pass == len(results) else 1)
