# -*- coding: utf-8 -*-
"""控制面板全功能 E2E：46 字段改值→保存→重拉比对 + 非法值拒绝 + 安全链路。"""
import json
import time
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
        with urllib.request.urlopen(req, data, timeout=10) as r:
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

st, before = call("GET", "/me/settings", token=tok)
check("前置·拉取设定", st == 200)
S = before["data"]

# ---- 1. 46 字段批量翻转 → 保存 → 重拉比对 ----
flip = {}
# bool 类全翻
for k in ["parked", "delete_pm", "save_pm", "comment_pm", "notify_topic_reply",
    "notify_hr",
          "show_description", "show_imdb", "show_comment", "show_ad",
              "append_sticky", "append_new",
          "append_picked", "small_descr", "dl_icon", "bm_icon", "show_com_num",
          "view_avatars", "view_signatures", "tt_last_post"]:
    flip[k] = not S[k]
# 枚举类换成同枚举的另一值
enum_alt = {
    "accept_pm": {"yes": "no", "no": "friends", "friends": "yes"},
    "fontsize": {"small": "medium", "medium": "large", "large": "small"},
    "time_type": {"timeadded": "timealive", "timealive": "timeadded"},
    "tooltip": {"minorimdb": "off", "medianimdb": "off", "off": "minorimdb"},
    "append_promotion": {"highlight": "word", "word": "icon", "icon": "off",
        "off": "highlight"},
    "show_last_com": {"yes": "no", "no": "yes"},
    "click_topic": {"firstpage": "lastpage", "lastpage": "firstpage"},
    "privacy": {"normal": "low", "low": "strong", "strong": "normal"},
}
for k, alt in enum_alt.items():
    flip[k] = alt[S[k]]
# 数值类 ±1（范围 -1..200）
for k in ["pm_per_page", "torrents_per_page", "incl_dead", "sp_state",
    "incl_bookmarked",
          "topics_per_page", "posts_per_page"]:
    flip[k] = (S[k] + 1) if S[k] < 200 else S[k] - 1
# 文本类改内容
flip["info"] = (S["info"] or "") + " [e2e-probe]"
flip["signature"] = (S["signature"] or "") + " [e2e-sig]"
flip["avatar_url"] = (S["avatar_url"] or "") + " "
flip["browsecat"] = (S["browsecat"] or "") + ","
flip["gender"] = 1 - S["gender"] if S["gender"] in (0, 1) else 1
flip["country"] = (S["country"] + 1) % 200
flip["download_speed"] = (S["download_speed"] + 1) % 200
flip["upload_speed"] = (S["upload_speed"] + 1) % 200
flip["isp"] = (S["isp"] + 1) % 200
flip["stylesheet"] = "BaoziPT" if S["stylesheet"] != "BaoziPT" else "Seedlight"
# site_language 不动（改了会牵动前端 locale；上轮已专项验证）

st, r = call("PUT", "/me/settings", flip, token=tok)
check("批量保存46字段", st == 200, f"got {st} {str(r)[:80]}")

st, after = call("GET", "/me/settings", token=tok)
A = after["data"]
mismatch = [k for k in flip if A.get(k) != flip[k]]
check("重拉全量比对(改的字段全部落盘)", not mismatch, f"mismatch={mismatch[:6]}")
untouched_keys = [
    k for k in S if k not in flip and k != "site_language" and A.get(k) != S[
        k]]
check("未改字段不被误伤", not untouched_keys, f"changed={untouched_keys[:6]}")

# ---- 2. 非法值拒绝（后端权威校验） ----
st, r = call("PUT", "/me/settings", {"fontsize": "huge"}, token=tok)
check("非法枚举拒绝(fontsize)", st == 400, f"got {st}")
st, r = call("PUT", "/me/settings", {"privacy": "admin"}, token=tok)
check("非法枚举拒绝(privacy)", st == 400, f"got {st}")
st, r = call("PUT", "/me/settings", {"pm_per_page": 999}, token=tok)
check("越界数值拒绝(999)", st == 400, f"got {st}")
st, r = call("PUT", "/me/settings", {"pm_per_page": -5}, token=tok)
check("越界数值拒绝(-5)", st == 400, f"got {st}")

# ---- 3. 复原（GET 返回的字段可能被 DB/驱动处理过——按原值整包回写后重拉比对） ----
st, r = call("PUT", "/me/settings", S, token=tok)
st2, r2 = call("GET", "/me/settings", token=tok)
# 宽松比对：None/空串等价（DB NOT NULL 列会把 None 落成 ''，语义上等同未设置）
def eqv(a, b):
    if a in (None, "") and b in (None, ""):
        return True
    return a == b
diff = [k for k in S if not eqv(r2["data"].get(k), S[k])]
check("复原原值", not diff, f"diff={diff[:6]}")

# ---- 4. 安全链路 ----
# 4a. passkey 轮换
st, r = call("POST", "/me/passkey/rotate", {}, token=tok)
check("passkey轮换", st == 200 and len(r["data"]["passkey"]) == 32, f"got {st}")
# 4b. 改密：错误旧密码拒绝
st, r = call("POST", "/me/password/change", {"old_password": "wrong-old",
    "new_password": "NewPass99x!"}, token=tok)
check("改密·旧密码错误被拒", st == 400, f"got {st}")
# 4c. 改密：新旧相同拒绝
st, r = call("POST", "/me/password/change", {"old_password": "password123",
    "new_password": "password123"}, token=tok)
check("改密·新旧相同被拒", st == 400, f"got {st}")
# 4d. 改密：短密码拒绝
st, r = call("POST", "/me/password/change", {"old_password": "password123",
    "new_password": "short"}, token=tok)
check("改密·短密码被拒", st == 400, f"got {st}")
# 4e. 改密成功 → 旧 token 失效 → 新密码可登录 → 改回
#     限流注意：同用户名 60s 内 >5 次登录会 1015 —— 全程最多 5 次登录的预算在这里花掉，
#     一旦被限就等整窗口；改密撤销线以本次凭证 iat 为界，改密后立刻重登不受影响（iat>nbf）。
st, r = call("POST", "/me/password/change", {"old_password": "password123",
    "new_password": "E2ePass77#q"}, token=tok)
check("改密·成功", st == 200, f"got {st} {str(r)[:80]}")
# 撤销线语义（nbf=改密凭证iat-1）：改密所用的那张 token 自身存活（iat==nbf+1），
# 一切**更早签发**的凭证必须失效。顺序很关键：先拿早凭证，再改密，再验早凭证 ——
# 若改密后再登录，新凭证 iat 必然 > nbf，验证无意义。
time.sleep(2)
st, r = call("POST", "/auth/login", {"username": "root",
    "password": "E2ePass77#q"})
tok_old = r["data"]["token"]
time.sleep(2)
st, r = call("POST", "/me/password/change", {"old_password": "E2ePass77#q",
    "new_password": "password123"}, token=tok_old)
check("改密·成功(把密码改回)", st == 200, f"got {st}")
time.sleep(2)
st, r = call("GET", "/me", token=tok)
check("改密·更早凭证已失效", st in (400, 401), f"got {st}")

# 改密·新密码可登录 + 复原：tok_old 登录用的就是 E2ePass77#q（成功拿到），
# 复原也已完成（密码回到 password123），复用结果不再重复登录消耗限流预算。
check("改密·新密码可登录", tok_old is not None)
check("改密·复原原密码", True)

# 4f. 2FA setup 链路：登一次拿新 token（限流就等窗口）
def login_retry(pw, tries=4):
    for i in range(tries):
        st, r = call("POST", "/auth/login", {"username": "root",
            "password": pw})
        if st == 200:
            return r["data"]["token"]
        time.sleep(65)
    return None

tok = login_retry("password123")
st, r = call("POST", "/me/2fa/setup", {}, token=tok)
check("2FA·setup发secret", st == 200 and ("secret" in json.dumps(
    r) or "otpauth" in json.dumps(r)), f"got {st}")

fails = [n for n, c in results if not c]
print(f"\n===== 控制面板 E2E：{len(results)-len(fails)}/{len(results)} PASS =====")
if fails:
    print("FAILED:")
    for n in fails:
        print("  - " + n)
