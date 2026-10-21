# -*- coding: utf-8 -*-
"""闸门 AX：签到主链 + 首页签到段口径 + 签到配置（批三十三）。

钉四件事（每件先说清「为什么这样断言」）：
  S2 P-90 首页连签口径漂移：/attendance 的 current_streak 取「最后一条非补签
     行」（checkin.rs:188-198，P2 审计时已修），首页那段是漏改的同族副本，
     直接取「按 date 排序的最后一行」——补签行 streak=0 是占位，被当基线就
     让首页在补签过的当天显示「连签 0」。断言：构造 非补签(d-3,streak5) +
     补签行在后，/home 与 /attendance 的 streak 必须相等且为 5。
  S2 P-91 首页补签卡数漏源：/attendance 与 /attendance/resub 都是双源口径
     （商店未消耗订单 + 娱乐屋发放−消耗，0262），首页只数商店订单——刮刮乐
     抽到的补签卡在首页显示 0，用户以为卡丢了。断言：发放 1 张 arcade
     resub_card 后两端口径一致为 1。
  S2 P-471 展示口的连签数断签后不衰减：签到主流程按「昨天有非补签行才 +1，
     否则重置为 1」落库，而两个展示口只把「最后一条非补签行的 streak」原样
     端出去——断签三天仍显示「连签 12 天」，点下签到拿回的却是 streak=1。
     断言：基线在 d-3 时两端口都归 0；基线在 d-1 时仍显示原计数（衰减不许过头）。
  S5 P-92 前端补签取日按 UTC：home-resub-button.tsx 用 toISOString 直接切
     日——北京 0-8 点窗口「昨天」不在列表、尾项落到第 8 天（后端必拒）。
     断言：node 在固定「北京 03:00」求值旧/新两式（旧式缺昨天、含第 8 天；
     新式恰为 7 天合法窗口），并静态钉住 tsx 已带 +8 偏移。
     注：文件层红绿证据取自 HEAD 对照（旧式无偏移、新式有），见审计文档。
  S8 P-97 签到里程碑假开关：全仓 grep 命中 0——后台摆着
     attendance_continuous_day="10,20,30" 与 attendance_continuous_value=
     "200,500,1000"，而代码把同一串数字硬编码在 economy::checkin_reward 的
     match 里，站长改了不生效（且界面上的数字与真实生效那份「撞脸」，最容易被
     当成已生效）。另有 initial/step/max/bonus/card 五个旧公式残留与在读键
     同义（0350 摘除）。断言：把里程碑改成「2,5 → 50,150」后，第 2 天签到
     必须 +50；红线（旧镜像）实测同一形状拿到 bonus=0 reward=8。

S1/S6 为链路正控：首签（含火花流水幂等键、抽卡券 daily_free）、连签 3→4、
里程碑 9→10(+200)、里程碑 29→30（日封顶）、重复签到拒单且不动账、
module_attendance 真开关关→404/4101（签到/状态/补签三口 + 首页段置 off）
→恢复回 200。补签链路（含管理补签面板与并发形状）在闸门 AY
（pt_audit_ay_resub.py）。

共享设置：module_attendance 与 attendance_continuous_day/value 串行改、
快照原值写回并回读；attendance_first/streak/daily_cap 只读。
探针产物（6 个成员号 + 出勤/发放/券账/流水/站内信）自己收尾。

跑法（仓库根目录）：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_ax_attendance.py
"""
import atexit
import json
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ROOT = login()
TAG = int(time.time()) % 100000
PIN = {}
CLEAN = []
UIDS = []
SPARK0 = {}
REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def q(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % r.stderr.strip()[:200])
    return r.stdout.strip()


def ex(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode:
        print("   ! SQL 失败: %s" % (r.stderr or "").strip()[:150])
    return r.returncode


def qi(sql, default=0):
    try:
        v = q(sql)
        return int(v) if v else default
    except Exception:  # noqa: BLE001
        return default


def setting(name):
    return q("SELECT COALESCE((SELECT value FROM site_settings WHERE name = "
             "'%s'), '')" % name)


def setv(name, val):
    assert ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
              % (val, name)) == 0
    got = setting(name)
    print("   [set] %s = %s（回读 %s）" % (name, val, got))
    return got == val


def pin(name):
    PIN[name] = setting(name)


def cleanup():
    """可重复执行：设置原值回读、出勤/发放/券账/流水全收干净。"""
    for name in PIN:
        ex("UPDATE site_settings SET value = '%s' WHERE name = '%s'"
           % (PIN[name], name))
    for uid in UIDS:
        for t in ("attendance", "resub_uses", "shop_orders",
                  "arcade_item_grants", "arcade_item_uses",
                  "gacha_ticket_ledger", "gacha_ticket_balance",
                  "spark_ledger"):
            ex("DELETE FROM %s WHERE user_id = %d" % (t, uid))
        ex("DELETE FROM messages WHERE receiver_id = %d" % uid)
        ex("DELETE FROM user_modify_logs WHERE uid = %d" % uid)
        if uid in SPARK0:
            ex("UPDATE users SET spark_balance = %d WHERE id = %d"
               % (SPARK0[uid], uid))
        ex("UPDATE users SET status = 2 WHERE id = %d" % uid)
    for stmt in reversed(CLEAN):
        ex(stmt)
    del CLEAN[:]
    print("   [cleanup] module_attendance=%s" % setting("module_attendance"))


atexit.register(cleanup)


def brief(x):
    return str(x)[:200]


def site_day(off):
    """站点时区（UTC+8）today 减 off 天，返回 YYYY-MM-DD。"""
    return q("SELECT ((CURRENT_TIMESTAMP + interval '8 hours')::date - %d)"
             "::text" % off)


def ymd(day):
    return day.replace("-", "")


def rej(name, s, r, code, kw):
    msg = str(r.get("message", "")) if isinstance(r, dict) else str(r)
    return ok(name, s == code and kw in msg, "%s %s %s" % (s, msg, brief(r)))


def new_member(role):
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEAN.append("DELETE FROM invites WHERE id = %d" % cid)
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None
    user = "ax%s%d" % (role, TAG)
    s, rr = call("POST", "/auth/register", {
        "username": user, "email": "%s@ax.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    uid = (rr.get("data") or {}).get("user_id")
    if not uid:
        return None
    UIDS.append(uid)
    tok = None
    for _ in range(8):
        tok = (call("POST", "/auth/login",
                    {"username": user, "password": "password123"})[1].get(
                        "data") or {}).get("token")
        if tok:
            break
        time.sleep(6)
    return (tok, uid) if tok else None


def spark(uid):
    return qi("SELECT spark_balance FROM users WHERE id = %d" % uid)


def get_home(tok):
    s, r = call("GET", "/home", token=tok)
    return s, (r.get("data") or {}).get("attendance") or {}


def get_att(tok):
    s, r = call("GET", "/attendance", token=tok)
    return s, (r.get("data") or {})


def craft_att(uid, off, streak, reward, makeup):
    ex("INSERT INTO attendance (user_id, date, streak, reward, makeup) "
       "VALUES (%d, ((CURRENT_TIMESTAMP + interval '8 hours')::date - %d), "
       "%d, %d, %s) ON CONFLICT (user_id, date) DO NOTHING"
       % (uid, off, streak, reward, "TRUE" if makeup else "FALSE"))


def wipe_from(off):
    """删站点日 today-off 及之后的出勤行（含补签占位行）：夹具复位用。"""
    ex("DELETE FROM attendance WHERE user_id = %d AND date >= "
       "((CURRENT_TIMESTAMP + interval '8 hours')::date - %d)" % (CID, off))


def pos_ids(raw):
    """'10,20,30' → [10,20,30]（与 economy_checkin::parse_id_list 同口径）。"""
    out = []
    for x in (raw or "").split(","):
        try:
            n = int(x.strip())
        except ValueError:
            continue
        if n > 0:
            out.append(n)
    return out


print("== S0 地基：探针成员 + 设置快照 ==")
pin("module_attendance")
pin("attendance_continuous_day")
pin("attendance_continuous_value")
FIRST = qi("SELECT value::int FROM site_settings WHERE name = "
           "'attendance_first'", 10)
STREAK = qi("SELECT value::int FROM site_settings WHERE name = "
            "'attendance_streak'", 5)
CAP = qi("SELECT value::int FROM site_settings WHERE name = "
         "'attendance_daily_cap'", 1000)
MILE = dict(zip(pos_ids(setting("attendance_continuous_day")),
                pos_ids(setting("attendance_continuous_value"))))
ok("S0 module_attendance=yes 且三直读键 + 里程碑两键可读",
   setting("module_attendance") == "yes" and FIRST > 0 and STREAK > 0
   and CAP > 0 and len(MILE) >= 3,
   "first=%s streak=%s cap=%s mile=%s" % (FIRST, STREAK, CAP, MILE))
C = new_member("c")   # 首页口径（造补签行 + 娱乐屋卡）
D = new_member("d")   # 首签正控
E = new_member("e")   # 连签 3→4
G = new_member("g")   # 里程碑 9→10
H = new_member("h")   # 里程碑 29→30 封顶
N = new_member("n")   # 里程碑配置改写后生效（P-97）
ok("六个探针成员可准备（注册/登录限流下）", all((C, D, E, G, H, N)))
if not all((C, D, E, G, H, N)):
    sys.exit(1)
CT, CID = C
DT, DID = D
ET, EID = E
GT, GID = G
HT, HID = H
NT, NID = N
for uid in (CID, DID, EID, GID, HID, NID):
    SPARK0[uid] = spark(uid)
T0 = site_day(0)


def expect_reward(streak_after, first_ever):
    """economy_checkin::checkin_reward 同口径：首签 +first，整日 +streak，
    命中里程碑（MILE，取自库里 attendance_continuous_*）当天加送，
    单日封顶 daily_cap。期望值读库而来，不写死数字。"""
    base = FIRST if first_ever else STREAK
    bonus = MILE.get(streak_after, 0)
    return base, bonus, min(base + bonus, CAP)


print("== S1 签到正控：首签 / 连签 / 里程碑 / 封顶 ==")
sp0 = spark(DID)
s, r = call("POST", "/attendance/checkin", {}, token=DT)
d = r.get("data") or {}
ok("S1 首签：streak=1、base=first=%d、无里程碑加送" % FIRST,
   s == 200 and d.get("streak") == 1 and d.get("reward") == FIRST
   and d.get("base") == FIRST and d.get("streak_bonus") == 0,
   "%s %s" % (s, brief(r)))
ok("S1 首签：火花 +%d 且流水幂等键 attendance:{uid}:{ymd}" % FIRST,
   spark(DID) == sp0 + FIRST and qi(
       "SELECT count(*) FROM spark_ledger WHERE idempotency_key = "
       "'attendance:%d:%s'" % (DID, ymd(T0))) == 1
   and qi("SELECT amount FROM spark_ledger WHERE idempotency_key = "
          "'attendance:%d:%s'" % (DID, ymd(T0))) == FIRST,
   "spark=%s" % spark(DID))
ok("S1 首签：抽卡券 daily_free 记账 +1 且余额 1",
   d.get("gacha_ticket") is True and qi(
       "SELECT count(*) FROM gacha_ticket_ledger WHERE idempotency_key = "
       "'gacha:df:%d:%s'" % (DID, ymd(T0))) == 1
   and qi("SELECT balance FROM gacha_ticket_balance WHERE user_id = %d"
          % DID) == 1)
ok("S1 首签：出勤行 streak=1 reward=%d" % FIRST,
   qi("SELECT streak FROM attendance WHERE user_id = %d AND date = "
      "'%s'" % (DID, T0)) == 1
   and qi("SELECT reward FROM attendance WHERE user_id = %d AND date = "
          "'%s'" % (DID, T0)) == FIRST)
s, r = call("POST", "/attendance/checkin", {}, token=DT)
rej("S1 重复签到拒单：400『今天已经签到过啦』", s, r, 400, "已经签到过")
ok("S1 拒单后火花/出勤/券账均未变",
   spark(DID) == sp0 + FIRST
   and qi("SELECT count(*) FROM attendance WHERE user_id = %d" % DID) == 1
   and qi("SELECT count(*) FROM gacha_ticket_ledger WHERE user_id = %d"
          % DID) == 1)
s, h = get_home(DT)
s2, a = get_att(DT)
ok("S1 两端口径一致：checked_today=true、streak=1",
   s == 200 and s2 == 200 and h.get("checked_today") is True
   and a.get("checked_today") is True
   and h.get("streak") == 1 and a.get("streak") == 1,
   "home=%s att=%s" % (brief(h), brief(a)))

craft_att(EID, 1, 3, STREAK, False)
sp0 = spark(EID)
s, r = call("POST", "/attendance/checkin", {}, token=ET)
d = r.get("data") or {}
ok("S1 连签 3→4：streak=4、reward=streak=%d、火花同幅" % STREAK,
   s == 200 and d.get("streak") == 4 and d.get("reward") == STREAK
   and spark(EID) == sp0 + STREAK
   and qi("SELECT streak FROM attendance WHERE user_id = %d AND date = "
          "'%s'" % (EID, T0)) == 4, "%s %s" % (s, brief(r)))

craft_att(GID, 1, 9, STREAK, False)
b, bn, tot = expect_reward(10, False)
sp0 = spark(GID)
s, r = call("POST", "/attendance/checkin", {}, token=GT)
d = r.get("data") or {}
ok("S1 里程碑 9→10：reward=%d（+200）、火花同幅" % tot,
   s == 200 and d.get("streak") == 10 and d.get("reward") == tot
   and d.get("streak_bonus") == bn and spark(GID) == sp0 + tot,
   "exp base=%s bonus=%s %s" % (b, bn, brief(r)))

craft_att(HID, 1, 29, STREAK, False)
b, bn, tot = expect_reward(30, False)
sp0 = spark(HID)
s, r = call("POST", "/attendance/checkin", {}, token=HT)
d = r.get("data") or {}
ok("S1 里程碑 29→30：reward=min(base %d + bonus %d, cap)=%d"
   % (b, bn, tot),
   s == 200 and d.get("streak") == 30 and d.get("reward") == tot
   and d.get("streak_bonus") == bn and bn > 0 and spark(HID) == sp0 + tot,
   "exp=%s %s" % (tot, brief(r)))

print("== S2 P-90 首页连签把补签行当基线（红=首页显示 0）==")
craft_att(CID, 1, 0, 0, True)
craft_att(CID, 3, 5, STREAK, False)
s, h = get_home(CT)
s2, a = get_att(CT)
ok("S2 P-90 /home 与 /attendance 连签同值（补签行在后不当基线）",
   s == 200 and s2 == 200 and h.get("streak") == a.get("streak"),
   "home.streak=%s att.streak=%s" % (h.get("streak"), a.get("streak")))
# 值本身归 P-471 管（P-90 只问「两端口是不是同一格」；旧写法在这里是 5/5，
# 修复后是 0/0——都在同值这条线上，具体该显示几由下面两条钉）。

print("== S2 P-91 首页补签卡只数商店订单（红=娱乐屋卡显示 0）==")
s, h = get_home(CT)
s2, a = get_att(CT)
ok("S2 P-91 基线：两端口径一致=0",
   h.get("makeup_cards") == 0 and a.get("makeup_cards") == 0,
   "home=%s att=%s" % (h.get("makeup_cards"), a.get("makeup_cards")))
ex("INSERT INTO arcade_item_grants (item_key, user_id, qty, game, side, "
   "idem) VALUES ('resub_card', %d, 1, 'ax', 'rand', 'ax-grant-c')" % CID)
s, h = get_home(CT)
s2, a = get_att(CT)
ok("S2 P-91 娱乐屋发 1 张后 /home 与 /attendance 一致=1",
   s == 200 and s2 == 200 and h.get("makeup_cards") == 1
   and a.get("makeup_cards") == 1,
   "home=%s att=%s" % (h.get("makeup_cards"), a.get("makeup_cards")))

print("== S2 P-471 断签后展示口不衰减（红=继续端库里的旧计数）==")
# 现场就是 P-90 那格夹具：最后一条非补签行落在 d-3，链条其实早断了（今天签到
# 主流程只会落 streak=1，见下面「S2 反控」），可 /home 与 /attendance 都把库里
# 那个 5 原样端出去——会员看到的「连签 5 天」和点下去拿回的 streak=1 是两回事。
wipe_from(1)
craft_att(CID, 3, 5, STREAK, False)
s, h = get_home(CT)
s2, a = get_att(CT)
ok("S2 P-471 基线在 d-3（断签）→ 两端口都归 0（实测 %s/%s）"
   % (h.get("streak"), a.get("streak")),
   s == 200 and s2 == 200 and h.get("streak") == 0 and a.get("streak") == 0,
   "home=%s att=%s｜主流程今天落 streak=1" % (h.get("streak"), a.get("streak")))
# 反向钉：昨天签过 → 链条还活着，两端口继续显示原计数（衰减做过头另一半也是洞）
wipe_from(1)
craft_att(CID, 1, 6, STREAK, False)
s, h = get_home(CT)
s2, a = get_att(CT)
ok("S2 P-471 基线在 d-1（昨天签过）→ 两端口仍显示 6（实测 %s/%s）"
   % (h.get("streak"), a.get("streak")),
   s == 200 and s2 == 200 and h.get("streak") == 6 and a.get("streak") == 6,
   "home=%s att=%s" % (h.get("streak"), a.get("streak")))
# 复位给下面的「S2 反控」：d-1 只留补签占位行，非补签基线回到 d-3
wipe_from(1)
craft_att(CID, 1, 0, 0, True)


def flat_src(p):
    """静态判据用源码：剥注释（本批注释里写着旧形状）+ 拼平 Rust 字符串续行。"""
    t = re.sub(r"/\*.*?\*/", "", open(p, encoding="utf-8").read(), flags=re.S)
    t = "\n".join(re.sub(r"(?<!:)//.*", "", ln) for ln in t.split("\n"))
    return re.sub(r"\\\n[ \t]*", "", t)


CKF = flat_src(os.path.join(REPO, "apps/api/src/economy_http/checkin.rs"))
HAF = flat_src(os.path.join(REPO, "apps/api/src/http/home_attendance.rs"))
ok("S2 P-471 静态：两端口都走 economy_checkin::shown_streak（判定只一处）",
   "economy_checkin::shown_streak(" in CKF
   and "economy_checkin::shown_streak(" in HAF,
   "%d/%d" % (CKF.count("shown_streak("), HAF.count("shown_streak(")))
ok("S2 P-90 静态：基线仍排除补签行（SQL 侧 NOT makeup / 首页侧 !*mk）",
   "AND NOT makeup" in CKF and "!*mk" in HAF,
   "att=%d home=%d" % (CKF.count("NOT makeup"), HAF.count("!*mk")))

print("== S2 反控：有补签行但无签到历史者今日签到 ==")
sp0 = spark(CID)
s, r = call("POST", "/attendance/checkin", {}, token=CT)
d = r.get("data") or {}
b, bn, tot = expect_reward(1, False)
ok("S2 反控：非补签基线是 d-3（不是昨天）→ streak=1、reward=%d" % tot,
   s == 200 and d.get("streak") == 1 and d.get("reward") == tot
   and spark(CID) == sp0 + tot, "%s %s" % (s, brief(r)))

print("== S5 P-92 前端补签取日按 UTC（红=北京凌晨窗口错日）==")
js = ("const ts=new Date('2026-10-12T03:00:00+08:00').getTime();"
      "const old7=Array.from({length:7},(_,i)=>new Date(ts-(i+1)*86400e3)"
      ".toISOString().slice(0,10));"
      "const new7=Array.from({length:7},(_,i)=>"
      "new Date(ts+8*3600e3-(i+1)*86400e3).toISOString().slice(0,10));"
      "console.log(JSON.stringify({old:old7,now:new7}));")
nr = subprocess.run(["node", "-e", js], capture_output=True, text=True,
                    encoding="utf-8")
valid = ["2026-10-11", "2026-10-10", "2026-10-09", "2026-10-08",
         "2026-10-07", "2026-10-06", "2026-10-05"]
try:
    out = json.loads(nr.stdout.strip())
except Exception:  # noqa: BLE001
    out = {}
ok("S5 P-92 新式（+8h）7 天恰为站内合法窗口（北京 03:00 求值）",
   out.get("now") == valid, brief(out))
ok("S5 P-92 旧式缺「昨天」且尾项是第 8 天（缺陷在案）",
   out.get("old") is not None and "2026-10-11" not in out["old"]
   and "2026-10-04" in out["old"], brief(out.get("old")))
tsx = os.path.join(REPO, "apps", "web", "components",
                   "home-resub-button.tsx")
txt = open(tsx, encoding="utf-8").read()
ok("S5 前端已按站点时区取日（+8 偏移在案）", "8 * 3600e3" in txt)

print("== S8 P-97 里程碑配置改写必须生效（旧镜像实测 bonus=0）==")
setv("attendance_continuous_day", "2,5")
setv("attendance_continuous_value", "50,150")
craft_att(NID, 1, 1, STREAK, False)
sp0 = spark(NID)
s, r = call("POST", "/attendance/checkin", {}, token=NT)
d = r.get("data") or {}
ok("S8 P-97 第 2 天签到按新配置加送 50（reward=%d）" % (STREAK + 50),
   s == 200 and d.get("streak") == 2 and d.get("streak_bonus") == 50
   and d.get("reward") == STREAK + 50
   and spark(NID) == sp0 + STREAK + 50, "%s %s" % (s, brief(r)))
ok("S8 P-97 出勤行落的就是配置口径（reward=%d）" % (STREAK + 50),
   qi("SELECT reward FROM attendance WHERE user_id = %d AND date = '%s'"
      % (NID, T0)) == STREAK + 50)
ok("S8 复原两键并回读一致",
   setv("attendance_continuous_day", PIN["attendance_continuous_day"])
   and setv("attendance_continuous_value",
            PIN["attendance_continuous_value"]))

print("== S6 module_attendance 真开关：关→404/4101→恢复 ==")
setv("module_attendance", "no")
print("   [wait] 模块缓存 TTL 30s，等失效后断言")
time.sleep(33)
s, r = call("POST", "/attendance/checkin", {}, token=CT)
ok("S6 关闭后签到口 404/4101", s == 404 and r.get("code") == 4101,
   "%s %s" % (s, brief(r)))
s2, r2 = call("GET", "/attendance", token=CT)
ok("S6 关闭后状态口 404/4101", s2 == 404 and r2.get("code") == 4101,
   "%s %s" % (s2, brief(r2)))
s3, r3 = call("POST", "/attendance/resub",
              {"target_date": site_day(5), "idempotency_key": "ax-mod"},
              token=CT)
ok("S6 关闭后补签口 404/4101", s3 == 404 and r3.get("code") == 4101,
   "%s %s" % (s3, brief(r3)))
s4, r4 = call("GET", "/home", token=CT)
ok("S6 关闭后首页签到段置 off（不留空日历）",
   s4 == 200 and ((r4.get("data") or {}).get("attendance") or {}).get(
       "off") is True, brief(r4))
setv("module_attendance", PIN["module_attendance"])
time.sleep(33)
s5, r5 = call("GET", "/attendance", token=CT)
ok("S6 恢复后状态口回 200", s5 == 200 and r5.get("code") == 0,
   "%s %s" % (s5, brief(r5)))
rej("S6 恢复后签到口回业务（D 已签→400 而非 4101）",
    *call("POST", "/attendance/checkin", {}, token=DT),
    400, "已经签到过")

print()
summary()
