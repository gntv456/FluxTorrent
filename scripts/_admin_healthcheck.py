# -*- coding: utf-8 -*-
"""控制面板全量体检：GET 全测，写接口做可回滚冒烟测试。"""
import subprocess, json, time, base64, hmac, hashlib, sys

env = open("D:/FluxTorrent/docker/.env", encoding="utf-8").read()
secret = [l.split("=",1)[1].strip() for l in env.splitlines() if l.startswith("JWT_SECRET=")][0]
b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
now = int(time.time())
h = b64(json.dumps({"typ":"JWT","alg":"HS256"}).encode())
p = b64(json.dumps({"sub":1,"class_id":99,"exp":now+7200,"iat":now}).encode())
TOK = f"{h}.{p}." + b64(hmac.new(secret.encode(), f"{h}.{p}".encode(), hashlib.sha256).digest())

def api(method, path, body=None):
    cmd = ["curl","-s","--max-time","15","-X",method,f"http://localhost:8080/api/v1{path}",
           "-H",f"Authorization: Bearer {TOK}","-H","Content-Type: application/json"]
    if body is not None:
        cmd += ["-d", json.dumps(body, ensure_ascii=False)]
    out = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8").stdout or "{}"
    try:
        return json.loads(out)
    except Exception:
        return {"code":-1,"message":"non-json:"+out[:120],"data":None}

results = []
def check(name, method, path, body=None, expect=0):
    r = api(method, path, body)
    code = r.get("code")
    ok = (code == expect)
    n = ""
    d = r.get("data")
    if isinstance(d, list): n = f"rows={len(d)}"
    elif isinstance(d, dict): n = f"keys={len(d)}"
    results.append((ok, "GET " if method=="GET" else method+" ", path, f"code={code} {r.get('message','')[:60]} {n}"))
    return r

# ---------- 读取类：全部 ----------
print("== READ ==")
check("overview","GET","/admin/overview")
check("reviews","GET","/admin/reviews")
check("reports","GET","/admin/reports")
check("appeals","GET","/admin/appeals")
check("audit","GET","/admin/audit")
check("cheaters","GET","/admin/cheaters")
check("users","GET","/admin/users?page=1")
check("user-detail","GET","/admin/users/1")
check("torrents","GET","/admin/torrents?page=1")
check("deny-reasons","GET","/admin/deny-reasons")
check("torrent-ops","GET","/admin/torrent-ops?page=1")
check("sticky-promos","GET","/admin/sticky-promos")
check("menu-items","GET","/admin/menu-items")
check("msg-templates","GET","/admin/message-templates")
check("claims","GET","/admin/claims?page=1")
check("home(news)","GET","/home")
check("fun-items","GET","/fun/items?status=all")
check("links","GET","/admin/links")
check("faq","GET","/faq")
check("rules-content","GET","/rules-content")
check("bans","GET","/admin/bans")
check("massmail","GET","/admin/massmail")
check("categories","GET","/admin/categories")
check("freeleech","GET","/admin/freeleech")
check("warned","GET","/admin/warned")
check("ipcheck","GET","/admin/ipcheck")
check("maxlogin","GET","/admin/maxlogin")
check("emailbans","GET","/admin/emailbans")
check("stats","GET","/admin/stats")
check("ads","GET","/admin/ads")
check("type-packs","GET","/admin/site-type-packs")
check("notconnectable","GET","/admin/notconnectable")
check("uploaders","GET","/admin/uploaders")
check("allagents","GET","/admin/allagents")
check("polloverview","GET","/admin/polloverview")
check("dbstats","GET","/admin/dbstats")
check("syslog","GET","/admin/syslog?page=1&q=")
check("locations","GET","/admin/locations?page=1")
check("settings-schema","GET","/admin/settings/schema")
check("plugins","GET","/admin/plugins")

# ---------- 写入类：可回滚冒烟 ----------
print("== WRITE (smoke, rollback) ==")

# FAQ 增改删
r = check("faq-add","POST","/admin/faq",{"question":"__体检测试问__","answer":"临时"})
fid = (r.get("data") or {}).get("id")
if fid:
    check("faq-update","PUT",f"/admin/faq/{fid}",{"question":"__体检测试问2__","answer":"临时2"})
    check("faq-del","DELETE",f"/admin/faq/{fid}")

# 规则增删
r = check("rules-add","POST","/admin/rules",{"title":"__体检规则__","body":"临时"})
rid = (r.get("data") or {}).get("id")
if rid: check("rules-del","DELETE",f"/admin/rules/{rid}")

# 拒绝理由增改删
r = check("deny-add","POST","/admin/deny-reasons",{"reason":"__体检理由__","sort":99})
did = (r.get("data") or {}).get("id") or (r.get("data") or {}).get("reason_id")
if did:
    check("deny-update","PUT",f"/admin/deny-reasons/{did}",{"reason":"__体检理由2__","sort":98})
    check("deny-del","DELETE",f"/admin/deny-reasons/{did}")

# 置顶促销增删
r = check("promo-add","POST","/admin/sticky-promos",{"title":"__体检促销__","url":"/faq","badge":"测","starts_at":"2026-09-12T00:00:00Z","ends_at":"2026-09-13T00:00:00Z"})
pid = (r.get("data") or {}).get("id")
if pid: check("promo-del","DELETE",f"/admin/sticky-promos/{pid}")

# 广告增-开关-删
r = check("ads-add","POST","/admin/ads",{"title":"__体检广告__","html":"<b>t</b>","position":"sidebar"})
aid = (r.get("data") or {}).get("id")
if aid:
    check("ads-toggle","PUT",f"/admin/ads/{aid}/toggle")
    check("ads-del","DELETE",f"/admin/ads/{aid}")

# 全站促销设置+清除
check("freeleech-set","POST","/admin/freeleech",{"kind":"free","hours":1})
check("freeleech-clear","DELETE","/admin/freeleech")

# 客户端规则增删
r = check("agentrule-add","POST","/admin/agentrules",{"mode":"allow","pattern":"__体检UA__","note":None})
ar = (r.get("data") or {})
arid = ar.get("id") if isinstance(ar, dict) else None
if arid: check("agentrule-del","POST","/admin/agentrules/delete",{"id":arid})

# 分类增删
r = check("cat-add","POST","/admin/categories",{"name":"__体检分类__"})
cid = (r.get("data") or {}).get("id")
if cid: check("cat-del","DELETE",f"/admin/categories/{cid}")

# 邮件域 ban 增删
r = check("emailban-add","POST","/admin/emailbans",{"pattern":"__体检@例.test","mode":"deny","note":None})
eb = r.get("data") or {}
ebid = eb.get("id") if isinstance(eb, dict) else None
if ebid: check("emailban-del","DELETE",f"/admin/emailbans/{ebid}")

# 火花调整（root ±1 对冲）
check("amountbonus+","POST","/admin/amountbonus",{"user_id":1,"amount":1,"comment":"体检+"})
check("amountbonus-","POST","/admin/amountbonus",{"user_id":1,"amount":-1,"comment":"体检-"})

# 清缓存（安全）
check("clearcache","POST","/admin/clearcache")

print("\n== RESULT ==")
fails = 0
for ok, m, path, info in results:
    mark = "PASS" if ok else "FAIL"
    if not ok: fails += 1
    print(f"[{mark}] {m:6} {path:42} {info}")
print(f"\nTOTAL {len(results)} | FAIL {fails}")
