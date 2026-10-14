# -*- coding: utf-8 -*-
"""EG 常驻闸门：.torrent 下载口的文件名头与私种标记（P-299 / P-300）

来历（2026-10-14 批八十八实测）：
  P-299  网页下载口 GET /torrents/{id}/download 只带 Content-Type，浏览器按
         URL 末段存盘成 `download`，而批量口早就是 `{种名}-{id}.torrent`。
         修后（宿主侧进程 :8097 打现网库实测）头为
         `attachment; filename="e2e-regression-seed-40.torrent"; filename*=…`
  P-300  读码时怀疑「`private=1` 只写在根级字典，BEP3 要求它在 `info` 内
         ⇒ 客户端忽略 ⇒ DHT/PEX 未关」。实测**部分推翻**：79 个存档里 72 个
         的 `info.private` 本来就是 1，缺标记的只有闸门/演示自造的 7 个。
         根级那枚确实是死键（客户端不认），但它只是冗余，不是全站漏遮。
         残留问题收敛为「上传侧要不要拦非私有种」——见 EG5 台账判据；
         本轮未做发种写入探针，那一半取证留在下一批（不拿未验的判据冒充绿）。
"""
import os
import re
import subprocess
import time
import sys
import urllib.request
import urllib.error

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, login, ok, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SF = os.path.join(ROOT, "apps", "api", "src")


def src(rel):
    p = os.path.join(SF, rel)
    if not os.path.exists(p):
        return ""
    s = open(p, encoding="utf-8", errors="replace").read()
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    out = []
    for line in s.splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def fn_body(s, name):
    """取某个 fn 的函数体（按花括号配平，够用于本仓写法）。"""
    i = s.find("fn " + name)
    if i < 0:
        return ""
    d = 0
    started = False
    for j in range(i, len(s)):
        if s[j] == "{":
            d += 1
            started = True
        elif s[j] == "}":
            d -= 1
            if started and d == 0:
                return s[i:j + 1]
    return s[i:]


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode != 0:
        print("   psql 失败：%s / %s" % (sql[:50], p.stderr.strip()[:160]))
        return ""
    return p.stdout.strip()


def bdecode(data, pos=0):
    c = data[pos:pos + 1]
    if c == b"i":
        e = data.index(b"e", pos)
        return int(data[pos + 1:e]), e + 1
    if c in (b"l", b"d"):
        pos += 1
        out = [] if c == b"l" else {}
        while data[pos:pos + 1] != b"e":
            k, pos = bdecode(data, pos)
            if c == b"d":
                v, pos = bdecode(data, pos)
                out[k.decode("utf-8", "replace")] = v
            else:
                out.append(k)
        return out, pos + 1
    colon = data.index(b":", pos)
    n = int(data[pos:colon])
    s = colon + 1
    return data[s:s + n], s + n


dl = src("publish_http/download.rs")
at = src("publish_http/attachment.rs")
bt = src("bencode/torrent.rs")
modl = src("publish_http/mod.rs")

# EG0 前置：必须真取到一个可下载的过审种，否则 EG4 是空转
tid = psql("SELECT id FROM torrents WHERE approval_status = 1 "
           "ORDER BY id LIMIT 1")
ok("EG0 前置：现网有过审种可取样（id=%s）——没有样本 EG4 无从判定" % tid,
   bool(tid))

# EG1 接线：网页口必须真的把文件名头插进响应（只写函数不接＝假修）
ok("EG1 网页下载口接上了 Content-Disposition（1 处插入 + 调用构造器）",
   dl.count("CONTENT_DISPOSITION") == 1
   and "content_disposition(" in dl)
ok("EG1b 承载文件已在 mod 注册（没注册就编译不进来）",
   "mod attachment" in modl)

# EG2 双口径：老客户端只认 filename=，中文必须靠 filename*
ok("EG2 头值同时给 ASCII 回退段与 filename*=UTF-8（RFC 6266/5987）",
   "attachment" in at and "filename*=UTF-8" in at
   and "filename=" in at)

# EG3 防注入：种名由发帖人给，回退段白名单里不得有引号/分号/反斜杠，
#     且必须真的把非法字节换成下划线
fb = fn_body(at, "ascii_fallback")
ok("EG3 回退名白名单不含引号/分号/反斜杠，且非白名单字节换成下划线"
   "（函数体 %s 行）" % fb.count("\n"),
   len(fb) > 40 and 'b\'"\'' not in fb and "b';'" not in fb
   and 'b"\\\\\\\\"' not in fb and "'_'" in fb)

# EG4 运行时三态（同 AL 的 KNOWN_P298 形状）：
#   头在 → 必须是 attachment 且以 -{种号}.torrent 结尾；
#   头缺 → 现网镜像还没带上本批改动的钉桩，PASS 但把读数写出来。
KNOWN_PENDING_DEPLOY = False   # 10-15 现网 latest 已回头名，撤桩
hdr = ""
sts = []
tok = login()
for _try in range(4):          # 下载口 20 次/分限流会把这一枪挡成 429，
    try:                        # 那不是「头缺失」——带退避重试并记下每次状态
        req = urllib.request.Request(
            BASE.replace("/api/v1", "")
            + "/api/v1/torrents/%s/download" % tid)
        req.add_header("Authorization", "Bearer " + tok)
        r = urllib.request.urlopen(req, timeout=30)
        sts.append(r.status)
        hdr = {k.lower(): v for k, v in r.headers.items()}.get("content-disposition", "")
        r.read()
        if hdr:
            break
    except urllib.error.HTTPError as e:
        sts.append(e.code)
        hdr = {k.lower(): v for k, v in e.headers.items()}.get("content-disposition", "")
        if hdr:
            break
    time.sleep(20)
print("   下载口逐次 http=%s（429=限流窗口，不等于头缺失）" % sts)
if hdr:
    ok("EG4 现网响应头合规：attachment 且以 -%s.torrent 结尾（读值=%s）"
       % (tid, hdr[:70]),
       hdr.startswith("attachment") and ("-%s.torrent" % tid) in hdr)
    if KNOWN_PENDING_DEPLOY:
        print("   WARN 钉桩可撤：现网已回头名，KNOWN_PENDING_DEPLOY 改 "
              "False 后「缺头」分支就不再被认可")
else:
    ok("EG4 现网响应头取到了吗（取不到时看上面逐次 http：429 属限流窗，"
       "要复跑而不是当缺陷）", bool(hdr), "读数=空 逐次=%s" % sts)

# EG5 P-300（修正版）：读码时我以为「private 只写根级 => 全站 DHT 未关」，
# 实测推翻——存档里 72 个种子的 info.private 全是 1，缺标记的只有闸门/演示
# 自造的 7 个。所以真正的残留是「**上传侧不校验私有标记**」：会员发一个 info
# 里没有 private 的种照样收得进来，而下发时补的根级 private 客户端根本不认，
# 等于这份内容在 DHT 上可见。判据按台账集合钉：多一个立刻红（拦不住新上传），
# 台账里的种被补好或删掉也红着提醒删桩（同 AL 的 KNOWN_P298 形状）。
KNOWN_UNPRIV = {40, 40049, 40079, 42826, 42838, 42839, 42840}
rows = psql("SELECT torrent_id||'|'||encode(raw,'hex') FROM torrent_files "
            "WHERE length(raw) < 200000")
n_all = n_one = n_zero = 0
miss = set()
for line in rows.splitlines():
    if "|" not in line:
        continue
    tid_, hex_ = line.split("|", 1)
    try:
        root, _ = bdecode(bytes.fromhex(hex_))
    except Exception:
        continue
    if not isinstance(root, dict):
        continue
    n_all += 1
    info = root.get("info")
    pv = info.get("private") if isinstance(info, dict) else None
    if pv == 1:
        n_one += 1
    elif pv == 0:
        n_zero += 1
    else:
        miss.add(int(tid_))
ok("EG5 P-300 台账：解析 %s 个存档，info.private=1 有 %s 个、=0 有 %s 个，"
   "无标记 %s 个——无标记集合必须与在册名单一致（多一个即「上传侧没拦非"
   "私有种」，少一个说明台账该删桩）" % (n_all, n_one, n_zero, len(miss)),
   n_all > 0 and n_zero == 0 and miss == KNOWN_UNPRIV)
if miss != KNOWN_UNPRIV:
    print("   新增无标记种=%s 已消失=%s"
          % (sorted(miss - KNOWN_UNPRIV), sorted(KNOWN_UNPRIV - miss)))

# EG6 回归护栏：绕过 tracker 的取源通道仍必须被剥干净
ok("EG6 下载重建仍剥离 url-list / httpseeds / dht_nodes"
   "（三条绕过 announce 计量的通道）",
   all(k in bt for k in ("url-list", "httpseeds", "dht_nodes")))

# EG7 口径一致：单击口与批量口同一命名法（archive::safe_name + 种号）
bd = psql("SELECT count(*) FROM torrents WHERE approval_status = 1")
ok("EG7 与批量口同一命名口径（safe_name 清洗 + 种号后缀），现网过审种 %s"
   % bd,
   "safe_name(" in dl and ".torrent" in dl and bool(bd))

print("\n== FAILURES (%d): %s" % (len(FAILURES), ", ".join(FAILURES)))
sys.exit(1 if FAILURES else 0)
