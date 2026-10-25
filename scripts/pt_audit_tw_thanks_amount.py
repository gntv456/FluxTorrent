"""感谢口（POST /torrents/{id}/thanks）的数额、未知键与准入门闸门（TW，P-644）。

三条病形同属「回 200 但做的事是另一件」：

1. 白名单判定写成 `amount > 0 && !valid` ⇒ **负数一路放行**：
   `{"amount": -100}` 走完整个口，静默落成一条「免费感谢」并回 200。
2. 请求体形状是 `Option<web::Json<ThankBody>>` ⇒ actix 把**一切解析错误折成
   `Ok(None)`**（本仓实测过的 prod-layer 陷阱），于是 `deny_unknown_fields`
   根本没机会生效：`{"amountt": 100}` 静默降级成免费感谢回 200。
3. 注解写着「种子不存在或不可见（code 1004）」，但实现只经 `owner_of` 读种主
   ⇒ 任意会员可往**别人待审/被驳回的种**上落一行感谢（P-643 同族）。

判据分两层：静态钉形态（提取器形状 / 带符号的白名单 / assert_visible 在册），
活体只打**必被拒**的载荷并核对「零写入」——thanks 行数、火花流水、当事人余额
前后一字不差；成功路径要真转一笔魔力，不给现网留夹具，故不演。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, ok, summary  # noqa: E402
from ci_reg_common import jwt, psql  # noqa: E402

SRC = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   "apps", "api", "src")
REL = os.path.join("torrent_http", "interact.rs")
LITERAL = "答谢数额需为 1/10/100/500/1000/10000"


def src(rel):
    return open(os.path.join(SRC, rel), encoding="utf-8",
                errors="replace").read()


def body_of(text, fn_name):
    """handler 函数体（剥 // 注释），用于判形态而不是全文共现。"""
    m = re.search(r"async fn %s\s*\(" % fn_name, text)
    if not m:
        return ""
    tail = text[m.start():]
    nxt = re.search(r"\n#\[post|\n#\[get|\n#\[put|\n#\[delete", tail[1:])
    seg = tail[:nxt.start() + 1] if nxt else tail
    return "\n".join(l.split("//")[0] for l in seg.split("\n"))


text = src(REL)
m = re.search(r"struct ThankBody\b", text)
blk = text[max(0, m.start() - 400):m.start() + 300] if m else ""
than = body_of(text, "do_thank")
print("== S1 静态形态 ==")
ok("TW1 ThankBody 带 deny_unknown_fields", "deny_unknown_fields" in blk,
   "找不到结构体或属性被摘" if not blk else blk[-160:])
ok("TW2 do_thank 用 OptJson（Option<web::Json<_>> 会把解析错误折成 None）",
   "Option<web::Json" not in than and "OptJson<ThankBody>" in than,
   than[:120])
ok("TW3 白名单判定带符号（amount != 0 而不是 amount > 0）",
   re.search(r"amount\s*!=\s*0\s*&&\s*!super::thanks_spark::is_valid_amount",
             than) is not None)
ok("TW4 do_thank 走统一准入门 assert_visible（P-643 同族）",
   "assert_visible" in than)
ok("TW5 校验文案仍是登记过的那句（不新增 i18n 欠账）", LITERAL in text)
ok("TW6 数额白名单本体没被动过（1/10/100/500/1000/10000）",
   "matches!(amount, 1 | 10 | 100 | 500 | 1000 | 10000)"
   in src(os.path.join("torrent_http", "thanks_spark.rs")))

print("== S7 活体前置：夹具与令牌必须活着 ==")
viewer = int(psql("SELECT id FROM users WHERE class_id < 90 AND status < 2 "
                  "AND id <> 1 ORDER BY id DESC LIMIT 1") or 0)
# 一案一颗种：上一版所有案共用一颗种，被**接受**的那一案先落了一行 thanks，
# 后面的案全部撞 5002「已感谢过」——那是被上一条带崩的级联红，不是各自的判据。
tgts = [int(x) for x in psql(
    "SELECT id FROM torrents WHERE approval_status = 1 AND owner_id <> %d "
    "ORDER BY id DESC LIMIT 6" % viewer).splitlines() if x.strip()]
hidden = int(psql("SELECT t.id FROM torrents t JOIN users u ON u.id = t.owner_id "
                  "WHERE t.approval_status IN (0, 2) AND u.class_id < 90 "
                  "AND t.owner_id <> %d ORDER BY t.id DESC LIMIT 1" % viewer) or 0)
ok("TW7 夹具取到（低档会员 / 过审种 >=5 颗 / 对本人不可见的种）",
   bool(viewer) and len(tgts) >= 5 and bool(hidden),
   "viewer=%s 种数=%s hidden=%s" % (viewer, len(tgts), hidden))
vtok = jwt(viewer, cls=1)
# 令牌活性先行断言：上一版这里出过「跑到一半会员被别的 tick 停用 ⇒ 全段 2001」，
# 那是夹具死了而不是产品坏了，必须当场认出来，不许让后半段拿 2001 判绿。
st_me, b_me = call("GET", "/torrents?limit=1", token=vtok)
ok("TW8 探针令牌可用（打公共口要回 code=0，回 2001 就是号死了）",
   b_me.get("code") == 0, "http=%s code=%s msg=%s" % (
       st_me, b_me.get("code"), str(b_me.get("message"))[:60]))
if not (viewer and hidden) or len(tgts) < 5 or b_me.get("code") != 0:
    summary()

tgt = tgts[0]
owner = int(psql("SELECT owner_id FROM torrents WHERE id = %d" % tgt) or 0)


def read(tid):
    return {
        "thanks": psql("SELECT count(*) FROM thanks WHERE torrent_id = %d" % tid),
        "ledger": psql("SELECT count(*) FROM spark_ledger WHERE ref_id = %d"
                       % tid),
    }


pre = read(tgt)
pre["bal"] = psql("SELECT spark_balance FROM users WHERE id = %d" % viewer)
pre["hid_thanks"] = read(hidden)["thanks"]
print("   前置读数：种主=%d %s" % (owner, pre))


def post(path, body, tok):
    st, resp = call("POST", path, body=body, token=tok)
    resp = resp if isinstance(resp, dict) else {}
    return st, resp.get("code"), str(resp.get("message") or resp.get("msg") or "")


print("== S9 必被拒的载荷 ==")
CASES = [
    ("负数数额", {"amount": -100}, "whitelist"),
    ("拼错的键", {"amountt": 100}, "unknown"),
    ("白名单外的正数", {"amount": 7}, "whitelist"),
    ("类型不对的数额", {"amount": "abc"}, "unknown"),
    # 注意不能演「整个体缺失」：空体 = 免费感谢，那是合法口径（不是缺陷）。
    # 这里演的是**带坏了的体**——`call()` 对 str 走 json.dumps，落成 JSON 标量。
    ("载荷是字符串不是对象", "not-an-object", "unknown"),
]
touched = []
for i, (label, body, kind) in enumerate(CASES):
    tid = tgts[i]
    touched.append(tid)
    st, code, msg = post("/torrents/%d/thanks" % tid, body, vtok)
    if kind == "whitelist":
        cond = code == 1002 and LITERAL in msg
    else:
        cond = code == 1002 and ("字段" in msg or "未识别" in msg
                                 or "格式" in msg or "期望" in msg)
    ok("TW10 %s 被拒（%s）" % (label, {
        "whitelist": "中文白名单", "unknown": "1002 说出哪里坏"}[kind]), cond,
        "http=%s code=%s msg=%s" % (st, code, msg[:90]))
    print("       实际 http=%s code=%s msg=%s" % (st, code, msg[:90]))
st, code, msg = post("/torrents/%d/thanks" % hidden, {"amount": 0}, vtok)
ok("TW10 对不可见的种致谢 被拒（1004 不给写）", code == 1004,
   "http=%s code=%s msg=%s" % (st, code, msg[:90]))

# 收尾：这一族**必须**零写入；万一某案被接受（打旧镜像/判据失效），
# 只删本轮这批种上属于探针号自己的行，别的都不许碰。
for tid in touched:
    stray = psql("SELECT count(*) FROM thanks WHERE torrent_id = %d "
                 "AND user_id = %d" % (tid, viewer))
    if stray != "0":
        psql("DELETE FROM thanks WHERE torrent_id = %d AND user_id = %d"
             % (tid, viewer))
        print("   收尾：删掉本轮自造的 thanks 行（torrent=%d 共 %s 条）"
              % (tid, stray))
residue = [(tid, psql("SELECT count(*) FROM thanks WHERE torrent_id = %d "
                      "AND user_id = %d" % (tid, viewer))) for tid in touched]
ok("TW-CL 残料清零（按 探针号+本轮种号 精确数：%s）" % residue,
   all(c == "0" for _, c in residue))

post_read = {
    "thanks": psql("SELECT count(*) FROM thanks WHERE torrent_id = %d" % tgt),
    "ledger": psql("SELECT count(*) FROM spark_ledger WHERE ref_id = %d" % tgt),
    "bal": psql("SELECT spark_balance FROM users WHERE id = %d" % viewer),
    "hid_thanks": psql("SELECT count(*) FROM thanks WHERE torrent_id = %d"
                       % hidden),
}
for k in ("thanks", "ledger", "bal"):
    ok("TW11 读数没动（%s：%s → %s）" % (k, pre[k], post_read[k]),
       pre[k] == post_read[k], "前=%s 后=%s" % (pre[k], post_read[k]))
ok("TW12 那颗隐种上没有被落感谢行（%s → %s）" % (
    pre["hid_thanks"], post_read["hid_thanks"]),
   pre["hid_thanks"] == post_read["hid_thanks"])

print("   读数出处：BASE=%s" % BASE)
summary()
