# -*- coding: utf-8 -*-
"""KR 闸门：JSON 请求体上限必须「显式选定 + 文案诚实 + 声明区间可达」。

病形一（仪器层，2026-10-23 实测）：`path_cfg::json_config()` 从来没写
`.limit()`，全站 JSON 请求体的上限其实是 actix 的 `DEFAULT_LIMIT = 2MiB`
——一个没人做过的决定。现网取证（:8080，同一口只改体大小）：

    body=2097152 → 进 handler（回「缺少必填字段：wasm_b64」）
    body=2097162 → 400 code=1002 message=「参数校验失败: 请求体读取失败」

病形二（文案撒谎）：超限原句是
`JSON payload (2097162 bytes) is larger than allowed (limit: 2097152 bytes).`
它被 humanize 里那条 `contains("payload")` 接住，出口写成「请求体读取失败」
——把「太大」演成「读不到」，运维只会去查网络与反代，永远查不到东西。

病形三（声明区间打不到）：`/admin/adapters/install` 写「wasm 需为 1B–2MiB」，
而 `wasm_b64` 走 JSON，base64 按 4/3 膨胀：2MiB 模块 = 2,796,204 字节的体，
在抽取层就被 2MiB 的墙挡掉，handler 那句承诺里 1.5MiB 以上那一段**永远走不到**
（不是「拒得对」，是根本到不了那行拒绝它的代码）。

判据三层：
  静态：上限显式化、超限分支排在通用 payload 分支之前（否则是死码）、
        wasm 上限由传输墙反推（两个数不许各自漂移）、报错文案的档位与常量
        对齐、体积类 const 普查与台账两头对账（PENDING 桩也钉住：将来谁把
        墙抬到能塞下 10 万条域名，这条会红一次提醒撤桩）。
  活体：墙两侧 A/B（超 512 字节 → 400「请求体过大」并报出两个字节数；
        差 512 字节 → 401，说明体被完整收下、走到了鉴权）；1.3MiB 的内容包
        必须走到应用层的 1MiB 闸——证明传输层与业务层两把尺各在其位。
  收尾：探针不改 adapters / content_packs 的行数（库未留痕）。
正控：同一条 wasm 可达性判据跑在 CTRL_REV（修复前那个提交）的
      adapter_http.rs 上必须报洞。

P-638 续账（同一把尺的第二处撒谎，已收口）：`/admin/email-domains/import`
原先写死「单批 10 万条」——按 `normalize()` 允许的 253 字符域名算，最坏包体
25.6MB，那种请求在抽取层就被墙拒掉，连 handler 里那句「单次导入上限」都执行
不到。现在批数上限由墙反推 `(JSON_BODY_LIMIT − 4KiB) / 256 = 8176`，于是它
**第一次变成打得到的承诺**：KR21b 用 8177 条（体仅约 205KB）当场验负控红在
业务闸而不是传输层，KR21c/d 用 3 条真导入验收单与幂等，KR22/22a 删净并回到
库内基线。文件导入通路（multipart 不受此墙约束）已由 KS 闸门覆盖
（`pt_audit_ks_file_import.py`，同一批的另一支，不在这里假收口）。
"""
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

sys.path.insert(0, "scripts")
from pt_audit_lib import (BASE, call, dig, login, ok, psql_stdin,
                          FAILURES)  # noqa: E402

API = "apps/api/src"
CTRL_REV = "c4f03d8a"  # 本批修复的前一个提交（绝对钉，别用 HEAD~1）
PATH_CFG = API + "/path_cfg.rs"
ADAPTER = API + "/adapter_http.rs"
PACK_IMPORT = API + "/settings_http/pack_import.rs"
# 信封余量：JSON 语法本身 + 其它字段（4KiB 足够宽）
ENVELOPE = 4096


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


def arith(expr):
    """常量右值算成整数。只认十进制与 + - * / 括号（本仓常量就这几个运算符）。

    不用 eval：判据要读的是别人的源码字面量，形状不认识就抛，
    不能把源码里的任意文本交进解释器。
    """
    e = re.sub(r"[\s_]", "", expr)
    toks = re.findall(r"\d+|[()+\-*/]", e)
    if not toks or "".join(toks) != e:
        raise ValueError("常量右值形状不认识：%s" % expr)
    cur = [0]

    def factor():
        t = toks[cur[0]]
        cur[0] += 1
        if t == "(":
            v = additive()
            if cur[0] >= len(toks) or toks[cur[0]] != ")":
                raise ValueError("括号不配对：%s" % expr)
            cur[0] += 1
            return v
        if not t.isdigit():
            raise ValueError("括号里不是数：%s" % expr)
        return int(t)

    def term():
        v = factor()
        while cur[0] < len(toks) and toks[cur[0]] in ("*", "/"):
            op = toks[cur[0]]
            cur[0] += 1
            r = factor()
            v = v * r if op == "*" else v // r
        return v

    def additive():
        v = term()
        while cur[0] < len(toks) and toks[cur[0]] in ("+", "-"):
            op = toks[cur[0]]
            cur[0] += 1
            r = term()
            v = v + r if op == "+" else v - r
        return v

    v = additive()
    if cur[0] != len(toks):
        raise ValueError("尾部有多余符号：%s" % expr)
    return v


def const_of(src, name, subs=None):
    """取 `const NAME: usize = <表达式>;` 的整数值（subs 先做符号替换）。"""
    m = re.search(r"const\s+%s:\s*usize\s*=\s*([^;]+);" % name, src or "")
    if not m:
        return None
    e = m.group(1)
    # 先做符号替换再去下划线：反了会把 JSON_BODY_LIMIT 洗成 JSONBODYLIMIT，
    # 于是替换键再也匹配不上（自造工具的测量假象）
    for k, v in (subs or {}).items():
        e = e.replace(k, str(v))
    return arith(e.replace("_", "").strip())


# ---------------------------------------------------------------- 静态层
src_cfg = read(PATH_CFG) or ""
src_ad = read(ADAPTER) or ""
src_pk = read(PACK_IMPORT) or ""

ok("KR1 json_config 显式设了 .limit()（不许再靠 actix 的隐式默认）",
   re.search(r"web::JsonConfig::default\(\)\s*\.limit\(", src_cfg) is not None,
   "path_cfg.rs 里 .default() 后没紧跟 .limit(")

LIMIT = const_of(src_cfg, "JSON_BODY_LIMIT")
ok("KR2 上限是站级唯一真源且是 MiB 级整数（常量读得出）",
   LIMIT is not None and LIMIT >= 256 * 1024, "JSON_BODY_LIMIT=%r" % LIMIT)

# 超限分支必须排在通用 payload 分支之前，否则永远走不到（死码）
i_big = src_cfg.find("larger than allowed")
i_gen = src_cfg.find('contains("payload")')
ok("KR3 超限分支排在通用 payload 分支之前（否则是死码）",
   0 <= i_big < i_gen, "big=%d gen=%d" % (i_big, i_gen))
seg = src_cfg[i_big:i_big + 700] if i_big >= 0 else ""
ok("KR4 超限出口同时报实测字节数与上限（不许塌成笼统一句）",
   "请求体过大" in seg and "上限" in seg
   and 'digits_after(body, "payload (")' in seg
   and 'digits_after(body, "allowed (limit: ")' in seg, seg[:150])

SUBS = {"crate::path_cfg::JSON_BODY_LIMIT": LIMIT or 0,
        "JSON_BODY_LIMIT": LIMIT or 0}


def wasm_gap(src):
    """wasm 声明区间可达性：base64 后的体必须塞得进传输墙。"""
    w = const_of(src, "MAX_WASM_BYTES", SUBS)
    if w is None or not LIMIT:
        return "常量读不出（wasm=%r 墙=%r）" % (w, LIMIT)
    if w * 4 // 3 + ENVELOPE > LIMIT:
        return "wasm %d 字节 → base64 %d，超传输墙 %d" % (
            w, w * 4 // 3, LIMIT)
    return None


ok("KR5 wasm 上限由传输墙反推（两个数不许各自漂移）",
   "crate::path_cfg::JSON_BODY_LIMIT" in src_ad,
   "MAX_WASM_BYTES 没引用 JSON_BODY_LIMIT，改墙不会带动它")
gap = wasm_gap(src_ad)
ok("KR6 wasm 声明区间打得到（base64 膨胀 4/3 后仍在墙内）",
   gap is None, gap or "")
WASM_FULL = const_of(src_ad, "MAX_WASM_BYTES", SUBS) or 0

# 报错文案承诺的档位必须等于常量（防「文案 2MiB / 实际只能 1.5MiB」复发）
m = re.search(r'wasm 需为 1B.(\d+(?:\.\d+)?)(MiB|KiB)?', src_ad)
claimed = None
if m:
    claimed = int(float(m.group(1)) * (1024 * 1024 if m.group(2) == "MiB"
                                       else 1024 if m.group(2) == "KiB"
                                       else 1))
ok("KR7 安装口文案的档位与常量一致（不许承诺打不到的区间）",
   claimed == WASM_FULL, "文案=%r 常量=%r" % (claimed, WASM_FULL))

ctrl = read(ADAPTER, CTRL_REV)
ok("KR8 正控：%s 那版必须被 KR6 报洞（判据承重）" % CTRL_REV,
   ctrl is not None and wasm_gap(ctrl) is not None,
   "该版不红＝正控失效；读不到旧文件也算失效")

# 内容包的业务闸仍在传输墙之内（两把尺各在其位）
m2 = re.search(r"payload\.to_string\(\)\.len\(\)\s*>\s*([\d\s_*/()]+)",
               src_pk)
PACK_CAP = arith(m2.group(1)) if m2 else None
ok("KR9 内容包 payload 上限存在且塞得进墙（约一倍余量）",
   PACK_CAP is not None and PACK_CAP + ENVELOPE <= (LIMIT or 0),
   "payload 上限=%r 墙=%r" % (PACK_CAP, LIMIT))

# 体积类普查：候选 = `const 名字: usize = ≥5 位数字;`
CAND_RE = re.compile(r"const\s+([A-Z_]+):\s*usize\s*=\s*(\d[\d_]{4,})\s*;")
# 每条候选写死「最坏合法包体」怎么算（per 是每个元素的字节数），两头对账
LEDGER = {
    "publish_http/upload_precheck.rs::DESCR_MAX": (
        1, "发种描述本身就是字节总量，60_000 在墙内"),
    "staff_http/wiki.rs::WIKI_BODY_LIMIT": (
        1, "wiki 正文 200_000 字节上限，在墙内"),
    "torrents/comments.rs::COMMENT_MAX_CHARS": (
        3, "评论 20_000 字，汉字按 3 字节折最坏 60_000，在墙内"),
    "http/auth_infra.rs::CAP": (
        0, "非请求体：进程内缓存容量（登记只为普查不留白）"),
    "admin_http/email_domains_file.rs::MAX_DOMAINS": (
        256, "非 JSON 通路：multipart 流式导入的条数天花板，不受本墙约束"
        "（站方 7a92f375 落的新口，10 万条逐条 INSERT，上限按时长与病态输入定）"),
    "admin_http/email_domains_file.rs::BATCH": (
        256, "非 JSON 通路：每批条数（一批一个独立事务），与请求体大小无关"),
}
out = sh(["git", "grep", "-l", "-E",
          r"const\s+[A-Z_]+:\s*usize\s*=\s*[1-9][0-9_]", "--", API])
cands = {}
for rel in (out.stdout or "").splitlines():
    rel = rel.strip()
    if not rel.endswith(".rs"):
        continue
    for name, val in CAND_RE.findall(read(rel) or ""):
        cands["%s::%s" % (rel.replace(API + "/", ""), name)] = int(
            val.replace("_", ""))
keys = set(cands)
unreg = sorted(keys - set(LEDGER))
stale = sorted(set(LEDGER) - keys)
ok("KR10 每个体积类常量都在台账里写了最坏包体算法", not unreg,
   "%d 条未登记：%s" % (len(unreg), unreg[:6]))
ok("KR11 台账没过期条目（候选集与台账两头对得上）", not stale,
   "%d 条已不在候选：%s" % (len(stale), stale[:6]))
ok("KR12 普查基数可信（≥ 4 个体积类常量在册）", len(keys) >= 4,
   "基数=%d %s" % (len(keys), sorted(keys)))
bad_fit = [k for k in keys
           if k in LEDGER and not LEDGER[k][1].startswith("PENDING")
           and not LEDGER[k][1].startswith("非请求体")
           and not LEDGER[k][1].startswith("非 JSON 通路")
           and cands[k] * LEDGER[k][0] + ENVELOPE > (LIMIT or 0)]
ok("KR13 在册项的最坏包体都塞得进墙（待裁桩与 multipart 通路除外）",
   not bad_fit,
   "超墙：%s" % bad_fit[:4])
pending = [k for k in LEDGER if LEDGER[k][1].startswith("PENDING")]
ok("KR14 待裁桩不悬空（PENDING 条目必须仍然超墙，否则就地撤桩）",
   all(cands.get(k, 0) * LEDGER[k][0] + ENVELOPE > LIMIT for k in pending),
   "已塞得进墙却还挂着待裁：%s" % pending)

# P-638 收口：域名库单批导入上限必须由墙反推，不许再声明送不到的一批
EMAIL = API + "/admin_http/email_domains.rs"
src_em = read(EMAIL) or ""
PER_DOMAIN = 256  # normalize() 判域名最长 253，JSON 里再加引号 2 + 逗号 1


def domain_batch_overshoots(src):
    """批数 × 最坏单条 + 信封 是否超墙（反推式与硬编码都按这一把尺量）。"""
    v = const_of(src, "IMPORT_MAX", SUBS)
    if v is None:
        return "IMPORT_MAX 读不出"
    if v * PER_DOMAIN + ENVELOPE > LIMIT:
        return "单批 %d 条最坏 %d 字节 > 传输墙 %s" % (
            v, v * PER_DOMAIN, LIMIT)
    return None


overshoots = domain_batch_overshoots(src_em)
BATCH_MAX = const_of(src_em, "IMPORT_MAX", SUBS) or 0
ok("KR14b 域名库批数上限塞得进墙（按最坏单条 256 字节算，非均值）",
   overshoots is None, overshoots or "")
ok("KR14c 批数上限由墙反推（改墙会自动跟着走，不许两处各写各的）",
   "crate::path_cfg::JSON_BODY_LIMIT" in src_em,
   "IMPORT_MAX 没引用 JSON_BODY_LIMIT")
ctrl_em = read(EMAIL, CTRL_REV)
ok("KR8b 正控：%s 那版写死 10 万条必须被 KR14b 报洞（判据承重）" % CTRL_REV,
   ctrl_em is not None
   and domain_batch_overshoots(ctrl_em) is not None,
   "该版不红＝正控失效")

print()
if os.environ.get("KR_SKIP_LIVE"):
    print("（KR_SKIP_LIVE=1：跳过活体层）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
print("=" * 68)
print("二、活体 A/B（打的是 %s）" % BASE)
print("=" * 68)


def post_bytes(path, data, token=None):
    req = urllib.request.Request(BASE + path, method="POST")
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        r = urllib.request.urlopen(req, data, timeout=90)
        return r.status, json.loads(r.read().decode("utf-8", "replace")
                                    or "{}")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read().decode("utf-8", "replace")
                                  or "{}")
    except Exception as e:  # noqa: BLE001
        return -1, {"message": type(e).__name__ + " " + str(e)[:120]}


def pad_to(size):
    """精确到字节的 JSON 体（pad 字段被 serde 忽略，只用来撑长度）。

    必须带齐 wasm_b64：少了它在抽取层就因「缺字段」失败（400），墙内侧那条
    「体被完整收下、走到了鉴权」的断言永远读不到 401，A/B 会塌成两条同红。
    """
    head = ('{"adapter_id":"kr","kind":"metadata","name":"kr",'
            '"wasm_b64":"AAAA","pad":"')
    return (head + "a" * max(0, size - len(head) - 2) + '"}').encode()


EP = "/admin/adapters/install"
before = psql_stdin(
    "SELECT (SELECT count(*) FROM adapters)||'|'||"
    "(SELECT count(*) FROM content_packs)")
dom0 = psql_stdin("SELECT count(*) FROM disposable_email_domains")
tok, probes = None, []
try:
    st, js = post_bytes(EP, pad_to(LIMIT + 512))
    msg = dig(js, "message") or ""
    mm = re.search(r"请求体过大：(\d+) 字节，上限 (\d+) 字节", msg)
    ok("KR15 超墙体被拒且文案诚实（报实测字节数与上限，不再谎称读失败）",
       st == 400 and dig(js, "code") == 1002 and mm is not None
       and "读取失败" not in msg,
       "http=%s code=%s msg=%s" % (st, dig(js, "code"), msg[:120]))
    ok("KR16 报出的两个数是真的（实测 > 上限，上限 = 常量）",
       mm is not None and int(mm.group(1)) > int(mm.group(2))
       and int(mm.group(2)) == LIMIT,
       "读数=%s 常量=%s" % (mm.groups() if mm else None, LIMIT))
    ok("KR17 状态码守站内口径（校验族只认 400，不冒 413/500）",
       st == 400, "http=%s" % st)

    st2, js2 = post_bytes(EP, pad_to(LIMIT - 512))
    ok("KR18 墙内侧不误伤：差 512 字节的体被完整收下并走到鉴权（401）",
       st2 == 401 and "请求体过大" not in (dig(js2, "message") or ""),
       "http=%s msg=%s" % (st2, (dig(js2, "message") or "")[:120]))

    # 端到端的「声明区间可达」：按文案承诺的上限装满 base64 再发一次
    full = 4 * ((WASM_FULL + 2) // 3)
    body = ('{"adapter_id":"kr","kind":"metadata","name":"kr",'
            '"wasm_b64":"%s"}' % ("a" * full)).encode()
    st4, js4 = post_bytes(EP, body)
    ok("KR19 满格 wasm（%d 字节→b64 %d）的体进得了 handler（KR6 的线上证）"
       % (WASM_FULL, full),
       st4 == 401 and "请求体过大" not in (dig(js4, "message") or ""),
       "http=%s 体=%d 墙=%s msg=%s" % (
           st4, len(body), LIMIT, (dig(js4, "message") or "")[:110]))

    tok = login()
    pack = {"format": "fluxtorrent.contentpack", "version": 1,
            "kind": "theme", "pack_id": "kr.probe.body",
            "payload": {"settings": {"site_desc": "x" * 1_300_000}}}
    st3, js3 = call("POST", "/admin/content-packs/import",
                    {"pack": pack, "confirm": False}, tok)
    msg3 = dig(js3, "message") or ""
    body_len = len(json.dumps(pack).encode())
    ok("KR20 夹具确实夹在两把尺之间（1MiB 业务闸 < 体 < 传输墙）",
       PACK_CAP < body_len < LIMIT,
       "体=%d 业务闸=%s 墙=%s" % (body_len, PACK_CAP, LIMIT))
    ok("KR21 这个体走到了应用层的包体闸而不是传输层（两层分工成立）",
       "1MiB" in msg3 and "请求体过大" not in msg3,
       "http=%s msg=%s" % (st3, msg3[:140]))

    # P-638 的线上证：批数上限现在**打得到**（负控红在业务闸，不撞墙）
    probes = ["kr-probe-%d.invalid" % i for i in range(3)]
    over = ["kr-probe-%d.invalid" % i for i in range(BATCH_MAX + 1)]
    ok("KR21a 负控夹具本身成立（超一批的条数对、且体在墙内＝只能红在业务闸）",
       BATCH_MAX > 0 and len(over) == BATCH_MAX + 1
       and len(json.dumps({"domains": over})) < LIMIT,
       "批上限=%d 条数=%d 体=%d 墙=%s" % (
           BATCH_MAX, len(over), len(json.dumps({"domains": over})), LIMIT))
    st5, js5 = call("POST", "/admin/email-domains/import",
                    {"domains": over}, tok)
    msg5 = dig(js5, "message") or ""
    ok("KR21b 超一批必须红在业务闸并点名上限（而不是传输层的拒收）",
       st5 == 400 and "单次导入上限 %d 条" % BATCH_MAX in msg5
       and "请求体过大" not in msg5,
       "http=%s msg=%s" % (st5, msg5[:140]))
    s6, j6 = call("POST", "/admin/email-domains/import",
                  {"domains": probes}, tok)
    ok("KR21c 一批内真收：3 条探针域名 added=3",
       s6 == 200 and dig(j6, "data", "added") == 3,
       "http=%s 回执=%s" % (s6, dig(j6, "data") or j6))
    s7, j7 = call("POST", "/admin/email-domains/import",
                  {"domains": probes}, tok)
    ok("KR21d 幂等：同批再导 added=0、skipped=3（不重复建行）",
       s7 == 200 and dig(j7, "data", "added") == 0
       and dig(j7, "data", "skipped") == 3,
       "http=%s 回执=%s" % (s7, dig(j7, "data") or j7))
finally:
    # 收尾只用已记录的 tok / probes：崩溃在半路时也不许在 finally 里
    # 二次登录或读未定义变量（那样红账会被 NameError 盖掉）
    if tok and probes:
        call("POST", "/admin/email-domains/delete",
             {"domains": probes}, tok)
    left_dom = psql_stdin(
        "SELECT count(*) FROM disposable_email_domains "
        "WHERE domain ~ '^kr-probe-[0-9]+[.]invalid$'")
    ok("KR22 收尾：探针域名已删净（残留=%s 条）" % left_dom,
       left_dom == "0", "残留=%s" % left_dom)
    dom1 = psql_stdin("SELECT count(*) FROM disposable_email_domains")
    ok("KR22a 库内域名数回到跑前基线（前=%s 后=%s）" % (dom0, dom1),
       dom0 == dom1, "前=%s 后=%s" % (dom0, dom1))
    after = psql_stdin(
        "SELECT (SELECT count(*) FROM adapters)||'|'||"
        "(SELECT count(*) FROM content_packs)")
    ok("KR23 收尾：探针没在库里留痕（adapters/content_packs 行数不变）",
       before == after, "前=%s 后=%s" % (before, after))
    left = psql_stdin(
        "SELECT count(*) FROM content_packs WHERE pack_id LIKE 'kr.probe%'")
    ok("KR23b 回读未登记任何 kr.probe 包", left == "0", "残留=%s" % left)

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
