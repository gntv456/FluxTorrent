# -*- coding: utf-8 -*-
"""EI 常驻闸门：写口对「拼错的键名」必须报错（P-310 / P-311 同族）

来历（2026-10-14~15 批一百零二/一百零五实测）：
  serde 默认忽略未知字段 ⇒ 客户端一个笔误不会被任何一层拦住，而是落到结构体
  里那个 `Option` 的缺省分支。缺省分支恰好是**反义**动作时，接口回 200 却把
  数据改成用户没要的东西。

现网红证（2026-10-15 批一百零五，127.0.0.1:8080，root 只持有本闸门自己插入的
user_medals 行，收尾删除；两条 shop 探针都在扣款之前 4xx，无流水）：
  PUT /medals/wear {"id":1,"wear":true}          -> 200 {"cleared":true}
                                                   wearing t->f（被摘光）
  PUT /medals/wear {"medal_id":1,"wear_it":true} -> 200，wearing f->t（toggle）
  POST /shop/buy {"item_id":18,"idempotency":k}  -> 1002「缺少 idempotency_key」
                                                   （明明给了，只是名字错）
  POST /shop/buy {"item_id":不存在,"idempotency":k} -> 1004 资源不存在
修后三处都走 path_cfg::humanize_parse_error 的未知字段出口：
  「参数校验失败: 含未识别的字段：id（请核对字段名）」

P-310（同一族的第一处）是 PUT /messages/{…} 的 MoveReq：面板「移动到盒子」把
box_id 当字段名发给只认 folder 的接口 ⇒ 消息一动不动、却回 200。

已知未收口（本轮只登记，不代改）：全量普查 85 个候选请求结构体、17 个高危，
逐条见 _doc/资深PT用户实测审计 批一百零五小节。本轮收口 4 只（WearReq、
BuyReq、MoveReq、TopicManageBatchReq），其余高危留在台账里等站方定夺——
其中管理类写口一旦 deny，前台动态拼载荷的调用点要逐个核。
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SF = os.path.join(ROOT, "apps", "api", "src")

# 现网 latest 还没含 41206fa8 ⇒ 活体三条判据本轮必然红，钉桩可撤
KNOWN_PENDING_DEPLOY = True
SRC_TAG = "probe_ei"


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode != 0:
        print("   psql 失败：%s / %s" % (sql[:70], p.stderr.strip()[:200]))
        return ""
    return p.stdout.strip()


def src(rel):
    """剥 // 注释再返回：静态判据不许被注释里的键名洗白"""
    p = os.path.join(SF, rel)
    if not os.path.exists(p):
        return ""
    out = []
    for line in open(p, encoding="utf-8", errors="replace").read().splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def denies(body, struct):
    """判「struct 声明前是否紧贴 #[serde(deny_unknown_fields)]」"""
    i = body.find("struct %s {" % struct)
    if i < 0:
        return None
    head = body[:i].rstrip().splitlines()[-3:]
    return any("deny_unknown_fields" in h for h in head)


print("== S1 代码层：三只已收口的请求结构体 ==")
md = src("community_http/medal.rs")
sh = src("economy_http/shop.rs")
mb = src("community_http/message_box.rs")
ok("EI1 /medals/wear 的 WearReq 拒未知字段（P-311）",
   denies(md, "WearReq") is True, str(denies(md, "WearReq")))
ok("EI2 /shop/buy 的 BuyReq 拒未知字段（P-311）",
   denies(sh, "BuyReq") is True, str(denies(sh, "BuyReq")))
ok("EI3 /messages/move 的 MoveReq 拒未知字段（P-310）",
   denies(mb, "MoveReq") is True, str(denies(mb, "MoveReq")))
ok("EI4 未知字段有中文出口，不会把 serde 原文糊到界面上（path_cfg）",
   "unknown field" in src("path_cfg.rs")
   and "含未识别的字段" in src("path_cfg.rs"))
ok("EI5 /forums/topics/manage-batch 拒未知字段（P-311 同族：面板发 "
   "{ids, ...op}，op 键名拼错 ⇒ COALESCE 原值写回、界面照报「已处理 N 条」）",
   denies(src("community_http/topic_admin.rs"), "TopicManageBatchReq") is True,
   str(denies(src("community_http/topic_admin.rs"), "TopicManageBatchReq")))

print("== S2 活体：笔误键必须被拒且不留副作用 ==")
# 前置的口径改了。原写法要求 root「开跑前不拥有任何勋章」，以便收尾时
# 能用「数量回到 0」验证清干净。但批一百五十四修了 P-342（medals.active 不存在，
# 自动成就 job 从未成功运行过）之后，**root 正确地自动获得了勋章**（「初次播种」
# 与「勤悟园丁」，与它实际 10+ 个种子一致）。那是产品正确行为，不能删掉去让
# 前置过（也不应该让闸门驱逐真人的勋章）。
# 改成按探针标记判：只计本闸门自己造的那几行。
_root = psql("SELECT id FROM users WHERE username = 'root'")
before_all = psql("SELECT count(*) FROM user_medals WHERE user_id = %s" % _root)
ok("S0 前置：记下 root 开跑前的勋章行数（本闸只动自己的那几行）",
   before_all.isdigit(), "before=%s" % before_all)
MID = psql("SELECT id FROM medals ORDER BY id LIMIT 1")
ok("S1 取到探针勋章", MID.isdigit(), "medal_id=%s" % MID)
tok = login()
ok("S2 root 登录", bool(tok))
psql("INSERT INTO user_medals(user_id, medal_id, source, wearing) "
     "VALUES ((SELECT id FROM users WHERE username = 'root'), %s, '%s', false)"
     % (MID, SRC_TAG))


def wearing():
    return psql("SELECT wearing FROM user_medals um JOIN users u"
                " ON u.id = um.user_id WHERE u.username = 'root'"
                " AND um.medal_id = %s AND um.source = '%s'"
                % (MID, SRC_TAG))


s, r = call("PUT", "/medals/wear",
            {"medal_id": int(MID), "wear": True}, token=tok)
ok("EI6 正控：正确键名戴得上（缺它则 EI7/EI9 的「没改动」不可信）",
   s == 200 and r.get("code") == 0 and wearing() == "t",
   "%s %s wearing=%s" % (s, json.dumps(r, ensure_ascii=False)[:110], wearing()))

s, r = call("PUT", "/medals/wear", {"id": int(MID), "wear": True}, token=tok)
blob = json.dumps(r, ensure_ascii=False)
ok("EI7 笔误键 id 被拒并点名（修前：200 cleared=true 把佩戴清空）",
   s == 400 and r.get("code") == 1002 and "id" in blob and "未识别" in blob,
   "%s %s" % (s, blob[:150]))
ok("EI8 笔误不留副作用（EI6 戴上的还戴着）", wearing() == "t", wearing())

psql("UPDATE user_medals SET wearing = false FROM users u "
     "WHERE user_medals.user_id = u.id AND u.username = 'root' "
     "AND medal_id = %s AND source = '%s'" % (MID, SRC_TAG))
s, r = call("PUT", "/medals/wear", {"medal_id": int(MID), "wear_it": True},
            token=tok)
blob = json.dumps(r, ensure_ascii=False)
ok("EI9 笔误键 wear_it 被拒并点名（修前：走 toggle，戴/摘反着来仍回 200）",
   s == 400 and r.get("code") == 1002 and "wear_it" in blob,
   "%s %s" % (s, blob[:150]))
ok("EI10 笔误不留副作用（未被 toggle 翻成戴）", wearing() == "f", wearing())

REAL = psql("SELECT id FROM shop_items WHERE price > 0 ORDER BY price LIMIT 1")
s, r = call("POST", "/shop/buy", {"item_id": int(REAL), "idempotency": "x"},
            token=tok)
blob = json.dumps(r, ensure_ascii=False)
ok("EI11 幂等键写成 idempotency：要说「字段不认识」而不是「你没给键」"
   "（修前：1002 缺少 idempotency_key）",
   s == 400 and "idempotency" in blob and "缺少" not in blob,
   "%s %s" % (s, blob[:150]))
s, r = call("POST", "/shop/buy",
            {"item_id": int(psql("SELECT max(id) + 1 FROM shop_items")),
             "idempotency": "x"}, token=tok)
ok("EI12 不存在的 SKU 也要先报字段名而不是 1004（反序列化在业务之前）",
   s == 400 and "idempotency" in json.dumps(r, ensure_ascii=False),
   "%s %s" % (s, json.dumps(r, ensure_ascii=False)[:150]))
NOSKU = int(psql("SELECT max(id) + 1 FROM shop_items"))
s3, r3 = call("POST", "/shop/buy",
              {"item_id": NOSKU, "idempotency_key": "probe-ei"},
              token=tok)
ok("EI13 正控：正确键名的合法载荷照过反序列化（红在业务 1004，不是拼错）",
   r3.get("code") == 1004,
   "%s %s" % (s3, json.dumps(r3, ensure_ascii=False)[:120]))

RU = psql("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
          "'%probe-ei%' OR idempotency_key = 'x'")
ok("EI14 探针零扣款（两条 shop 判据都在 spend 之前 4xx）", RU == "0",
   "流水=%s" % RU)

psql("DELETE FROM user_medals WHERE source = '%s'" % SRC_TAG)
ok("EI15 收尾：探针行已删（user_medals 无残留）",
   psql("SELECT count(*) FROM user_medals WHERE source = '%s'"
   % SRC_TAG) == "0")

if KNOWN_PENDING_DEPLOY:
    print("   WARN 钉桩可撤：现网 latest 一旦含 41206fa8，把 "
          "KNOWN_PENDING_DEPLOY 改 False（活体判据此时应转绿）")
print("   基址 %s" % BASE)
summary()
