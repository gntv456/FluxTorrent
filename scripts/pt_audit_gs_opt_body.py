# -*- coding: utf-8 -*-
"""闸门 GS：可选 JSON 请求体不得把「载荷写坏」折成「没带参数」（P-478 一族）。

病形（实测）：actix 的 `Option<web::Json<T>>` 走 `FromRequestOpt`，内层**任何**
解析错误都被折成 `Ok(None)`，于是请求照样执行，只是按「什么都没填」的缺省走。
各端口的代价差别很大，都不是 500 那种会响的错：
  * 九宫格/扭蛋/大转盘/宠物投喂/口粮券核销：幂等键丢失 ⇒ `idem_key` 回落随机键
    ⇒ 网络重试或双击**再扣一次票**（活体实测旧镜像对 `{"count":5}` 回 200 并扣票）；
  * 贷款还款：`amount` 丢失 ⇒ 「还 500」被演成**整笔结清**；
  * 临时下载凭证吊销：`torrent_id` 丢失 ⇒ 「只撤这一种」放大成**撤本人全部**；
  * 宠物改名：name/species 丢失 ⇒ 回 200 报「成功」，一个字没改。
修法：`crate::http::OptJson<T>`（与查询侧 `Q<T>` 同一先例）——空体/裸 `null`
仍算「没带参数」，坏载荷交回 1002 + `humanize_parse_error` 的人话细节。

判据：
  GS1 普查 `Option<web::Json<`（剥注释后）的**文件清单**必须恰等于在册清单；
      多一个＝新长出来的口，少一个＝有人改了却没销桩；
  GS2 九个换过 OptJson 的口逐个点名在册；
  GS3 提取器本体不许再把错误吞成 None（静态：必须有 Validation 出口，
      且不得出现 `.ok()` / `unwrap_or(None)` 那种吞法）；
  GS4 活体反例：探针成员用 `{"count":5}` 打 /games/jgg ⇒ 400 且点到那个键，
      余额不变、该键流水 0 行（旧镜像这条会红＝缺陷本体）；
  GS5 活体正例：合法载荷打一局 ⇒ 200；同键重放 ⇒ 被拒且**只扣一次**
      （证明严格化没有把正路挡掉，也没把幂等弄丢）；
  GS6 收尾：探针停用，本轮专属前缀的残留逐条报数。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_gs_opt_body.py
"""
import inspect
import os
import re
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, login, ok, psql_stdin, summary)  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOTDIR, "apps", "api", "src")
RUN = int(time.time())
MARK = "gs%d" % RUN
# 站长在途件（interact.rs 本轮未提交）：等那批落地后把答谢口换成 OptJson，
# 再把这一项从清单里删掉（销桩＝清单变空）。
# 2026-10-26 版主撤桩：最后一只（感谢口 /torrents/{id}/thanks）已换 OptJson，
# 清单清空。这一支从此只作**棘轮**用——谁再写 `Option<web::Json<T>>`，
# 该形状会把一切解析错误折成 `Ok(None)`（坏载荷静默走缺省），GS1 当场红。
OPTION_JSON_IN = set()
OPT_PORTS = [
    ("apps/api/src/games_http/pool_games.rs", 3),
    ("apps/api/src/games_http/pets.rs", 2),
    ("apps/api/src/games_http/pet_custom.rs", 1),
    ("apps/api/src/games_http/linkage.rs", 1),
    ("apps/api/src/economy_http/loan_repay.rs", 1),
    ("apps/api/src/compat_http/download_keys.rs", 1),
    ("apps/api/src/torrent_http/interact.rs", 1),
]


def rel(p):
    return os.path.relpath(p, ROOTDIR).replace("\\", "/")


def flat_comments(text):
    """剥掉 `//` 注释：本仓 torrent_delete.rs 的模块说明里就写着
    `Option<web::Json<T>>`，不剥注释会把「已经修好的口」算成待改项。"""
    return "\n".join(re.sub(r"(?<!:)//.*", "", ln) for ln in text.split("\n"))


def rs(path):
    return open(path, encoding="utf-8", errors="replace").read()


def sval(q):
    return psql_stdin(q).strip()


print("== GS1 普查：还在用 Option<web::Json< 的口 ==")
hits = set()
for base, _d, names in os.walk(API):
    for n in names:
        if not n.endswith(".rs"):
            continue
        p = os.path.join(base, n)
        if re.search(r"Option<web::Json<", flat_comments(rs(p))):
            hits.add(rel(p))
ok("GS1 剩余口清单 == 在册清单（实抓=%s）" % sorted(hits),
   hits == OPTION_JSON_IN,
   "多出来＝新写的口在用吞错形状（坏载荷静默走缺省，钱照样扣而不报错），"
   "要一起换 OptJson；少了＝清单里那一行已修，从 OPTION_JSON_IN 删掉")

print("== GS2 换过 OptJson 的口逐个在册 ==")
miss = []
total = 0
for r, want in OPT_PORTS:
    got = len(re.findall(r"crate::http::OptJson<",
                         flat_comments(rs(os.path.join(ROOTDIR, r)))))
    total += got
    if got != want:
        miss.append("%s 应 %d 实 %d" % (r, want, got))
ok("GS2 七个文件共 %d 处 OptJson 形参（应 10）；缺配=%s" % (total, miss or "无"),
   total == 10 and not miss, "少一处＝那一口退回吞错误写法，钱照样扣而不报错")

print("== GS3 提取器本体不许吞错 ==")
qenv = flat_comments(rs(os.path.join(API, "http", "qenv.rs")))
i = qenv.index("impl<T: DeserializeOwned + 'static> FromRequest for OptJson")
body = qenv[i:qenv.index("\n}\n", i)]
ok("GS3 OptJson 有 Validation 出口且没有 .ok()/unwrap_or(None) 吞法",
   "DomainError::Validation" in body
   and "from_slice::<Option<T>>" in body
   and re.search(r"\.ok\(\)|unwrap_or\(None\)", body) is None,
   "取体段长度 %d 字符；吞错＝GS4 那类活体反例重新成立" % len(body))

print("== GS4/GS5 活体：反例必须被拒、正例与幂等必须还通 ==")
PW = inspect.signature(login).parameters["pw"].default
ROOT = login()
user = "%su" % MARK
st, r = call("POST", "/admin/adduser", {"username": user, "password": PW,
                                       "email": "%s@gs.local" % MARK},
             token=ROOT)
uid = int(sval("SELECT id FROM users WHERE username = '%s'" % user))
t0 = login(user)
call("POST", "/me/password/change", {"old_password": PW,
                                     "new_password": PW + "x"}, token=t0)
tok = login(user, PW + "x")
# 注资走发放口（不落直写 SQL）：直写会让「余额 == 流水净额」全站对账长期不平，
# 而那条对账正是 GT5 的判据——闸门自己不该制造同族假红。
sf, rf = call("POST", "/admin/amountbonus",
              {"amount": 5000, "user_id": uid,
               "idempotency_key": "%s-fund" % MARK}, token=ROOT)
bal_now = int(sval("SELECT spark_balance FROM users WHERE id = %d" % uid))
ok("GS0 探针成员可用且注资走发放口（uid=%d 余额=%d http=%s）"
   % (uid, bal_now, sf), tok is not None and bal_now >= 5000,
   "code=%s msg=%s" % ((rf or {}).get("code"),
                       str((rf or {}).get("message"))[:30]))

bal0 = int(sval("SELECT spark_balance FROM users WHERE id = %d" % uid))
badk = "%s-unk" % MARK
st4, r4 = call("POST", "/games/jgg", {"idempotency_key": badk, "count": 5},
               token=tok)
msg4 = str((r4 or {}).get("message") or "")
bal4 = int(sval("SELECT spark_balance FROM users WHERE id = %d" % uid))
rows4 = int(sval("SELECT count(*) FROM spark_ledger WHERE user_id = %d AND "
                 "idempotency_key LIKE '%%%s'" % (uid, badk)))
ok("GS4 多带未知键 ⇒ 400 且点到该键、余额不变、流水 0 行"
   "（实测 http=%s %s；余额 %d→%d 流水=%d）"
   % (st4, msg4[:40], bal0, bal4, rows4),
   st4 == 400 and "未识别的字段" in msg4 and bal4 == bal0 and rows4 == 0,
   "旧镜像这条必然红：它把坏载荷当「没带参数」，200 + 扣票 100 ⇒ 重试双扣")

goodk = "%s-ok" % MARK
st5, r5 = call("POST", "/games/jgg", {"idempotency_key": goodk}, token=tok)
bal5 = int(sval("SELECT spark_balance FROM users WHERE id = %d" % uid))
st5b, r5b = call("POST", "/games/jgg", {"idempotency_key": goodk}, token=tok)
bal5b = int(sval("SELECT spark_balance FROM users WHERE id = %d" % uid))
# 扣票那一行的键就是 idem_key 拼出来的 `game-jgg:<uid>:<客户端键>`；
# 派彩是另一行（`game-jgg-win:…`），不计进「这一注扣了几次」。
spend = "game-jgg:%d:%s" % (uid, goodk)
rows5 = int(sval("SELECT count(*) FROM spark_ledger WHERE idempotency_key = "
                 "'%s'" % spend))
ok("GS5 合法载荷打一局 200（余额 %d→%d），同键重放被拒（http=%s %s）、"
   "余额不再变（%d→%d）、扣票流水恰 1 行"
   % (bal4, bal5, st5b, str((r5b or {}).get("message") or "")[:24],
      bal5, bal5b),
   (r5 or {}).get("code") == 0 and st5b == 400 and rows5 == 1
   and bal5b == bal5,
   "正例被挡＝严格化过头；重放又扣＝幂等没接上（旧随机键写法正是这样双扣）")

print("== GS6 收尾与残留 ==")
sval("UPDATE users SET status = 2 WHERE id = %d" % uid)
pat = "%" + MARK + "%"
left = int(sval("SELECT count(*) FROM spark_ledger WHERE idempotency_key "
                "LIKE '%s'" % pat))
ok("GS6 探针已停用（status=%s）；本轮前缀 %s 的流水残留 %d 行（有意留作账面）"
   % (sval("SELECT status FROM users WHERE id = %d" % uid), MARK, left),
   sval("SELECT status FROM users WHERE id = %d" % uid) == "2",
   "流水是历史不删；探针号停用即可，余额 %d 随号一起退役" % bal5)

summary()
