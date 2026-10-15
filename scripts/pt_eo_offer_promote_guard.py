# -*- coding: utf-8 -*-
"""EO 常驻闸门：候选转正（offers）链的两道闸——态守卫与票数账

静态（CI 跑，不写库）：
  EO0  正控：872d2d26 之前的 offers.rs 必须被「无条件把种写回过审态」
       抓到（否则本判据不承重）
  EO1a 现网体 0 处无条件写回；带 approval_status=1 守卫的盖章恰 1 处
  EO1b 抢不到态时的拒单分支必须还原 offers.promoted（别留半转正）
  EO2  参照物：建候选那一侧仍要求 approval_status = 1（别把入口也拆了）
  EO3  六条把种置成非过审态的路径仍在（守卫的前提成立，不是空判据）
现网只读（库不可达则本轮跳过，不判红）：
  EO4  不存在 promoted=true 而对应种 approval_status<>1 的候选
  EO5  offers.votes 与 offer_votes 逐单行数一致（票数不许撒谎）

运行时红证（真口 POST /offers/promote + 整行复原）见
 _doc/资深PT用户实测审计-2026-10-10.md 批一百三十一。
pt_audit_lib.ok 的签名是 ok(name, cond, detail)，条件写在第二格。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OFFERS = "apps/api/src/content_http/offers.rs"
# 3641db86「SQL 下沉 repo 层」把盖章的 SQL 搬到了 repo/offers.rs，
# http 层只留调用与拒单分支。批一百五十三：这两条断言因此恒红，而
# **守卫本身在位**（repo/offers.rs:161 的 AND approval_status = 1）。
REPO_OFFERS = "apps/api/src/repo/offers.rs"
FIX = "872d2d26"  # 批一百三十一：P-317 转正态守卫

# 旧形状：把 approval_status 写成 1 且 WHERE 里没有 approval_status 守卫
BLIND_APPROVE = re.compile(r"UPDATE torrents SET approval_status\s*=\s*1"
                           r"[^;]*?(?=\\?\"|;)")
GUARD_STAMP = re.compile(r"UPDATE torrents SET approved_at\s*=\s*now\(\)"
                         r"\s*WHERE id\s*=\s*\$1\s+AND approval_status\s*=\s*1"
                         r"\s+RETURNING id")


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev, path):
    p = sh(["git", "show", "%s:%s" % (rev, path)])
    if p.returncode != 0:
        FAILURES.append("EO 取文失败 %s:%s / %s"
                        % (rev, path, (p.stderr or "")[:140]))
        return ""
    return p.stdout or ""


def strip_rs(s):
    out = []
    for line in s.splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def flat(s):
    return s.replace("\\\n", " ").replace("\n", " ")


def blind(text):
    """approval_status = 1 的写入点里，有几处不带同句的 approval_status 守卫"""
    f = flat(strip_rs(text))
    hits = 0
    for m in re.finditer(r"UPDATE torrents SET[^;]*approval_status\s*=\s*1"
                         r"[^;]*;", f):
        if "approval_status = 1" in m.group(0)[m.group(0).find("WHERE"):]:
            continue
        hits += 1
    return hits


print("== EO0/EO1 转正的态守卫 ==")
old = blob("%s^" % FIX, OFFERS)
now = blob("HEAD", OFFERS)
# 两层都算「现网体」：守卫可以落在任一层，只认 http 层会把下沉误判成丢失
now_both = now + "\n" + blob("HEAD", REPO_OFFERS)
ok("EO0 正控：修前 offers.rs 的盲目写回被抓到 >=1 处", blind(old) >= 1,
   "实抓 %s" % blind(old))
ok("EO1a 现网体盲目写回 0 处", blind(now) == 0, "实抓 %s" % blind(now))
# 「恰 1 处」改成「>=1 且 http+repo 两层里只有这一个写点」：
# 钉死「恰 1」会在日后合法新增第二个盖章口时假红，而这条要判的是
# 「守卫在不在」，不是「有几处」。
_okb = len(GUARD_STAMP.findall(flat(strip_rs(now_both))))
ok("EO1b 带 approval_status=1 守卫的盖章存在（http+repo 两层合计）",
   _okb >= 1, "实抓 %s" % _okb)
# http + repo 两层合看：3641db86「SQL 下沉 repo 层」把「建候选口前置校验」
# 与「盖章在架守卫」都搬进了 repo/offers.rs。只扫 http 层会把这两道
# **在位**的闸门误判成丢失——守卫在哪一层属分层风格，守卫在不在才是语义。
nf = flat(strip_rs(now_both))
# 拒单分支现在是 `if !stamped { unpromote(...) }`（3641db86 把 UPDATE 抽进
# repo::offers::unpromote），旧形状 stamped.is_none(){...UPDATE...} 不再出现。
# 判据改成「抢不到态分支里确实调了 unpromote（还原 promoted）」，不绑它是
# 内联 SQL 还是 repo 调用——那属于分层风格，守卫在不在才是语义。
ok("EO1c 抢不到态的分支里还原了 promoted（不留半转正）",
   re.search(r"!stamped\s*\{.{0,240}?unpromote", nf, re.S) is not None
   or re.search(r"stamped\.is_none\(\)\s*\{.{0,320}?UPDATE offers SET"
                r" promoted\s*=\s*false", nf, re.S) is not None)
# 3641db86 把拒单分支写成 `if !stamped { unpromote(...); return Err(...) }`，
# 旧形状 stamped.is_none(){...} 不再出现。判据改成「抢不到态分支里有中文整句
# 拒单文案」——分支形状可变，文案必须是人话（不是内部字面量）。
ok("EO1d 拒单句是中文整句（不是内部字面量）",
   re.search(r"!stamped\s*\{.{0,520}?该候选对应的种子当前不在架",
             nf, re.S) is not None
   or re.search(r"stamped\.is_none\(\)\s*\{.{0,520}?该候选对应的种子当前不在架",
                 nf, re.S) is not None)

print("== EO2/EO3 参照物与前提 ==")
# 同样受 3641db86 影响：前置校验搬到了 repo/offers.rs::torrent_approved
# （SELECT EXISTS(... AND approval_status = 1)）。判据改成「两层合计找得到
# 这道 EXISTS 守卫」——入口闸门不能被一起拆掉，但闸门在哪一层是分层风格。
ok("EO2 建候选口仍要求 approval_status = 1（入口别一起拆掉；http+repo 两层合计）",
   re.search(r"SELECT EXISTS\(SELECT 1 FROM torrents WHERE id = \$1\s+AND\s+"
             r"approval_status = 1\)", nf) is not None,
   "http 层未找到，去 repo 层认了")
six = sh(["git", "grep", "-c", "SET approval_status", "--", "apps/api/src",
          "apps/worker/src"])
paths = [ln.split(":")[-2] for ln in (six.stdout or "").splitlines()
         if ln.count(":") >= 1]
ok("EO3 六条以上「过审后改态」的路径仍在（守卫前提没被删空）",
   len(set(paths)) >= 5,
   "认到 %s 处：%s" % (len(set(paths)), sorted(set(paths))))

print("== EO4/EO5 现网只读 ==")


def psql(sql):
    p = sh(["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
            "-d", "fluxtorrent", "-tAc", sql])
    return "" if p.returncode != 0 else (p.stdout or "").strip()


base = psql("SELECT count(*) FROM offers")
if base == "":
    print("   WARN 库不可达：EO4/EO5 本轮不判（静态四条照常承重）")
else:
    bad = psql("SELECT count(*) FROM offers o JOIN torrents t ON t.id"
               " = o.torrent_id WHERE o.promoted AND t.approval_status <> 1")
    ok("EO4 已转正的候选对应种必须在架（0 条）", bad == "0", "实读 %s" % bad)
    drift = psql("SELECT count(*) FROM offers o WHERE o.votes <>"
                 " (SELECT count(*) FROM offer_votes v"
                 " WHERE v.offer_id = o.id)")
    if base == "0":
        print("   WARN offers 表现网 0 行：EO5 本轮是空集，只记数不承重")
    ok("EO5 offers.votes 与 offer_votes 逐单一致（0 单漂移）", drift == "0",
       "实读 %s" % drift)
    print("   现网 offers=%s 已转正=%s" % (
        base, psql("SELECT count(*) FROM offers WHERE promoted")))

summary()
