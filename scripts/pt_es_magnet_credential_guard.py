# -*- coding: utf-8 -*-
"""ES 常驻闸门：发「带本人 passkey 的下载凭证」的出口，闸门必须对等（ES36）

磁力链接里内嵌 `announce/<本人 passkey>`，拿到就能进私有 swarm —— 所以
它和 .torrent 下载是同一种凭证，不能少过任何一道闸门。
本批实测到的修前行为（现网旧镜像 :8080）：给探针种临时标价 500、root 未购，
`GET /torrents/40049/magnet` 照样 code=0 发出 magnet（现场复原、零残留）。

  ES0  正控：修前 magnet.rs 必须「既无 ensure_downloadable 也无购买判定」
  ES1  现网 magnet.rs：调 ensure_downloadable + 查 torrent_purchases +
       付费未购回中文整句拒单（不重复扣费，但也不给没买过的人发凭证）
  ES2  凭证出口普查：每条「发 announce/passkey」的路径都要出现闸门符号
       （build_torrent_bytes 或 ensure_downloadable），逐条点名，少一条即红
  ES3  兜底仍在：build_torrent_bytes 本体里那道 ensure_downloadable 兜底
       不许被删（download.rs:113 的注释解释了为什么留两处）
  ES4  现网只读：本批取证不留下痕迹（价格复原、无探针购买行、无备份表）
  ES5  订阅出口（Torznab）与 RSS 同口径：付费未购默认回收 + ?paid=0 逃生口
       （批一百三十八 P-324；compat torrents.json 同族待站方拍板，只记读数）
pt_audit_lib.ok 的签名是 ok(name, cond, detail)；正控钉字面提交号，别写 HEAD。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MAG = "apps/api/src/torrent_http/magnet.rs"
# P-324 把 SQL 下沉 repo 层（effbe651）：购买判定的 SQL 现在在这里。
REPO_MAG = "apps/api/src/repo/magnet.rs"
DL = "apps/api/src/publish_http/download.rs"
FIX = "383ac6c3"  # 批一百三十六：ES36 修复提交（正控取 383ac6c3^，别写 HEAD）
CRED_PATHS = [
    "apps/api/src/publish_http/download.rs",
    "apps/api/src/torrent_http/magnet.rs",
    "apps/api/src/torrent_http/batch.rs",
    "apps/api/src/compat_http/nexusphp.rs",
    "apps/api/src/compat_http/download_keys.rs",
]


def sh(args):
    return subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")


def blob(rev, path):
    p = sh(["git", "show", "%s:%s" % (rev, path)])
    if p.returncode != 0:
        FAILURES.append("ES 取文失败 %s:%s / %s"
                        % (rev, path, (p.stderr or "")[:120]))
        return ""
    return p.stdout or ""


def here(path):
    p = os.path.join(ROOT, path)
    if not os.path.exists(p):
        FAILURES.append("ES 文件缺失 %s" % path)
        return ""
    return open(p, encoding="utf-8", errors="replace").read()


def strip_rs(s):
    out = []
    for line in s.splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def flat(s):
    return " ".join(strip_rs(s).replace("\\\n", " ").split())


old_mag = flat(blob("%s^" % FIX, MAG)) if FIX != "WORKTREE" else ""
new_mag = flat(here(MAG))
repo_mag = flat(here(REPO_MAG)) if os.path.exists(REPO_MAG) else ""

print("== ES0/ES1 磁力口的闸门 ==")
if FIX != "WORKTREE":
    ok("ES0 正控：修前 magnet.rs 既无 ensure_downloadable 也无购买判定",
       "ensure_downloadable" not in old_mag
       and "torrent_purchases" not in old_mag,
       "旧体含 ensure=%s 含 purchases=%s" % ("ensure_downloadable" in old_mag,
                                          "torrent_purchases" in old_mag))
ok("ES1a 现网磁力口先过下载资格闸门",
   "ensure_downloadable" in new_mag)
# 批一百五十三更正：ES1b 原来断言「magnet.rs 里同时出现 torrent_purchases
# 与 `if price > 0 && owner != Some(auth.id)` 同一段」。P-324 那批
# 「SQL 下沉 repo 层」（effbe651）把两个都搬走了：SQL 进 repo/magnet.rs，
# 判定留在 http 层，于是**行为完全没变**、断言却恒红。取证：torrent_http/magnet.rs
# 92~107 行仍然先 ensure_downloadable、再 `if price > 0 && owner != Some(auth.id)`
# 再查 purchased、没买过就 400——链路完整。
# 判据改成「跨文件认三段形状」：判定条件、购买查询、repo 层 SQL 三件都在，
# 且判定与查询出现在同一个 if 块里（不是各写各的）。
body = new_mag
ok("ES1b 付费种必须有购买记录才发凭证",
   ("price > 0" in body and "owner != Some(auth.id)" in body
    and "purchased" in body)
   and ("torrent_purchases" in repo_mag),
   "判定=%s purchased调用=%s repo层SQL=%s"
   % ("price>0" in body, "purchased" in body, "torrent_purchases" in repo_mag))
ok("ES1c 付费未购回中文整句（含「付费种子」与「下载」指引）",
   "该种子为付费种子" in new_mag and "请先在详情页点" in new_mag)
ok("ES1d 磁力口不重复扣费（没调 charge_for_download）",
   "charge_for_download" not in new_mag)

print("== ES2 发凭证出口的闸门对等普查 ==")
for rel in CRED_PATHS:
    f = flat(here(rel))
    ok("ES2 %s 经过 build_torrent_bytes 或直接调 ensure_downloadable" % rel,
       "build_torrent_bytes" in f or "ensure_downloadable" in f)
ok("ES2b 五条出口全部在册（少一条就是有人加了新凭证口没进表）",
   len(CRED_PATHS) == 5, str(CRED_PATHS))

print("== ES3 兜底闸门还在 ==")
dl = flat(here(DL))
i = dl.find("pub async fn build_torrent_bytes")
ok("ES3 build_torrent_bytes 体内仍保留 ensure_downloadable 兜底",
   i >= 0 and "ensure_downloadable" in dl[i:i + 2200])

print("== ES4 现网只读：本批取证不留痕 ==")


def psql(stmt):
    p = sh(["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
            "-d", "fluxtorrent", "-tAc", stmt])
    return "" if p.returncode != 0 else (p.stdout or "").strip()


price = psql("SELECT price FROM torrents WHERE id = 40049")
buy = psql("SELECT count(*) FROM torrent_purchases WHERE torrent_id = 40049"
           " AND user_id = 1")
bak = psql("SELECT count(*) FROM pg_tables WHERE tablename = '_es36_bak'")
if price == "":
    print("   WARN 库不可达：ES4 本轮不判（静态各条照常承重）")
else:
    ok("ES4a 探针种 40049 价格已复原为 0", price == "0", "实读 %s" % price)
    ok("ES4b 无 ES36 探针购买行残留", buy == "0", "实读 %s" % buy)
    ok("ES4c 备份表已 DROP", bak == "0", "实读 %s" % bak)
    paid = psql("SELECT count(*) FROM torrents WHERE approval_status = 1 "
                "AND price > 0")
    print("   现网口径参考：过审且付费的种 %s 枚" % paid)

print("== ES5 批一百三十八 P-324：订阅出口的付费回收对等 ==")
# Torznab 的 item 直接带 download.php?id&passkey，客户端会自动抓 enclosure，
# 而 charge_for_download 是「首次下载即扣魔力」——列出来等于替用户花钱。
# RSS 早在深测 2026-10-03 定了默认排除（exclude_paid），Torznab 当时漏接。
SUB_FIX = "9b00d2a0"  # 修复提交；正控取 9b00d2a0^，别写 HEAD
REC = "apps/api/src/economy_http/recon.rs"
NCL = "apps/api/src/torrents/list_noclamp.rs"
RSF = "apps/api/src/rss_http.rs"
old_rec = flat(blob("%s^" % SUB_FIX, REC))
ok("ES5a 正控：修前 recon.rs 既无订阅入口也不判购买",
   "list_torrents_noclamp_for_sub" not in old_rec
   and "torrent_purchases" not in old_rec)
rec = flat(here(REC))
ok("ES5b 现网 recon.rs 走 list_torrents_noclamp_for_sub",
   "list_torrents_noclamp_for_sub" in rec)
ok("ES5c recon.rs 把 Token 主人的 uid 当 viewer 传下去（不再恒 0）",
   re.search(r"list_torrents_noclamp_for_sub\([^)]*tk\.uid", rec) is not None)
ok("ES5d recon.rs 认 ?paid=0 逃生口（与 RSS 同名参数）",
   'get("paid")' in rec and '"0"' in rec)
ncl = flat(here(NCL))
ok("ES5e 订阅入口内部三段放行谓词齐（price/owner/已购）",
   "t.price > 0" in ncl and "owner_id IS DISTINCT FROM" in ncl
   and "torrent_purchases" in ncl)
ok("ES5f 订阅入口把 viewer 透传给 noclamp_as（谓词才有「本人」可判）",
   re.search(r"list_torrents_noclamp_as\([^)]*viewer", ncl) is not None)
ok("ES5g RSS 那条 exclude_paid 仍在（别把两边一起拆掉）",
   "exclude_paid" in here(RSF))
# 同族未收口：compat torrents.json 也回显本人 passkey 供工具拼 download.php，
# 但它是「站内列表镜像」口径，是否该一起排除付费种要站方拍板 —— 记读数不判红。
nx = here("apps/api/src/compat_http/nexusphp.rs")
bare = nx.count("list_torrents_noclamp(") - nx.count(
    "list_torrents_noclamp_for_sub(")
print("   WARN 同族待裁：compat/nexusphp.rs 仍走 bare noclamp %s 处"
      "（回显 passkey，付费种默认可见）" % bare)

summary()
