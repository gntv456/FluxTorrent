# -*- coding: utf-8 -*-
"""CW 闸门：限长校验的「按字节 vs 按字」一致性（P-128 同族一次性收口）。

病形（批一百五十三，2026-10-19 实测）：`x.len()` 数的是 UTF-8 字节，而校验文案
对成员承诺的是「字/长度/上限」⇒ 中文用户实际只有三分之一的额度。先例早就立过
（批三十八 P-128 把举报理由改成 `chars().count()`；注册口用户名单前就按字符，
domain/mod.rs:107），但会员面还有一批口没跟上。

现网取证（:8080）：POST /wishlist 30 个汉字（90 字节）收、34 个汉字（102 字节）
当场 400「关键词长度 1-100」——而 components/torrent-actionbar-mount.tsx:67 是
拿**种名**当关键词提交的，中文长名点一下就失败。

本批按字判收了 14 处：心愿单关键词 / 聊天室发言 / 合集名 / 组名 / 作弊申诉正文
（两口）/ 案件备注 / 站名申请用户名与理由 / 改名 / 字幕标题 / 申诉内容下限 /
复核与处置说明 / API Token 备注名。

判据两层：
  静态普查：`.len()` 与数字比较，且**它自己那条**报错文案（同 if 块向后 4 行）
    承诺了字长（含 字/长度/上限/需/≤/至少，且不是「字节」、不是「N 个/条/项」
    计数）⇒ 候选。候选必须逐条在 LEDGER 里写明为什么可以按字节；台账与候选
    两头对账（多一条红、少一条也红），新写的中文文案口就逃不掉。
  活体 A/B：心愿单 40 个汉字必须收、101 个汉字必须挡；聊天室 120 个汉字必须收，
    收尾用各自删除口清干净。
正控：普查跑在 CTRL_REV（本批修复的前一个提交）的 wishlist.rs 原文上必须报洞。
"""
import glob
import os
import re
import subprocess
import sys

sys.path.insert(0, "scripts")
from pt_audit_lib import call, login, ok, dig, FAILURES  # noqa: E402

API = "apps/api/src"
CTRL_REV = "4297893a"  # 本批「按字判」修复的前一个提交（绝对钉，别用 HEAD~1）
# 数字支持 200_000 这种写法，否则会把它读成 200
CHECK_RE = re.compile(
    r"([A-Za-z_][\w.()]*)\.len\(\)\s*(<=|>=|<|>)\s*(\d[\d_]*)"
)
MSG_RE = re.compile(r'"([^"\n]{2,90})"')
# 计数语义（N 个/条/项）不是字符长度；「位」不在此列——「至少 8 位」承诺的正是字符
COUNT_RE = re.compile(r"\d\s*(个|条|项)")

LEDGER = {
    # —— Vec 计数：条数/人数上限，与字节/字符无关 ——
    "admin_p3_http/increment_targets.rs::body.user_ids > 500":
        "单批用户 id 个数上限，是 Vec 长度不是字长",
    "http/home_layout.rs::body > 20":
        "首页布局格子个数上限，是数组长度不是字长",
    # —— Idempotency-Key：ASCII 随机串，字节与字符等价 ——
    "admin_http/user_ops.rs::k < 8":
        "Idempotency-Key 是 ASCII 随机串，字节数即字符数",
    "admin_p3_http/increment_bulk.rs::k < 8":
        "Idempotency-Key 是 ASCII 随机串，字节数即字符数",
    "gacha_http/craft.rs::t < 8":
        "Idempotency-Key 是 ASCII 随机串，字节数即字符数",
    "gacha_http/staff.rs::k < 8":
        "Idempotency-Key 是 ASCII 随机串，字节数即字符数",
    "staff_http/ipcheck.rs::k < 8":
        "Idempotency-Key 是 ASCII 随机串，字节数即字符数",
    "staff_http/ipcheck.rs::k >= 8":
        "Idempotency-Key 上限那一半，同上按 ASCII 等价",
    "staff_http/users_ops.rs::k < 8":
        "Idempotency-Key 是 ASCII 随机串，字节数即字符数",
    # —— ASCII 技术标识/代码串 ——
    "adapter_http.rs::adapter_id > 100":
        "适配器 id 是 ASCII 标识符，不会出现多字节",
    "admin_http/embed_rule_check.rs::b.provider > 40":
        "提供方 slug（ASCII），由代码写死不是自然文本",
    "admin_http/embed_rule_check.rs::extra > 200":
        "URL 查询参数模板，限定 ASCII 且文案禁空格",
    "admin_p3_http/promo_kinds.rs::k > 40":
        "促销档位标识（slug），中文名走 label 另一字段",
    "auth_http/passkey_reg.rs::cred_id > 512":
        "base64 credential id，恒 ASCII",
    "domain/terms/mod.rs::k > 64":
        "术语键是 ASCII slug；术语值另有上限",
    "rules_engine.rs::expr > 200":
        "规则表达式是代码串不是自然语言文本",
    "settings_http/pack_format.rs::pack_id > 100":
        "内容包 id（ASCII），来自包文件不是人写文案",
    "staff_http/pack_save.rs::code > 40":
        "custom_ 前缀 slug，校验限定小写字母数字下划线",
    "twofa_http/admin_misc.rs::body.chat_id < 5":
        "Telegram chat_id 是数字串，文案要求纯数字",
    # —— 体积上限：文案没承诺「字数」，按字节是保守防护 ——
    "settings_http/engine.rs::raw > 4096":
        "设置值字节上限（文案只说长度不得超过 4096，未承诺字数）",
    "settings_http/engine.rs::raw > 65535":
        "textarea 设置值字节上限，防超长灌库",
    "settings_http/engine.rs::v > 4096":
        "设置值逐条字节上限，与上面同一条口径",
    "settings_http/pack_import.rs::head.payload.to_string() > 1024":
        "包体 MiB 级体积上限，按字节合理",
    "staff_http/custom_pages.rs::b.body > 200000":
        "自定义页面正文体积上限（200_000 字节）",
    # —— PENDING 待裁：文案写「至少 8 位」而判据数字节 ⇒ 中文密码 3 字就过 ——
    "auth_http/me_security.rs::body.new_password < 8":
        "PENDING：改密下限按字节，收紧会挡掉存量短中文密码，待站长裁",
    "domain/mod.rs::u.password < 8":
        "PENDING：注册密码下限按字节，同上待裁（改法是 8 位=8 字符）",
    "gaps_http/reset.rs::body.new_password < 8":
        "PENDING：重置密码下限按字节，与注册口同一条口径",
    "staff_http/users_ops.rs::body.password < 8":
        "PENDING：管理侧补号初始密码下限按字节",
}


def sh(args):
    return subprocess.run(args, capture_output=True, encoding="utf-8",
                          errors="replace")


def read(rel, rev=None):
    if rev:
        out = sh(["git", "show", "%s:%s" % (rev, rel)])
        return out.stdout if out.returncode == 0 else None
    try:
        with open(rel, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def violations(src_map):
    """候选 = 一条 `.len()` 数字比较 + 它自己的报错文案承诺了字长。"""
    hits = []
    for rel, src in src_map.items():
        if src is None:
            continue
        lines = src.splitlines()
        for i, ln in enumerate(lines):
            m = CHECK_RE.search(ln)
            if not m or int(m.group(3).replace("_", "")) == 0:
                continue  # 与 0 比较 = 判空，字节/字符无关
            window = " ".join(lines[i:i + 5])
            promised = [t for t in MSG_RE.findall(window)
                        if ("字" in t or re.search(
                            r"(长度|上限|至多|最多|需|≤|至少)", t))
                        and "字节" not in t and not COUNT_RE.search(t)]
            if not promised:
                continue
            key = "%s::%s %s %s" % (rel.replace(API + "/", ""), m.group(1),
                                    m.group(2), m.group(3).replace("_", ""))
            hits.append((key, rel, i + 1, promised[0][:44]))
    return hits


def files_with(rev=None):
    out = sh(["git", "grep", "-l", "-E",
              r"\.len\(\)\s*(<=|>=|<|>)\s*[1-9]"]
             + ([rev] if rev else []) + ["--", API])
    rows = {}
    for line in out.stdout.splitlines():
        rel = line.split(":", 1)[1] if rev and ":" in line else line
        if rel.endswith(".rs"):
            rows[rel] = read(rel, rev)
    return rows


print("=" * 68)
print("一、静态普查（文案承诺字长却按字节判）")
print("=" * 68)
base = files_with()
hits = violations(base)
keys = {h[0] for h in hits}
print("普查基数：%d 个文件含 `.len()` 数字比较；候选 %d 处；台账 %d 条"
      % (len(base), len(keys), len(LEDGER)))
for h in sorted(hits)[:8]:
    print("   候选 %-52s:%-5s %s" % (h[1].replace(API + "/", ""), h[2], h[0]))
unreg = sorted(keys - set(LEDGER))
stale = sorted(set(LEDGER) - keys)
ok("CW1 每个候选都在台账里写明为什么可以按字节", not unreg,
   "%d 条未登记：%s" % (len(unreg), unreg[:6]))
ok("CW2 台账没有过期条目（候选集与台账两头对得上）", not stale,
   "%d 条已不在候选里：%s" % (len(stale), stale[:6]))
ok("CW3 普查基数可信（≥ 40 个文件参与，别测在空集合上）",
   len(base) >= 40, "基数=%d" % len(base))
ok("CW4 台账每条都带实质理由（≥ 12 字，不许空转放行）",
   all(len(v) >= 12 for v in LEDGER.values()),
   "过短：%s" % [k for k, v in LEDGER.items() if len(v) < 12][:3])

ctrl_rel = API + "/gaps_http/wishlist.rs"
ctl = violations({ctrl_rel: read(ctrl_rel, CTRL_REV)})
ok("CW5 正控：%s 那版 wishlist.rs 必须被报出（判据承重）" % CTRL_REV,
   any(c[0] == "gaps_http/wishlist.rs::kw > 100" for c in ctl),
   "该版候选=%s" % [c[0] for c in ctl])
ok("CW6 工作树已按字符计数",
   "kw.chars().count() > 100" in (read(ctrl_rel) or ""),
   "wishlist.rs 里找不到 chars().count()")
# 本批收口的口必须已经是 chars（少一个就是「以为修了其实没修」）
FIXED = ["community_http/shoutbox.rs", "publish_http/collections.rs",
         "publish_http/group.rs", "gaps_http/cheat_appeals.rs",
         "http/cheat_case/mod.rs", "admin_p3_http/user_rename.rs",
         "content_http/subtitles_util.rs", "http/cheat_case/sanction.rs",
         "openapi_http/tokens.rs"]
lack = [f for f in FIXED
        if ".chars().count()" not in (read(API + "/" + f) or "")]
ok("CW7 本批点名的按字判口全部落地", not lack, "仍缺 chars：%s" % lack)

print()
if os.environ.get("CW_SKIP_LIVE"):
    print("（CW_SKIP_LIVE=1：跳过活体层）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
print("=" * 68)
print("二、活体 A/B（打的是 %s）" % os.environ.get(
    "FLUX_API_BASE", "http://127.0.0.1:8080/api/v1"))
print("=" * 68)
tok = login()
KW40 = "世纪" * 20              # 40 个汉字 = 120 字节
KW101 = "世" * 101             # 101 个汉字：字超上限，字节 303
SHOUT120 = "今天天气不错" * 20  # 120 个汉字 = 360 字节
made, sid = [], None
try:
    s1, r1 = call("POST", "/wishlist", {"keyword": KW40}, tok)
    ok("CW8 心愿单 40 个汉字必须收（P-128 同族缺陷本体）",
       s1 == 200 and r1.get("code") == 0,
       "http=%s code=%s msg=%s" % (s1, r1.get("code"), r1.get("message")))
    s2, r2 = call("GET", "/wishlist", None, tok)
    made = [it for it in (dig(r2, "data", "items") or [])
            if it["keyword"] == KW40]
    ok("CW8b 收下的关键词原样回显（没被截断）",
       len(made) == 1, "回显=%s" % made)
    s3, r3 = call("POST", "/wishlist", {"keyword": KW101}, tok)
    ok("CW9 上限没被改松：101 个汉字仍须挡（字节 303 却按字拒）",
       s3 == 400 or r3.get("code") == 1002,
       "http=%s code=%s msg=%s" % (s3, r3.get("code"), r3.get("message")))
    s4, r4 = call("POST", "/shoutbox", {"message": SHOUT120}, tok)
    ok("CW10 聊天室 120 个汉字必须收（文案写「发言需 1-300 字」）",
       s4 == 200 and r4.get("code") == 0,
       "http=%s code=%s msg=%s" % (s4, r4.get("code"), r4.get("message")))
    sid = dig(r4, "data", "id")
    if not sid:
        s5, r5 = call("GET", "/shoutbox", None, tok)
        rows = dig(r5, "data") or []
        if isinstance(rows, dict):
            rows = rows.get("items") or rows.get("messages") or []
        for row in rows:
            if isinstance(row, dict) and row.get("message") == SHOUT120:
                sid = row.get("id")
                break
    ok("CW10b 取到探针快讯 id（拿不到就没法收尾，必须红）", bool(sid),
       "id=%r" % sid)
finally:
    for it in made:
        call("POST", "/wishlist/remove", {"id": it["id"]}, tok)
    if sid:
        call("POST", "/shoutbox/delete", {"id": sid}, tok)
    s6, r6 = call("GET", "/wishlist", None, tok)
    left = [it for it in (dig(r6, "data", "items") or [])
            if it["keyword"] in (KW40, KW101)]
    ok("CW11 收尾：探针心愿单已清零（建 %d 删后剩 %d）" % (len(made), len(left)),
       not left, "残留=%s" % left)
    s7, r7 = call("GET", "/shoutbox", None, tok)
    rows = dig(r7, "data") or []
    if isinstance(rows, dict):
        rows = rows.get("items") or rows.get("messages") or []
    leak = [x for x in rows if isinstance(x, dict)
            and x.get("message") == SHOUT120]
    ok("CW12 收尾：探针快讯已从聊天室消失（不在现网留痕）", not leak,
       "残留=%s" % leak)

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
