# -*- coding: utf-8 -*-
"""KS 闸门：域名库「文件导入」通路（P-638 续账）——大清单的正解。

背景：JSON 口的批数上限必须由墙反推（IMPORT_MAX = 8176 条），整张公共
清单一次灌不进去。文件口走 multipart 流式读取，不受 JSON 请求体墙约束；
代价是上限必须自己数，而且**中途失败时前面几批已经提交**——错误文案与
审计都必须说出「已导入多少条」，否则站长会在「以为没导」和「导了一半」
之间猜。

判据三层：
  静态：新口在册且挂载；鉴权与 JSON 口同档且早于读书；两个上限是读取
        过程中数出来的（不是声明）；分批 = 独立事务；复用同一份
        `normalize` 判据；正控钉 a3affae2（文件口出现之前）必须报缺；
        KS17/18 钉界面半：面板文件按钮接到本口（lib 同端点 + CSRF + 三语键）。
  活体：匿名 401 且不落库；3 条真导入；幂等；文件内大小写重复；**体
        超过 JSON 墙（2MiB）仍被完整收下**（这是「不受墙约束」的行为
        证据，不靠嘴说）；条数上限真打得到且有 2000 条已提交时文案点名。
  收尾：探针按本轮专属 marker 精确取残留；库内回到跑前基线。
"""
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

sys.path.insert(0, "scripts")
from pt_audit_lib import (BASE, login, ok, psql_stdin,
                          FAILURES)  # noqa: E402

API = "apps/api/src"
NEW = API + "/admin_http/email_domains_file.rs"
OLD = API + "/admin_http/email_domains.rs"
MOD = API + "/admin_http/mod.rs"
REPO = API + "/repo/email_domains.rs"
CTRL_REV = "a3affae2"  # 文件口出现之前（P-638 那版）
EP = "/admin/email-domains/import-file"
TAG = "kr-probe-file-%08x-" % os.getpid()
LINK = "http://127.0.0.1:8099/api/v1"
WEB_LIB = "apps/web/lib/email-domain-file-import.ts"
WEB_PANEL = "apps/web/components/staff-tools-security-domains.tsx"


def read(rel, rev=None):
    if rev:
        out = subprocess.run(["git", "show", "%s:%s" % (rev, rel)],
                             capture_output=True, encoding="utf-8",
                             errors="replace")
        return out.stdout if out.returncode == 0 else None
    try:
        with open(rel, encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def const(src, name):
    m = re.search(r"const %s: usize = ([\d_]+)" % name, src or "")
    return int(m.group(1).replace("_", "")) if m else None


# ---------- 一、静态 ----------
print("=" * 68)
print("一、静态判据（%s）" % NEW)
print("=" * 68)
src = read(NEW) or ""
mod = read(MOD) or ""
repo = read(REPO) or ""
old = read(OLD) or ""
MAX_DOMAINS = const(src, "MAX_DOMAINS")
MAX_BYTES = const(src, "FILE_MAX_BYTES")

mount = mod.find("pub fn mount_admin")
mounted = mount >= 0 and ".service(email_domains_import_file)" in \
    mod[mount:]
ok("KS1 文件口在册且真挂进 mount_admin（mod/pub use/service 三件齐）",
   "#[post(\"/admin/email-domains/import-file\")]" in src
   and "mod email_domains_file;" in mod
   and "pub use email_domains_file::*;" in mod and mounted,
   "mod=%s use=%s service=%s" % (
       "mod email_domains_file;" in mod,
       "pub use email_domains_file::*;" in mod, mounted))

# 只在 handler 体内比先后：payload.next() 住在 read_file 里，全文 find
# 会把它排到 staff 前面（第一次就误红了一版）
h_at = src.find("async fn email_domains_import_file(")
body = src[h_at:] if h_at >= 0 else ""
i_auth = body.find("staff(&req")
ok("KS2 鉴权与 JSON 口同档，且早于读书（匿名请求不消耗解析预算）",
   i_auth >= 0 and "SETTINGS_MANAGE" in body
   and "require_perm" in body and 0 <= i_auth < body.find("read_file(")
   and src.count("payload.next()") == 1,
   "i_auth=%s i_read=%s" % (i_auth, body.find("read_file(")))

ok("KS3 两个上限都是读取时数出来的（不是声明一个送不到的数）",
   "acc.bytes > FILE_MAX_BYTES" in src
   and "acc.total >= MAX_DOMAINS" in src
   and MAX_DOMAINS is not None and MAX_BYTES is not None,
   "MAX_DOMAINS=%s MAX_BYTES=%s" % (MAX_DOMAINS, MAX_BYTES))

ok("KS4 部分提交必须出声（台账口径：先落账再报错）",
   "已导入前" in src and "已提交" in src and "此前已提交" in src
   and src.count("Some(acc.added)") >= 1,
   "缺：已导入前/已提交/此前已提交/审计条数")

ok("KS5 一批一个事务（BATCH 攒满即 flush，repo 内 begin/commit）",
   "acc.batch.len() >= BATCH" in src
   and "domain_repo::import_domains" in src
   and "begin()" in repo and "commit()" in repo,
   "flush 或用例缺一段")

ok("KS6 复用同一份 normalize 判据（不许文件口另写一套校验）",
   "use super::email_domains::normalize;" in src
   and "pub(super) fn normalize" in old,
   "两份校验会各自漂移")

ctrl = read(NEW, CTRL_REV)
ok("KS7 正控：%s 那版必须被 KS1/KS6 报缺（判据承重）" % CTRL_REV,
   ctrl is None,
   "该版已有文件：正控失效")

# 界面半（同批的 web 侧）：后端不受墙约束只到接口层为止——面板没有文件
# 按钮，站长依旧只有 8176 条的文本框。lib 里的端点字符串与闸门 EP 同源对账；
# 用完整带引号的 URL 比（裸 EP 是前缀形态，漂成 import-file-v2 也照样命中）。
lib = read(WEB_LIB) or ""
panel = read(WEB_PANEL) or ""
url = '"/api/v1%s"' % EP
ok("KS17 面板的文件按钮真接到本口（lib 同端点 + CSRF + 文件输入）",
   url in lib and 'method: "POST"' in lib
   and "rawFetchHelpers.csrf" in lib
   and "importEmailDomainsFile" in panel and 'type="file"' in panel
   and "t.edFileBtn" in panel,
   "lib[url=%s POST=%s csrf=%s] 面板[import=%s input=%s btn=%s]" % (
       url in lib, 'method: "POST"' in lib,
       "rawFetchHelpers.csrf" in lib,
       "importEmailDomainsFile" in panel, 'type="file"' in panel,
       "t.edFileBtn" in panel))
dicts = {n: read("apps/web/i18n/%s.ts" % n) or ""
         for n in ("zh-CN", "en", "zh-TW")}
ok("KS18 三语 edFileBtn 键齐（en/zh-TW 缺键 tsc 也会挡，这里独立读数）",
   all("edFileBtn:" in s for s in dicts.values()),
   " / ".join("%s=%s" % (n, "edFileBtn:" in s)
              for n, s in dicts.items()))

print()
if os.environ.get("KS_SKIP_LIVE"):
    print("（KS_SKIP_LIVE=1：跳过活体层）")
    print("红账：%s" % (FAILURES or "无"))
    sys.exit(1 if FAILURES else 0)
print("=" * 68)
print("二、活体（%s）" % BASE)
print("=" * 68)

BND = "----ksprobe" + TAG.replace("-", "")


def post_file(text, token=None, timeout=120):
    head = ("--%s\r\nContent-Disposition: form-data; name=\"file\"; "
            "filename=\"list.txt\"\r\nContent-Type: text/plain\r\n\r\n"
            % BND).encode()
    body = head + text + ("\r\n--%s--\r\n" % BND).encode()
    req = urllib.request.Request(BASE + EP, method="POST")
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=" + BND)
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        r = urllib.request.urlopen(req, body, timeout=timeout)
        return r.status, json.loads(r.read().decode("utf-8", "replace"))
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read().decode("utf-8", "replace"))


def names(n, prefix="p"):
    return ["%s%s%03d.invalid" % (TAG, prefix, i) for i in range(n)]


def lines(items):
    return ("\n".join(items) + "\n").encode()


probes = names(3)
dup_hi = TAG + "dup.invalid"
dup_lo = TAG + "DUP.invalid"
dup_other = TAG + "other.invalid"
big = names(3, "big")
cp = names(2000, "cp")
tok, small_ok = None, []
dom0 = psql_stdin("SELECT count(*) FROM disposable_email_domains")
try:
    st0, js0 = post_file(b"x.invalid\n")
    ok("KS8 匿名打文件口 401（鉴权早于读书）",
       st0 == 401, "http=%s msg=%s" % (st0, js0.get("message", "")[:100]))

    tok = login()
    s1, j1 = post_file(lines(probes), tok)
    d1 = j1.get("data") or {}
    ok("KS9 3 条真导入：added=3、batches=1",
       s1 == 200 and d1.get("added") == 3 and d1.get("batches") == 1,
       "http=%s data=%s" % (s1, d1))
    small_ok.extend(probes)

    s2, j2 = post_file(lines(probes), tok)
    d2 = j2.get("data") or {}
    ok("KS10 幂等：同文件再导 added=0、skipped=3",
       s2 == 200 and d2.get("added") == 0 and d2.get("skipped") == 3,
       "http=%s data=%s" % (s2, d2))

    s3, j3 = post_file(lines([dup_lo, dup_hi, dup_other]), tok)
    d3 = j3.get("data") or {}
    ok("KS11 文件内大小写重复按归一化去重（total=3 added=2 dup=1）",
       s3 == 200 and d3.get("total") == 3 and d3.get("added") == 2
       and d3.get("duplicates") == 1,
       "http=%s data=%s" % (s3, d3))
    small_ok.extend([dup_lo, dup_other])

    pad = ("x" * 1000 + "\n") * 2_200
    body_size = len(pad) + 200
    s4, j4 = post_file(lines(big) + pad.encode(), tok, timeout=180)
    d4 = j4.get("data") or {}
    ok("KS12 体 %d 字节（> JSON 墙 2MiB）被完整收下并导入 3 条"
       % body_size,
       body_size > 2_097_152 and s4 == 200 and d4.get("added") == 3
       and d4.get("invalid", 0) > 0,
       "http=%s data=%s" % (s4, d4))
    small_ok.extend(big)

    s5, j5 = post_file(("a" * 1200 + "\n").encode(), tok)
    d5 = j5.get("data") or {}
    ok("KS13 超长 token 记无效而不炸（invalid=1、不落库）",
       s5 == 200 and d5.get("invalid") == 1 and d5.get("added") == 0,
       "http=%s data=%s" % (s5, d5))

    # 条数上限要真打得到，且已提交的批必须点名（部分提交不许瞒）
    over = lines(cp) + b"y\n" * (MAX_DOMAINS - len(cp) + 1)
    s6, j6 = post_file(over, tok, timeout=300)
    msg6 = j6.get("message") or ""
    # 只数 cp 那批：按 TAG 总数会把 KS9/11/12 的探针一并算进来（首跑就红在
    # 2008 而不是 2000）
    in_db = psql_stdin(
        "SELECT count(*) FROM disposable_email_domains "
        "WHERE domain LIKE '%scp%%'" % TAG)
    ok("KS14 条数上限 %s 真打得到，且文案点名已导入的 2000 条" %
       MAX_DOMAINS,
       s6 == 400 and "条数上限 %s" % MAX_DOMAINS in msg6
       and "已导入前 2000 条" in msg6 and in_db == "2000",
       "http=%s msg=%s 库内=%s" % (s6, msg6[:140], in_db))
finally:
    try:
        if tok:
            req = urllib.request.Request(
                BASE + "/admin/email-domains/delete", method="POST")
            req.add_header("Content-Type", "application/json")
            req.add_header("Authorization", "Bearer " + tok)
            body = json.dumps({"domains": small_ok}).encode()
            urllib.request.urlopen(req, body, timeout=30).read()
    except Exception:  # noqa: BLE001
        pass
    # 大批探针按本轮 marker 用 SQL 收，别拼 2000 条 JSON
    psql_stdin("DELETE FROM disposable_email_domains "
               "WHERE domain LIKE '%s%%'" % TAG)
    left = psql_stdin(
        "SELECT count(*) FROM disposable_email_domains "
        "WHERE domain LIKE '%s%%'" % TAG)
    ok("KS15 收尾：本轮探针删净（残留=%s）" % left, left == "0",
       "残留=%s" % left)
    dom1 = psql_stdin("SELECT count(*) FROM disposable_email_domains")
    ok("KS16 库内域名数回到跑前基线（前=%s 后=%s）" % (dom0, dom1),
       dom0 == dom1, "前=%s 后=%s" % (dom0, dom1))

print()
print("红账：%s" % (FAILURES or "无"))
sys.exit(1 if FAILURES else 0)
