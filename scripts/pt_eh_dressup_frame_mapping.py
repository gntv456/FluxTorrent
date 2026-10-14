# -*- coding: utf-8 -*-
"""EH 常驻闸门：装扮（头像框）的映射与时效（P-307 修 + 一条待裁反向钉桩）

来历（2026-10-14~15 批九十八/九十九实测）：
  P-307  旧写法在 config 缺 frame_id 时兜底发「全站最贵的一帧」。现网红证：
         SKU 15「头像框」config 无 frame_id 无 avatar_url，佩戴 -> code=0，
         users.avatar_frame_id 被写成 id=3（彩虹描边 price=10000）。已改为
         两级来源 + 回滚 wearing + 1002 中文拒单（宿主侧进程实测 A/B 双绿）。
  待裁    限时装扮过期后：面板 owned/wearing 都变 false（读路径带 expires 谓词），
         而 /me、/users/{id}、/top/boards 用的展示列 users.avatar_frame_id
         仍指向那帧（实测 C 成立）。修法要么给读路径加时效谓词（需要
         item->frame 映射），要么加回收作业（要小心误清管理组直接发的帧）。
         EH5 反向钉桩：现状一改（有人修好）本条就红着提醒删桩。
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, FAILURES  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")

# 现网实测：这是开闸时的真实台账，多一个＝有新 SKU 上架没配帧
KNOWN_UNCONFIGURED = {15}


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode != 0:
        print("   psql 失败：%s / %s" % (sql[:60], p.stderr.strip()[:180]))
        return ""
    return p.stdout.strip()


def src(rel):
    p = os.path.join(SRC, rel)
    if not os.path.exists(p):
        return ""
    out = []
    for line in open(p, encoding="utf-8",
                  errors="replace").read().splitlines():
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "\n".join(out)


def flat(s):
    """把源码压成一行去空白：Rust 的长中文字面量常用 \\ 续行，跨行也要能认出"""
    return s.replace("\\\n", "").replace("\n", "").replace(" ", "")


dr = src("economy_http/dressup.rs")
flat_dr = flat(dr)

# EH1 回归护栏：兜底发最贵一帧的那条 SQL 必须已经不在
ok("EH1 佩戴口不得再用「ORDER BY price DESC ... LIMIT 1」兜底发最贵的一帧",
   "ORDER BY price DESC, id DESC LIMIT 1" not in dr)

# EH2 拒单文案与回滚必须都在（只拒单不回滚 = 面板「佩戴中」而无展示列）
ok("EH2 缺 frame_id 时回中文拒单（文案点出缺哪一项）",
   "该装扮未配置头像框" in flat_dr and "frame_id" in flat_dr
   and "请联系管理组" in flat_dr)
ok("EH2b 拒单前必须把 user_dressups.wearing 回滚（否则面板说不戴、列也没了，"
   "状态与库存不一致）",
   dr.count("SET wearing = FALSE") >= 2)

# EH3 台账：现网有多少「kind=avatar_frame 但 config 解析不出帧」的 SKU
# 不按 active 过滤：兜底对「历史拥有 / 被赠送」的下架件同样会触发
rows = psql(
    "SELECT id FROM shop_items WHERE kind = 'avatar_frame' "
    "AND coalesce(config->>'frame_id', '') = '' "
    "AND coalesce(config->>'avatar_url', '') !~ '^[0-9]+$' ORDER BY id")
act = psql("SELECT coalesce(string_agg(id::text, ','), '无') FROM shop_items "
           "WHERE kind = 'avatar_frame' AND active "
           "AND coalesce(config->>'frame_id', '') = ''")
unconf = set(int(x) for x in rows.split() if x.isdigit())
ok("EH3 未配置的头像框 SKU 必须恰好是台账 %s（多一个 = 新 SKU 没配帧，"
   "少一个 = 台账该删桩）；实测 = %s，其中在架的 = %s"
   % (sorted(KNOWN_UNCONFIGURED), sorted(unconf), act),
   unconf == KNOWN_UNCONFIGURED)

ids = ", ".join(str(i) for i in (sorted(unconf) or [0]))
n_frame = psql(
    "SELECT count(*) FROM avatar_frames WHERE name IN "
    "(SELECT name FROM shop_items WHERE id = ANY(ARRAY[%s]))" % ids)
ok("EH4 这些 SKU 在 avatar_frames 里确实没有同名帧（count=%s）——"
   "所以旧兜底只会把它们发成别人的框" % n_frame, n_frame == "0")

# EH5 反向钉桩：展示列的读路径仍不带时效谓词（实测 C 的现状）
readers = ["auth_http/me.rs", "auth_http/profile.rs",
           "auth_http/me_settings.rs", "content_http/misc.rs"]
no_expiry = 0
for f in readers:
    s = src(f)
    if "f.id = u.avatar_frame_id" in s and "user_dressups" not in s:
        no_expiry += 1
ok("EH5 钉桩在册（实测 C）：%d/%d 个渲染读路径 join 展示列时不看 "
   "user_dressups.expires_at，限时框过期后别人主页仍挂着；修好后本条转红"
   "提醒删桩" % (no_expiry, len(readers)),
   no_expiry == len(readers))

# EH6 面板那几条谓词必须带时效（不然连面板都会说「还在佩戴」）
ok("EH6 装扮列表的 owned/wearing 谓词必须带 expires_at 过滤",
   dr.count("ud.expires_at IS NULL OR ud.expires_at > now()") >= 3)

print("\n== FAILURES (%d): %s" % (len(FAILURES), ", ".join(FAILURES)))
sys.exit(1 if FAILURES else 0)
