# -*- coding: utf-8 -*-
"""CS 闸门（批七十七 P-281）：后台说了中文，前台也得把那句中文演出来。

现网（修前，web:aa1011s + api:aa1024a）实测同一件事的两端：
  接口侧  GET /api/v1/torrents?date_from=2025-13-45
          → http=400 code=1002「参数校验失败: 日期格式无效：2025-13-45
            （要用 YYYY-MM-DD，且日历上得真有这一天）」
  页面侧  GET /torrents?date_from=2025-13-45（带 flux_token + flux.session）
          → http=200，可见文本里只有通用「加载失败」，
            既没有那句中文说明，也没有错误码 chip、没有 request_id
  ⇒ 批七十四/七十五刚把筛选参数收紧（未知值不再静默当「不筛」），
    如果界面不把「为什么被拒」演出来，收紧对用户就是「页面突然空了」——
    症状和 P-279 要治的那个洞一模一样。这条链不闭合，批七十五只算修了一半。

判据口径（三条自证的坑，写法按此避）：
  ① 只认「剥掉 <script>/<style> 之后的可见文本」。整页 HTML 里 i18n 字典
     blob 含 27 处「加载失败」，用裸 substring 判会把好页也判成坏页；
  ② 错误面板必须按类名认（ui-err__msg / ui-err__code / ui-err__rid），
     文案本身会变（本地化），类名是这套组件的契约；
  ③ 「失败」与「确实没有结果」要各判一条：CS5 用空搜索词控住空态，
     否则「一律渲染错误面板」也能全绿。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_cs_err_readable.py
"""
import atexit  # noqa: E402
import io  # noqa: E402
import os  # noqa: E402
import re  # noqa: E402
import subprocess  # noqa: E402
import sys  # noqa: E402
import time  # noqa: E402
import urllib.error  # noqa: E402
import urllib.request  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, summary  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.environ.get("FLUX_WEB_BASE", "http://127.0.0.1:3000")
ROOT = login()

# (查询串, 后端该说的中文关键词, 说明)
BAD = (
    ("date_from=2025-13-45", "日期格式无效", "日历上没有这一天"),
    ("size_min=abc", "体积无效", "非数字体积"),
    ("promo=zzz", "优惠档位无效", "未知促销档位"),
    ("anonymous=7", "匿名筛选取值无效", "越界枚举"),
)
ANTI = 'error={{ message: dict'
PAGES = ("apps/web/app/(main)/torrents/page.tsx",)
AUDIT_N = [0]


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", q], stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, universal_newlines=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s / %s" % (q[:44], (r.stderr or "").strip()[:70]))
    return (r.stdout or "").strip()


def vis(h):
    """剥掉 script/style 与标签后的可见文本（判据 ①）。"""
    h = re.sub(r"<(script|style)[^>]*>.*?</\1>", " ", h, flags=re.S)
    h = re.sub(r"<[^>]+>", " ", h)
    h = h.replace("&quot;", '"').replace("&#x27;", "'")
    return re.sub(r"\s+", " ", h).strip()


def page_once(qs, cookie=True, locale=None):
    ck = []
    if cookie:
        ck.append("flux_token=%s" % ROOT)
        ck.append("flux.session=1")
    if locale:
        ck.append("flux.locale=%s" % locale)
    req = urllib.request.Request(WEB + "/torrents" + qs)
    if ck:
        req.add_header("Cookie", "; ".join(ck))
    try:
        r = urllib.request.urlopen(req, timeout=60)
        return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")


def settled(v):
    """SSR 是否已落定：Next 忙时先回 loading.tsx 骨架壳（「正在获取列表」），
    那一版里既没有筛选区也没有错误面板，会被判成「没有面板」的假绿。"""
    return "正在获取列表" not in v and "跳到主要内容" in v


def page(qs, cookie=True, locale=None, tries=10):
    """取「落定」的那一版；取不到就把最后一次交回去并在细节里点名没落定。"""
    st, h = page_once(qs, cookie, locale)
    for _ in range(tries):
        if settled(vis(h)):
            break
        time.sleep(2.5)
        st, h = page_once(qs, cookie, locale)
    if not settled(vis(h)):
        print("   WARN 页面 %s 在 %d 次重试后仍是骨架壳（机器忙），"
              "本轮读数不可信" % (qs, tries))
    return st, h


def src(rel):
    return io.open(os.path.join(REPO, rel), encoding="utf-8",
                   errors="replace").read()


def sweep():
    """收尾自检：本闸门只读，审计流水不得因本轮新增。"""
    n = psql("SELECT count(*) FROM audit_log")
    if AUDIT_N[0] and n and n != AUDIT_N[0]:
        print("   WARN 审计流水条数变化 %s → %s（本闸门应为只读）"
              % (AUDIT_N[0], n))


atexit.register(sweep)
AUDIT_N[0] = psql("SELECT count(*) FROM audit_log")

print("== CS0 正控：接口侧四条中文 400 ==")
for qs, want, why in BAD:
    s, r = call("GET", "/torrents?" + qs, token=ROOT)
    msg = (r or {}).get("message", "") if isinstance(r, dict) else ""
    ok("CS0 %s（%s）" % (qs.split("=")[0], why),
       s == 400 and (r or {}).get("code") == 1002 and want in msg,
       "http=%s code=%s msg=%s" % (s, (r or {}).get("code"), str(msg)[:80]))

print("== CS0b 正控：合法筛选页不该报错 ==")
s, h = page("?alive=0&limit=3")
vb = vis(h)
# 前置条件必须先钉住：探针若被当匿名，整页是登录表单，「无错误面板」会假绿
ok("CS0b0 带 cookie 确实进了列表页（不是登录表单/骨架壳）",
   s == 200 and settled(vb) and "基础筛选" in vb and "用户登录" not in vb,
   "http=%s 落定=%s 含基础筛选=%s 含用户登录=%s"
   % (s, settled(vb), "基础筛选" in vb, "用户登录" in vb))
ok("CS0b 合法页无错误面板",
   s == 200 and "ui-err__msg" not in h,
   "http=%s ui-err=%d" % (s, h.count("ui-err")))

print("== CS1..CS4 渲染层：后端那句中文必须出现在可见文本 ==")
for i, (qs, want, why) in enumerate(BAD, start=1):
    s, h = page("?" + qs)
    v = vis(h)
    hit_msg = want in v
    hit_code = "ui-err__code" in h and "1002" in v
    hit_rid = "ui-err__rid" in h
    j = v.find("共")
    snip = (v[j:j + 60] if j >= 0 else v[:60]).replace(chr(10), " ")
    ok("CS%d ?%s 演出「%s」+码+号" % (i, qs.split("=")[0], want),
       s == 200 and hit_msg and hit_code and hit_rid,
       "http=%s 在列表页=%s 文案=%s 码chip=%s rid=%s｜%s"
       % (s, "基础筛选" in v, hit_msg, hit_code, hit_rid, snip))

print("== CS5 区分态：确实没有结果 ≠ 失败 ==")
s, h = page("?search=zzzqqxx%E4%B8%8D%E5%AD%98%E5%9C%A8%E7%9A%84%E8%AF%8D")
v = vis(h)
ok("CS5 空结果走空态（无错误面板、无「加载失败」）",
   s == 200 and "基础筛选" in v and "ui-err__msg" not in h
   and "加载失败" not in v,
   "http=%s 在列表页=%s ui-err=%d 片段=%s"
   % (s, "基础筛选" in v, h.count("ui-err"),
      v[v.find("共"):v.find("共") + 70]))

print("== CS6 未登录：不得把内部参数细节演给匿名访客 ==")
s, h = page("?date_from=2025-13-45", cookie=False)
v = vis(h)
ok("CS6 匿名页走登录表单且不泄漏后端校验细节",
   s == 200 and "用户登录" in v and "日期格式无效" not in v
   and "ui-err__rid" not in h,
   "http=%s 登录页=%s 泄漏=%s"
   % (s, "用户登录" in v, "日期格式无效" in v))

print("== CS7 静态：反模式在 apps/web 全域清零 + 本页已接线 ==")
r = subprocess.run(["git", "grep", "-n", "--", ANTI, "apps/web"],
                   cwd=REPO, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                   universal_newlines=True, encoding="utf-8")
hits = [x for x in (r.stdout or "").splitlines() if x.strip()]
ok("CS7a 反模式「硬编码 message 进 ErrorPanel」= 0 命中",
   len(hits) == 0, "命中 %d：%s" % (len(hits), hits[:2]))
txt = src(PAGES[0])
bare = chr(10).join(l.split("//")[0] for l in txt.splitlines())
ok("CS7b 种列表页确实调了 normalizeError",
   "normalizeError" in bare and "import { ErrorPanel, normalizeError }"
   in bare, "normalizeError 出现 %d 次" % bare.count("normalizeError"))
ok("CS7c catch 里接住了错误对象（不再 .catch(() => )）",
   ".catch((e)" in bare or ".catch(e =" in bare,
   "形状=%s" % re.findall(r"\.catch\([^)]{0,6}\)", bare)[:3])

print("== CS8 英文界面：记现网读数（细节仍为中文，不判红） ==")
s, h = page("?date_from=2025-13-45", locale="en")
v = vis(h)
seg = v[v.find("日期") - 40 if "日期" in v else 0:][:120]
print("   WARN en 界面片段：%s" % seg.replace(chr(10), " "))
ok("CS8 en 界面也仍有错误面板（只是文案语言不同）",
   s == 200 and "ui-err__msg" in h, "http=%s" % s)

rc = summary()
if rc == 0:
    print("CS 闸门全绿：收紧后的四条 4xx 在前台可见文本里各有其句，"
          "并带 1002 码 chip 与 request_id；空结果与失败两态不混。")
sys.exit(rc)
