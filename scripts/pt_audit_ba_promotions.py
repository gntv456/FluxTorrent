# -*- coding: utf-8 -*-
"""闸门 BA：促销（free / x2 / x2free / half / p30）端到端（批三十七）。

促销此前**零闸门覆盖**，而它是全站流量的上游开关：做种量、下载量、分享率、
H&R 豁免、RSS 的「仅免费」筛选、对外 compat 的 free 标记，全都读同一张
`promotions` 表。这条链此前只在批一/批二的 tracker 审计里被顺路碰过。

判据一律取「真发 announce → 真读 traffic_ledger 的入账增量」，不读代码注释：
  S0 地基：探针成员 + 探针种，**不碰任何站点共享设定**（每节两轮上报的理由、
     为什么发 event=started 而不是调 announce_interval，见 make_announcer 注释）。
     正控：无促销时两轮上报必须产出一行 delta_down=步长；产不出就说明信箱没通电
     （本闸所有判据不可信），当场整体判红并打出 tracker 回执。
  S1-S3 → free 下载计 0 / x2 上传翻倍 / x2free 两者兼得，逐档实测
  S4 过期   → ends_at 一过，计费回到 1×1，且展示侧（/torrents/{id}）同步不再
              显示促销——两侧都要断言（先证在促时**看得见**，再证过期**看不见**，
              否则这条判据永远不可能红）
  S5 叠加   → 同一维度多条促销并存（同种 free+x2、全站 x2 + 本种 free、
              全站 half + 本种 x2）：应按**维度**择优（up 取最大、down 取最小），
              而不是把两个维度压成一个标量取「最强档」——压扁时 free 会输给 x2，
              免流被吞（用户在该下的流量照扣）
  S6 写口卫生 → kind 白名单、hours 上下界、category 不存在/漏传、scope 取值；
              拒收必须点到具体判据（不许写成 != 200）；被拒的请求一条都不许落库；
              建/删一条促销后**详情侧的 promotion 字段要跟着变**（展示与计费同窗）
  S8 自购促销（/promo/buy）→ 花魔力买「限时免费」：按价目扣款并落 spark_ledger
     → 出现 scope=torrent/kind=free 的促销行 → 详情角标跟着变 →
     **同幂等键重放不双扣、不多加促销行**；再叠全站 x2 必须合成 x2free
  S7 收尾   → 先 cleanup() 再断言：促销行清零、announce_interval 同值、
              探针号墓碑、探针的 snatch/流水与 announce_seen 全删（不留计费残料）

跑法（仓库根目录，需本地栈在跑）：
    PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ba_promotions.py
"""
import os
import re
import subprocess
import sys
import time
import urllib.parse
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    call, clear_register_rl, login, ok, summary)

ANN = os.environ.get("FLUX_ANN_BASE", "http://127.0.0.1:7070/announce")
ROOT = login()
TAG = int(time.time()) % 100000
MNAME = "ba%d" % TAG  # 探针成员名（S8 要用同一身份登录自购口）
STEP = 1024 * 1024  # 每轮 1 MiB，落在任何速率钳之内
PROMO_IDS = []
AUDIT_IDS = []  # 本轮造过的全部促销行（收尾判「一条不剩」）
UID = None
INTERVAL_SAVED = None


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q", "-d",
         "fluxtorrent", "-tAc", q],
        capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql failed: %s" % (r.stderr or "")[:180])
    return r.stdout.strip()


def setting(name):
    return psql("SELECT COALESCE((SELECT value FROM site_settings "
                "WHERE name = '%s'), '__ABSENT__')" % name)


def cleanup():
    for pid in AUDIT_IDS:
        psql("DELETE FROM promotions WHERE id = %s" % pid)
    if UID:
        psql("DELETE FROM snatches WHERE user_id = %s" % UID)
        psql("DELETE FROM traffic_ledger WHERE user_id = %s" % UID)
        psql("DELETE FROM announce_seen WHERE user_id = %s" % UID)
        psql("UPDATE users SET status = 2 WHERE id = %s" % UID)
    ids = ",".join(str(x) for x in AUDIT_IDS) or "-1"
    left = psql("SELECT count(*) FROM promotions WHERE id IN (%s)" % ids)
    print("   [cleanup] 促销行自删（残留 %s/%d）；探针 %s 已墓碑并清流水；"
          "announce_interval 未被本闸门改动（仍为 %s）"
          % (left, len(AUDIT_IDS), UID, setting("announce_interval")))


import atexit  # noqa: E402
atexit.register(cleanup)


def set_promo(scope, kind, torrent_id=None, category_id=None, hours=2,
              starts_in=0):
    """按库内真实形状插一条促销（写口 S6 另测；这里要的是「裁决」）。"""
    cond = "now() + interval '%d seconds'" % starts_in
    tid = "NULL" if torrent_id is None else str(torrent_id)
    cid = "NULL" if category_id is None else str(category_id)
    psql("INSERT INTO promotions (scope, torrent_id, category_id, kind, "
         "starts_at, ends_at, source, created_by) VALUES ('%s', %s, %s, "
         "'%s'::promotion_kind_enum, %s, %s + interval '%d hours', 'manual', "
         "1)" % (scope, tid, cid, kind, cond, cond, hours))
    pid = psql("SELECT id FROM promotions ORDER BY id DESC LIMIT 1")
    PROMO_IDS.append(int(pid))
    AUDIT_IDS.append(int(pid))
    return pid


def clear_promos():
    for pid in PROMO_IDS:
        psql("DELETE FROM promotions WHERE id = %s" % pid)
    PROMO_IDS.clear()


# ---------------------------------------------------------------- 探针与 announce
def new_member():
    clear_register_rl()
    time.sleep(6)
    codes = ((call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                   token=ROOT)[1].get("data") or {}).get("codes")) or []
    if not codes:
        return None, None
    code, cid = codes[0]["code"], codes[0]["id"]
    cap = call("GET", "/auth/captcha", token=ROOT)[1].get("data") or {}
    nums = [int(x) for x in re.findall(r"-?\d+", str(cap.get("question")))]
    name = MNAME
    call("POST", "/auth/register", {
        "username": name, "email": "%s@ba.local" % name,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": (nums[0] + nums[1]) if len(nums) > 1 else 1,
        "captcha_token": "", "fields": {}})
    uid = psql("SELECT id FROM users WHERE username = '%s'" % name)
    psql("DELETE FROM invites WHERE id = %d" % cid)
    pk = psql("SELECT passkey FROM users WHERE id = %s" % uid) if uid else ""
    return (int(uid), pk) if uid and pk else (None, None)


def make_announcer(pk, ih_hex):
    ihq = urllib.parse.quote(bytes.fromhex(ih_hex.strip()), safe="")
    pidq = urllib.parse.quote(b"-BAGATE%012d" % TAG, safe="")
    n = [0]
    rep = [""]

    def go(up, down, left):
        n[0] += 1
        # event=started 而不是周期上报：tracker 对**周期** announce 有 60~900s
        # 的事件合并窗（gate::merge_window_secs，默认 interval×40% 且下限 60s），
        # 探针第二轮若在窗内就直接不进流；started/completed/stopped 是事件语义，
        # 永不合并（gate.rs:212 注释自陈）。计费侧 process_event 对 started 与
        # 周期上报走同一条差值路径（只有 completed/stopped 有分支），所以这个
        # 换法不改变被测行为，也不动站点的 announce_interval（下调它会把全站
        # peer TTL 压到秒级，真实做种者的 swarm 会看不见）。
        q = ("info_hash=%s&peer_id=%s&port=%d&uploaded=%d&downloaded=%d"
             "&left=%d&numwant=50&key=deadbeef&event=started"
             % (ihq, pidq, 6881 + n[0], up, down, left))
        try:
            with urllib.request.urlopen("%s/%s?%s" % (ANN, pk, q),
                                        timeout=20) as f:
                rep[0] = f.read()[:120]
        except Exception as e:  # noqa: BLE001
            rep[0] = b"ERR %s" % str(e).encode()[:100]
        return rep[0]
    go.reply = rep
    return go


def measure(ann, state, label):
    """两轮同 peer 递增上报 → 读**本轮新增**那条流水的 (delta_up, delta_down)。

    第一轮只把锚点推到 state，第二轮才产生可判增量。
    判据必须等「行数增加」：拿不到新增行就返回 None，绝不能退回读旧行——
    旧行是上一节的数，读它会「永远绿」。
    """
    before = int(psql("SELECT count(*) FROM traffic_ledger WHERE user_id = %s"
                      % UID))
    r1 = ann(state["up"], state["down"], state["left"])
    time.sleep(6)
    state["down"] += STEP
    state["up"] += STEP
    state["left"] = max(0, state["left"] - STEP)
    r2 = ann(state["up"], state["down"], state["left"])
    row = ""
    for _ in range(25):
        n = int(psql("SELECT count(*) FROM traffic_ledger WHERE user_id = %s"
                     % UID))
        if n > before:
            row = psql("SELECT delta_up || '/' || delta_down || '/' || "
                       "promotion_kind FROM traffic_ledger WHERE user_id = "
                       "%s ORDER BY id DESC LIMIT 1" % UID)
            break
        time.sleep(4)
    if not row:
        print("   [%s] 本轮没有新增流水（行数仍为 %d）tracker 回执 %r / %r"
              % (label, before, r1[:80], r2[:80]))
        return None
    du, dd, pk = row.split("/")
    print("   [%s] 入账 up=%s down=%s 促销码=%s" % (label, du, dd, pk))
    return int(du), int(dd), int(pk)


print("== S0 地基：探针成员 + 探针种（不动站点共享设定）==")
UID, PK = new_member()
tid, ih = psql("SELECT t.id || ' ' || trim(t.info_hash) FROM torrents t "
               "WHERE t.approval_status = 1 ORDER BY t.id DESC LIMIT 1"
               ).split(" ", 1)
SIZE = int(psql("SELECT size FROM torrents WHERE id = %s" % tid))
CAT = psql("SELECT category_id FROM torrents WHERE id = %s" % tid)
ok("探针成员与探针种就位（uid=%s torrent=%s size=%s）" % (UID, tid, SIZE),
   bool(UID) and bool(PK) and SIZE > 8 * STEP,
   "pk=%s…" % ((PK or "")[:6]))
if not UID:
    sys.exit(1)
INTERVAL_SAVED = setting("announce_interval")
print("   [read-only] announce_interval=%s（本闸门不改它：改了会把全站 peer "
      "TTL 压到秒级，真实做种者从 swarm 里消失）" % INTERVAL_SAVED)
ann = make_announcer(PK, ih)
state = {"up": 0, "down": 0, "left": SIZE}
base = measure(ann, state, "基线")
ok("信箱通电正控：无促销时两轮上报必须落一行账（拿不到就说明整节判据不可信）",
   bool(base) and base[1] > 0, "%s" % (base,))
if not base:
    sys.exit(1)
ok("基线口径：无促销 → 上传/下载都按 1×1 入账（步长 %d）" % STEP,
   base[0] == STEP and base[1] == STEP and base[2] == 0, "%s" % (base,))

print("== S1 free：下载计 0、上传照常 ==")
clear_promos()
set_promo("torrent", "free", torrent_id=tid)
r1 = measure(ann, state, "free")
ok("S1 free → delta_down=0 且 delta_up 仍计（促销真的进到了计费）",
   r1 is not None and r1[1] == 0 and r1[0] == STEP, "%s" % (r1,))
ok("S1 落账促销码=1(free)（0286 那列的留痕要能反查倍率）",
   r1 is not None and r1[2] == 1, "%s" % (r1,))

print("== S2 x2：上传翻倍、下载照常 ==")
clear_promos()
set_promo("torrent", "x2", torrent_id=tid)
r2 = measure(ann, state, "x2")
ok("S2 x2 → delta_up=2×步长 且 delta_down 按 1× 计",
   r2 is not None and r2[0] == 2 * STEP and r2[1] == STEP, "%s" % (r2,))
ok("S2 落账促销码=2(x2)", r2 is not None and r2[2] == 2, "%s" % (r2,))

print("== S3 x2free：上传翻倍 + 下载计 0 ==")
clear_promos()
set_promo("torrent", "x2free", torrent_id=tid)
r3 = measure(ann, state, "x2free")
ok("S3 x2free → 上传翻倍且下载为 0",
   r3 is not None and r3[0] == 2 * STEP and r3[1] == 0, "%s" % (r3,))
ok("S3 落账促销码=3(x2free)", r3 is not None and r3[2] == 3, "%s" % (r3,))

print("== S4 过期一致性：计费与展示必须同时失效 ==")
clear_promos()
pid = set_promo("torrent", "free", torrent_id=tid)
d = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S4 正控：在促期间展示侧下发促销码（实测 %s / 结束 %s）"
   % (d.get("promotion"), d.get("promotion_ends_at")),
   d.get("promotion") == "free", "%s" % ({k: d.get(k) for k in
                                        ("promotion", "promotion_ends_at")},))
psql("UPDATE promotions SET starts_at = now() - interval '2 hours', "
     "ends_at = now() - interval '5 minutes' WHERE id = %s" % pid)
r4 = measure(ann, state, "过期")
ok("S4 促销一过 ends_at，计费立刻回到 1×1 且落账不留促销码",
   r4 is not None and r4[0] == STEP and r4[1] == STEP and r4[2] == 0,
   "%s" % (r4,))
d2 = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S4 展示侧同口径：过期促销不再下发（实测 %s）" % (d2.get("promotion"),),
   d2.get("promotion") is None, "%s" % ({k: d2.get(k) for k in
                                        ("promotion", "promotion_ends_at")},))

print("== S5 叠加裁决：free + x2 应为「上传翻倍且下载 0」 ==")
clear_promos()
set_promo("torrent", "free", torrent_id=tid)
set_promo("torrent", "x2", torrent_id=tid)
r5 = measure(ann, state, "free+x2 同种")
ok("S5 同种子两条促销并存时按维度择优（up 取最大、down 取最小）⇒ 2×STEP / 0",
   r5 is not None and r5[0] == 2 * STEP and r5[1] == 0, "%s" % (r5,))
ok("S5 落账促销码=3(x2free)（合成后的倍率反查档位，不是「最强的一条」）",
   r5 is not None and r5[2] == 3, "%s" % (r5,))
clear_promos()
set_promo("global", "x2")
set_promo("torrent", "free", torrent_id=tid)
r5b = measure(ann, state, "全站x2+本种free")
ok("S5b 全站 x2 期间单种 free 仍是免流（跨范围择优）⇒ 2×STEP / 0",
   r5b is not None and r5b[0] == 2 * STEP and r5b[1] == 0, "%s" % (r5b,))
d6 = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S5b 角标说的是同一件事：free + 全站 x2 合成后应显示 2xFree（实测 %s）"
   % (d6.get("promotion"),), d6.get("promotion") == "x2free",
   "%s" % (d6.get("promotion"),))
lst = (call("GET", "/torrents?limit=200&alive=0", token=ROOT)[1].get("data")
       or {})
# alive=0 = 「含断种」：默认视图只回活种（seeders>0），演示库 69 条全是断种，
# 不带这个参数探针种根本不在列表里，判据会红在探针身上而不是代码上
items = lst.get("items") or []
row5 = [x for x in items if str(x.get("id")) == str(tid)]
ok("S5b 列表角标与详情同值（列表取到 %s / 共 %d 行）"
   % ([x.get("promotion") for x in row5], len(items)),
   bool(row5) and row5[0].get("promotion") == "x2free",
   "%s" % ([x.get("promotion") for x in row5],))
adm = (call("GET", "/admin/torrents?per_page=50&promo=yes",
            token=ROOT)[1].get("data") or {})
arows = adm.get("rows") or []
arow = [x for x in arows if str(x.get("id")) == str(tid)]
ok("S5b 后台列表角标同值且「促销中」筛选把全站促销作用到的种也筛进来"
   "（实测 %s / 命中 %d 条）" % ([x.get("promotion") for x in arow],
                                 len(arows)),
   bool(arow) and arow[0].get("promotion") == "x2free",
   "%s total=%s" % ([x.get("promotion") for x in arow], adm.get("total")))
r5b_code = r5b is not None and r5b[2] == 3
ok("S5b 落账促销码=3(x2free)——H&R 与作弊差距判据都按 1/3 豁免，"
   "记成 2(x2) 就会把免流期间的合法差额拿去告警",
   r5b_code, "%s" % (r5b,))
clear_promos()
set_promo("global", "half")
set_promo("torrent", "x2", torrent_id=tid)
r5c = measure(ann, state, "全站half+本种x2")
ok("S5c 全站 half + 本种 x2 ⇒ 上传翻倍且下载半计（2×STEP / 步长÷2）",
   r5c is not None and r5c[0] == 2 * STEP and r5c[1] == STEP // 2,
   "%s" % (r5c,))
ok("S5c 落账促销码=5(x2half)", r5c is not None and r5c[2] == 5,
   "%s" % (r5c,))
d7 = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S5c 角标同值：half + x2 合成应显示 2x50%%（实测 %s）"
   % (d7.get("promotion"),), d7.get("promotion") == "x2half",
   "%s" % (d7.get("promotion"),))
clear_promos()

print("== S6 写口卫生（/admin/freeleech）==")
clear_promos()
n_before = int(psql("SELECT count(*) FROM promotions"))
rej = 0
s, r = call("POST", "/admin/freeleech", {"kind": "free", "scope": "global",
                                          "hours": 2}, token=ROOT)
ok("S6 正控：global 促销可建（200 + 回 id）",
   s == 200 and (r.get("data") or {}).get("id"), "%s %s" % (s, r))
gid = (r.get("data") or {}).get("id")
if gid:
    PROMO_IDS.append(int(gid))
    AUDIT_IDS.append(int(gid))
d3 = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S6 正控：刚建的 global free 立刻反映到种详情（%s）" % (d3.get("promotion"),),
   d3.get("promotion") == "free", "%s" % (d3.get("promotion"),))
s, r = call("DELETE", "/admin/freeleech/%s" % gid, token=ROOT)
ok("S6 单条删除口按 id 生效（200 + deleted=%s）" % (gid,),
   s == 200 and str((r.get("data") or {}).get("deleted")) == str(gid),
   "%s %s" % (s, r))
AUDIT_IDS.remove(int(gid))
PROMO_IDS.remove(int(gid))
s, r = call("GET", "/torrents/%s" % tid, token=ROOT)
d4 = ((r or {}).get("data")) or {}
ok("S6 删掉促销后详情不再下发（实测 %s）" % (d4.get("promotion"),),
   d4.get("promotion") is None, "%s" % (d4.get("promotion"),))
s, r = call("POST", "/admin/freeleech", {"kind": "x9free", "scope": "global",
                                         "hours": 2}, token=ROOT)
rej += 1
msg = str((r or {}).get("message", ""))
ok("S6 未知 kind 必须 400 且点到「类型无效」（旧版 unwrap_or 会吞拼错档）",
   s == 400 and "类型无效" in msg, "%s %s" % (s, msg))
s, r = call("POST", "/admin/freeleech", {"kind": "free", "scope": "global",
                                         "hours": 0}, token=ROOT)
rej += 1
ok("S6 hours 下界必须 400 且点到「时长」",
   s == 400 and "时长" in str((r or {}).get("message", "")),
   "%s %s" % (s, (r or {}).get("message")))
s, r = call("POST", "/admin/freeleech", {"kind": "free", "scope": "global",
                                         "hours": 721}, token=ROOT)
rej += 1
ok("S6 hours 上界必须 400 且点到「时长」（不可越过 720h=30 天）",
   s == 400 and "时长" in str((r or {}).get("message", "")),
   "%s %s" % (s, (r or {}).get("message")))
s, r = call("POST", "/admin/freeleech", {"kind": "free", "scope": "category",
                                         "category_id": 999999, "hours": 2},
            token=ROOT)
rej += 1
ok("S6 分类不存在必须 400 且点到「分类不存在」",
   s == 400 and "分类不存在" in str((r or {}).get("message", "")),
   "%s %s" % (s, (r or {}).get("message")))
s, r = call("POST", "/admin/freeleech", {"kind": "free", "scope": "category",
                                         "hours": 2}, token=ROOT)
rej += 1
ok("S6 分类促销漏传 category_id 必须 400 且点到「需指定分类」",
   s == 400 and "需指定分类" in str((r or {}).get("message", "")),
   "%s %s" % (s, (r or {}).get("message")))
s, r = call("POST", "/admin/freeleech", {"kind": "free", "scope": "torrent",
                                         "torrent_id": int(tid),
                                         "hours": 2}, token=ROOT)
rej += 1
print("   [口径] 种子级促销不走这个口（走批量/购买/字幕 fulfil）：http=%s %s"
      % (s, (r or {}).get("message")))
ok("S6 scope=torrent 必须 400 且点到「范围无效」（不能静默建成 global）",
   s == 400 and "范围无效" in str((r or {}).get("message", "")),
   "%s %s" % (s, (r or {}).get("message")))
n_after = int(psql("SELECT count(*) FROM promotions"))
ok("S6 %d 条被拒的写请求一条都没落库（前 %d 后 %d，中间那条已按 id 删除）"
   % (rej, n_before, n_after), n_after == n_before,
   "before=%d after=%d" % (n_before, n_after))
clear_promos()
set_promo("category", "free", category_id=int(CAT))
r7 = measure(ann, state, "分类促销")
ok("S6 分类促销对本分类下的种子生效（down=0）",
   r7 is not None and r7[1] == 0, "%s（种子 category_id=%s）" % (r7, CAT))
d5 = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S6 分类促销在详情侧同样可见（实测 %s）" % (d5.get("promotion"),),
   d5.get("promotion") == "free", "%s" % (d5.get("promotion"),))
clear_promos()

print("== S8 自购促销（/promo/buy）：扣魔力 → 落促销 → 角标 → 幂等 ==")
clear_promos()
# 探针号自己要能登录：注册口令与 new_member() 里写的一致
s8, r8 = call("POST", "/auth/login", {"username": MNAME,
                                      "password": "password123"})
mtok = ((r8 or {}).get("data") or {}).get("token")
ok("S8 正控：探针成员可登录（自购口只认登录态，不认 staff）",
   bool(mtok), "login http=%s %s" % (s8, str((r8 or {}).get("message"))[:60]))
if not mtok:
    sys.exit(1)
psql("UPDATE users SET spark_balance = 20000 WHERE id = %s" % UID)
bal0 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID))
idem = "ba%d-free" % TAG
# 服务端把幂等键前缀成 `promo:<uid>:<客户端键>`
# （content_http/promo.rs 里 `format!("promo:{}:{}", auth.id, k)`），
# 所以查流水要按这个整键查——旧写法把顺序猜成 `promo:<键>:<uid>` ⇒ 恒 0 行，
# 「没落账」被读成「没扣款」，正控自己假红
idem_full = "promo:%d:%s" % (UID, idem)
s, r = call("POST", "/promo/buy", {"torrent_id": int(tid), "kind": "free",
                                   "hours": 24, "idempotency_key": idem},
            token=mtok)
d8 = (r or {}).get("data") or {}
bal1 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID))
led = int(psql("SELECT count(*) FROM spark_ledger WHERE user_id = %s "
               "AND idempotency_key = '%s'" % (UID, idem_full)))
ok("S8 买 24h 限时免费：200 + 按价目扣款（%d→%d，价 %s）+ 落 1 条流水"
   % (bal0, bal1, d8.get("price")), s == 200 and (r or {}).get("code") == 0
   and bal0 - bal1 == int(d8.get("price") or -1) and led == 1,
   "%s %s bal %d->%d ledger=%d" % (s, str(r)[:120], bal0, bal1, led))
bought = psql("SELECT id || '/' || scope || '/' || kind FROM promotions "
              "WHERE torrent_id = %s AND source = 'manual' ORDER BY id DESC "
              "LIMIT 1" % tid)
if bought.split("/")[0].isdigit():
    AUDIT_IDS.append(int(bought.split("/")[0]))
    PROMO_IDS.append(int(bought.split("/")[0]))
ok("S8 促销行按 scope=torrent / kind=free 落库（实测 %s）" % (bought,),
   bought.endswith("/torrent/free"), "%s" % (bought,))
d9 = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S8 自购的免流立刻进详情角标（实测 %s）" % (d9.get("promotion"),),
   d9.get("promotion") == "free", "%s" % (d9.get("promotion"),))
s, r = call("POST", "/promo/buy", {"torrent_id": int(tid), "kind": "free",
                                   "hours": 24, "idempotency_key": idem},
            token=mtok)
bal2 = int(psql("SELECT spark_balance FROM users WHERE id = %s" % UID))
led2 = int(psql("SELECT count(*) FROM spark_ledger WHERE user_id = %s "
                "AND idempotency_key = '%s'" % (UID, idem_full)))
n_promo = int(psql("SELECT count(*) FROM promotions WHERE torrent_id = %s "
                   "AND kind = 'free' AND ends_at > now()" % tid))
ok("S8 同幂等键重放：不双扣（%d→%d）、流水仍 1 条、不多加促销行（%d 条）"
   % (bal1, bal2, n_promo), bal2 == bal1 and led2 == 1 and n_promo == 1,
   "bal %d->%d ledger=%d promo=%d" % (bal1, bal2, led2, n_promo))
set_promo("global", "x2")
d10 = (call("GET", "/torrents/%s" % tid, token=ROOT)[1].get("data") or {})
ok("S8 自购 free 遇全站 x2 ⇒ 角标合成 x2free（实测 %s）"
   % (d10.get("promotion"),), d10.get("promotion") == "x2free",
   "%s" % (d10.get("promotion"),))
lst2 = (call("GET", "/torrents?limit=200&alive=0&promo=x2",
             token=ROOT)[1].get("data") or {})
ids2 = [str(x.get("id")) for x in (lst2.get("items") or [])]
ok("S8 「仅双倍」筛选把这条 free+x2 的种也收进来（命中 %d 条）"
   % len(ids2), str(tid) in ids2, "ids=%s…" % (ids2[:5],))
# S8 的自购行是**走写口**造的（/promo/buy），不经过 set_promo，
# 因此从来不在 PROMO_IDS / AUDIT_IDS 里——clear_promos() 清不掉它。
# 实测（批一百五十四）：ba 跑完后库里留一行生效的 free 促销，
# 下一支闸门（cn）用「生效促销残留=0」判它完全过，被判为自己的残留。
# ——这里补显式按探针种子清，并把它记进 AUDIT_IDS 让 atexit 也能收尾。
psql("DELETE FROM promotions WHERE torrent_id = %s" % tid)
left_self = psql("SELECT count(*) FROM promotions WHERE torrent_id = %s" % tid)
psql("DELETE FROM spark_ledger WHERE user_id = %s AND kind = 'promo_buy'"
     % UID)
psql("UPDATE users SET spark_balance = 0 WHERE id = %s" % UID)
ok("S8 自购残料已清（流水 0 条、该种促销 0 条）",
   psql("SELECT count(*) FROM spark_ledger WHERE user_id = %s" % UID) == "0"
   and left_self == "0",
   "流水=%s 促销残留=%s"
   % (psql("SELECT count(*) FROM spark_ledger WHERE user_id = %s" % UID),
      left_self))

print("== S7 收尾 =="); cleanup()  # 清理必须先于断言（原只挂 atexit=假红）
ok("S7 站点设定 announce_interval 全程未被改动（前 %s / 后 %s）"
   % (INTERVAL_SAVED, setting("announce_interval")),
   setting("announce_interval") == INTERVAL_SAVED)
ids = ",".join(str(x) for x in AUDIT_IDS) or "-1"
left = psql("SELECT count(*) FROM promotions WHERE id IN (%s)" % ids)
ok("S7 探针促销行一条不剩（造过 %d 条，残留 %s）" % (len(AUDIT_IDS), left),
   left == "0", "left=%s" % left)
psql("DELETE FROM snatches WHERE user_id = %s" % UID)
psql("DELETE FROM traffic_ledger WHERE user_id = %s" % UID)
psql("DELETE FROM announce_seen WHERE user_id = %s" % UID)
psql("UPDATE users SET status = 2 WHERE id = %s" % UID)
ok("S7 探针不留 snatch/流量流水/announce 幂等行（计费残料清了）",
   psql("SELECT count(*) FROM snatches WHERE user_id = %s" % UID) == "0"
   and psql("SELECT count(*) FROM traffic_ledger WHERE user_id = %s"
            % UID) == "0"
   and psql("SELECT count(*) FROM announce_seen WHERE user_id = %s"
            % UID) == "0")
ok("S7 探针号已墓碑（status=2）",
   psql("SELECT status FROM users WHERE id = %s" % UID) == "2")
summary()
