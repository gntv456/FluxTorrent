# -*- coding: utf-8 -*-
"""闸门 DY：Web Push 的话题词表两侧必须同一张表（P-370）。

会员视角：设置页勾了浏览器推送，H&R 预警却从来不来。

  P-370（本批已修，API 两侧）：订阅端 `VALID_TOPICS` 是
  ["promo","request","message"]，投递端发 H&R 事件用的是 topic="hr"，而
  `push_to_user` 的过滤是 `$topic = ANY(p.topics)` ⇒ **hr 事件结构上永远匹配不到
  任何订阅**：用户想订也订不了（422「topic 仅支持 […]」），worker 那边入队的
  事件还被无条件标 `processed_ok = true`（现网 push_outbox 里 57 条 hr 事件全在
  这种状态下被吞掉）。反向也漏：消费侧 `TOPICS` 不含 "request"，而订阅端与界面
  都把 request 当成可订话题 ⇒ 任何 request 行入队后永远进不了消费循环。
  修法是把两个常量收成同一张表（hr/promo/message/request），
  `default_topics()` 也跟着覆盖全集。

  本批没动的两处（点名）：
  · 界面 `push-settings.tsx:56` 仍只发 ["promo","request","message"] ——
    加 "hr" 要同步三语文案，而 `apps/web/i18n/{zh-CN,zh-TW,en}.ts` 正被站方改，
    `git add` 是整文件粒度会把别人的在途改动带进本批。修复前用户可用 API 订阅 hr。
  · 偏好键 push_request 不存在，`pref_key` 的 `_ =>` 把 request 折进 push_message
    （刻意借用，不新增白名单键）。这条形状由 DY3 钉住，别让人当成漏判。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dy_push_topics.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SUB = os.path.join(ROOTDIR, "apps", "api", "src", "push_http.rs")
NOT = os.path.join(ROOTDIR, "apps", "api", "src", "push_notify.rs")
UI = os.path.join(ROOTDIR, "apps", "web", "components", "push-settings.tsx")
NOTICE = os.path.join(ROOTDIR, "apps", "api", "src", "community_http",
                      "notice.rs")


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        print("   psql 失败：%s" % (r.stderr or "").strip()[:240])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


def consts(text, name):
    m = re.search(name + r"[^=]*=\s*&?\[[^\]]*\]", text)
    return set(re.findall(r'"([a-z_]+)"', m.group(0))) if m else set()


sub_t = cut(SUB)
not_t = cut(NOT)
valid = consts(sub_t, "VALID_TOPICS")
topics = consts(not_t, "TOPICS")

# ---------- DY1 两侧同一张表 ----------
ok("DY1 订阅端 VALID_TOPICS=%s 与投递端 TOPICS=%s 必须相等"
   % (sorted(valid), sorted(topics)),
   valid == topics and len(valid) >= 4,
   "两侧词表一旦分叉：订阅得到的话题发不出去，或发出去的话题订不到"
   "（P-370 本体就是 hr 只在投递侧、request 只在订阅侧）")
ok("DY1b hr 与 request 都必须在两侧同时出现",
   {"hr", "request", "promo", "message"} <= valid
   and {"hr", "request"} <= topics,
   "少一个就是这一族的具体死法")

# ---------- DY2 默认订阅覆盖全集 ----------
m = re.search(r"fn default_topics\(\)[^}]*\}", sub_t, re.S)
dflt = set(re.findall(r'"([a-z_]+)"', m.group(0))) if m else set()
ok("DY2 不带 topics 订阅时的默认集合=%s 必须覆盖投递侧全集" % sorted(dflt),
   topics <= dflt,
   "默认漏掉某个话题＝不勾选项的会员永远收不到该类事件")

# ---------- DY3 偏好键兜底形状 ----------
pref = re.search(r"fn pref_key.*?\n\}", not_t, re.S)
ok("DY3 偏好兜底是刻意的（无 push_request 键，request 折进 push_message）",
   pref is not None and '_ => "push_message"' in (pref.group(0) if pref
                                                  else ""),
   "白名单里没有 push_request；若将来补了该键，这里必须一起改成显式分支")
wl = cut(NOTICE)
ok("DY3b notice.rs 白名单仍登记 push_hr/push_promo/push_message",
   all(k in wl for k in ('"push_hr"', '"push_promo"', '"push_message"')),
   "偏好键不在白名单＝用户关不掉推送（写不进去的开关）")

# ---------- DY4 真库 A/B：投递过滤真的能命中 hr ----------
AB = r"""
BEGIN;
SELECT 'no_sub=' || count(*) FROM push_subscriptions p
 WHERE p.expired_at IS NULL AND 'hr' = ANY(p.topics);
INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, topics)
VALUES ((SELECT min(id) FROM users), 'https://example.invalid/dy-probe',
        'BMdy', 'ZHl', ARRAY['hr'])
ON CONFLICT (endpoint) DO UPDATE SET topics = EXCLUDED.topics,
  expired_at = NULL;
SELECT 'with_hr=' || count(*) FROM push_subscriptions p
 WHERE p.expired_at IS NULL AND 'hr' = ANY(p.topics);
SELECT 'no_request_sub=' || count(*) FROM push_subscriptions p
 WHERE p.expired_at IS NULL AND 'request' = ANY(p.topics);
ROLLBACK;
SELECT 'residue=' || count(*) FROM push_subscriptions
 WHERE endpoint = 'https://example.invalid/dy-probe';
"""
pc = {}
for ln in psql(AB).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        pc[k.strip()] = v.strip()
ok("DY4 正控：无订阅时命中 %s（须 0）、植入 {hr} 后命中 %s（须 1）、"
   "回滚残留 %s（须 0）"
   % (pc.get("no_sub"), pc.get("with_hr"), pc.get("residue")),
   pc.get("no_sub") == "0" and pc.get("with_hr") == "1"
   and pc.get("residue") == "0",
   "数不到 1＝投递过滤是空判据；残留非 0＝探针把订阅表弄脏了")

# ---------- DY5 现网空转台账 ----------
hr_swallowed = sval("SELECT count(*) FROM push_outbox WHERE topic = 'hr' "
                    "AND processed_ok")
subs = sval("SELECT count(*) FROM push_subscriptions WHERE expired_at IS NULL")
print("   [现网账] topic=hr 且已标 processed_ok 的推送 %s 条，而活跃订阅 %s 条"
      "——修复前这些事件不可能送到任何人（消费侧无条件标完成，丢在哪看不出来）"
      % (hr_swallowed, subs))

# ---------- DY6 界面话题在册（本批未改，理由见文件头） ----------
ui = rd(UI)
m2 = re.search(r"topics:\s*\[([^\]]*)\]", ui)
ui_t = set(re.findall(r'"([a-z_]+)"', m2.group(1))) if m2 else set()
print("   [待办] 界面订阅话题清单=%s；缺 hr 需同步三语文案（i18n 三份文件在站方"
      "手里），补上前用户可用 API 订阅 hr" % sorted(ui_t))
ok("DY6 界面清单必须是 VALID_TOPICS 的子集（不许发一个后端不收的话题）",
   ui_t <= valid,
   "界面若发了后端不认的 topic，订阅整单被拒（422），会员只会看到失败")

summary()
