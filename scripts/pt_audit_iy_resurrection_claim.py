# -*- coding: utf-8 -*-
"""闸门 IY：单人复活任务只认「领取后新增」的做种时长（P-483）。

病形（批二百四十一实测，现网）：`POST /resurrections/claim` 只校验
「已过审 + 不能领自己的种」——
  * 活种也能领：seeders=2、列表根本看不到的 50482 直接回 200；
  * 验收用 `snatches.seeded_seconds` 的**累计值**：领取者在该种上已有
    240h 做种就直接达标，worker 下一跳发 5000 魔力 + 免费券 + 7 天免费，
    一天补种都不用做。组队契约那边一直用 `seed_seconds_begin` 增量口径，
    单人是唯一例外。

修法：领取口补死种谓词（列表/领取/组队共用 `DEAD_TORRENT_CLAUSE`），
并把领取时的做种秒数快照进 `resurrections.seed_base_seconds`（0415）；
worker 验收改成 `seeded_seconds >= seed_base_seconds + required_hours*3600`。

判据：
  IY1 静态：领取口引用共用死种谓词，INSERT 带 seed_base_seconds；
  IY2 静态：worker 验收必须同时引用两者（只加列不改谓词＝白加）；
  IY3 静态：0415 迁移在册，组队建任务也走同一死种谓词；
  IY4 真库 A/B：活种 dead=false、死种 dead=true；摘掉谓词正控放回活种；
  IY5 基线快照：tx 内给死种造 12345s 旧做种，领取查询必须读到 12345；
  IY6 验收 A/B：tx 内建基线 864000 的行——旧谓词选中 1，新谓词 0；
      再补足 240h 新增后新谓词选中 1（两个方向都钉）；
  IY7 活体：活种 claim 必须拒且不落行（旧镜像红＝缺陷本体）；
  IY8 活体正控：死种 claim 必须 200 且落一行（证明不是一刀切拒绝）；
  IY9 收尾：探针任务/做种零残留，成员停用；
  IY10 静态：我的任务进度必须减基线（否则面板显示已达标却永不发奖）；
  IY11 真库 A/B：旧展示读 900000、新展示读 36000（只钉增量口径）。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_iy_resurrection_claim.py
"""
import inspect
import os
import re
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (call, dig, login, ok, psql_stdin, summary)  # noqa

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUN = int(time.time())
USER = "iyu%d" % RUN
RPW = inspect.signature(login).parameters["pw"].default
NEWPW = RPW + "x"
DEAD = ("t.seeders = 0 AND NOT EXISTS (SELECT 1 FROM snatches s "
        "WHERE s.torrent_id = t.id AND s.last_seen_at > now() "
        "- interval '30 days')")


def q(sql):
    return psql_stdin(sql).strip()


def read(rel):
    return open(os.path.join(ROOTDIR, rel), encoding="utf-8",
                errors="replace").read()


def strip_comments(t):
    t = "\n".join(re.sub(r"(?<!:)//.*", "", ln) for ln in t.split("\n"))
    return re.sub(r"/\*.*?\*/", "", t, flags=re.S)


CLAIM = strip_comments(read(os.path.join(
    "apps", "api", "src", "ops_http", "resurrections.rs")))
SETTLE = strip_comments(read(os.path.join(
    "apps", "worker", "src", "jobs", "settle_periodic.rs")))
TEAM = strip_comments(read(os.path.join(
    "apps", "api", "src", "social_http", "team.rs")))

ok("IY1 领取口引用共用死种谓词（=%s）、INSERT 带 seed_base_seconds（=%s）"
   % ("DEAD_TORRENT_CLAUSE" in CLAIM, "seed_base_seconds" in CLAIM),
   "DEAD_TORRENT_CLAUSE" in CLAIM and "seed_base_seconds" in CLAIM
   and "ON CONFLICT (torrent_id) DO NOTHING" in CLAIM,
   "自己另写一份死种判定＝列表与领取早晚漂开（P-483 就是这么漏的）")
ok("IY2 worker 验收要求「基線 + 新增」(=%s)"
   % ("r.seed_base_seconds" in SETTLE and "required_hours * 3600" in SETTLE),
   "r.seed_base_seconds" in SETTLE and "required_hours * 3600" in SETTLE
   and "s.seeded_seconds >= r.seed_base_seconds" in SETTLE,
   "只加列不改验收谓词＝历史做种照样即时达标")
MIG = read(os.path.join("apps", "api", "migrations",
                        "0415_resurrection_seed_baseline.sql"))
ok("IY3 0415 迁移在册且组队建任务共用死种谓词",
   "ADD COLUMN IF NOT EXISTS seed_base_seconds" in MIG
   and "DEAD_TORRENT_CLAUSE" in TEAM,
   "迁移缺列会让新 SQL 在重建时 500；组队留旧口径会绕过同一闸门")
MINE = CLAIM.split("async fn resurrection_my", 1)[-1]
ok("IY10 我的任务进度减基线（=%s）"
   % ("- r.seed_base_seconds" in MINE),
   "GREATEST(COALESCE(s.seeded_seconds, 0)::bigint "
   "- r.seed_base_seconds, 0)" in MINE,
   "累计值展示会让老会员看到「已达标」而 worker 永不发奖")

ROOT = login()
uid = 0
tok = None
try:
    st, r = call("POST", "/admin/adduser",
                 {"username": USER, "password": RPW,
                  "email": "%s@iy.local" % USER}, token=ROOT)
    uid = int(q("SELECT id FROM users WHERE username = '%s'" % USER) or 0)
    if not uid:
        raise RuntimeError("adduser 失败 http=%s resp=%s"
                           % (st, str(r)[:160]))
    t0 = login(USER)
    call("POST", "/me/password/change",
         {"old_password": RPW, "new_password": NEWPW}, token=t0)
    tok = login(USER, NEWPW)
    ok("IY0 探针成员就绪（uid=%d，不占 root 的 3 枚 Token 上限）" % uid,
       True)

    healthy = q("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
                "AND t.seeders > 0 AND t.owner_id <> %d AND NOT EXISTS "
                "(SELECT 1 FROM resurrections r WHERE r.torrent_id = t.id) "
                "ORDER BY t.id DESC LIMIT 1" % uid)
    dead = q("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
             "AND (%s) AND t.owner_id <> %d AND NOT EXISTS "
             "(SELECT 1 FROM resurrections r WHERE r.torrent_id = t.id) "
             "ORDER BY t.id DESC LIMIT 1" % (DEAD, uid))
    ok("IY4a 现网取样：活种=%s 死种=%s（两个都必须取到）"
       % (healthy, dead), bool(healthy) and bool(dead))

    claim_sql = ("SELECT t.owner_id, (%s) AS dead, COALESCE((SELECT "
                 "s.seeded_seconds FROM snatches s WHERE s.user_id = %d "
                 "AND s.torrent_id = t.id), 0)::bigint FROM torrents t "
                 "WHERE t.id = %%s AND t.approval_status = 1" % (DEAD, uid))
    h_dead = q(claim_sql % healthy).split("|")
    d_dead = q(claim_sql % dead).split("|")
    no_clause = ("SELECT t.owner_id, TRUE AS dead, COALESCE((SELECT "
                 "s.seeded_seconds FROM snatches s WHERE s.user_id = %d "
                 "AND s.torrent_id = t.id), 0)::bigint FROM torrents t "
                 "WHERE t.id = %%s AND t.approval_status = 1" % uid)
    ctrl = q(no_clause % healthy).split("|")
    ok("IY4 死种谓词：活种 dead=%s 死种 dead=%s；摘掉谓词正控活种 dead=%s"
       % (h_dead[1] if len(h_dead) > 1 else "?",
          d_dead[1] if len(d_dead) > 1 else "?",
          ctrl[1] if len(ctrl) > 1 else "?"),
       h_dead[1:2] == ["f"] and d_dead[1:2] == ["t"]
       and ctrl[1:2] == ["t"], "活种=%s 死种=%s" % (h_dead, d_dead))

    base = q("BEGIN;"
             "INSERT INTO snatches (user_id, torrent_id, seeded_seconds, "
             "seeding, last_port, downloaded, uploaded, connectable, "
             "last_seen_at) VALUES (%d, %s, 12345, TRUE, 6881, 1024, 2048, "
             "-1, now()) ON CONFLICT (user_id, torrent_id) DO UPDATE SET "
             "seeded_seconds = 12345;"
             "SELECT COALESCE((SELECT s.seeded_seconds FROM snatches s "
             "WHERE s.user_id = %d AND s.torrent_id = t.id), 0)::bigint "
             "FROM torrents t WHERE t.id = %s;"
             "ROLLBACK;" % (uid, dead, uid, dead))
    got_base = base.split("\n")[-2].strip() if base else ""
    ok("IY5 领取查询快照旧做种秒数（造 12345 ⇒ 读回 %s）" % got_base,
       got_base == "12345", base[:160])

    old_pred = ("s.seeded_seconds >= r.required_hours * 3600")
    new_pred = ("s.seeded_seconds >= r.seed_base_seconds + "
                "r.required_hours * 3600")
    settle_probe = (
        "BEGIN;"
        "ALTER TABLE resurrections ADD COLUMN IF NOT EXISTS "
        "seed_base_seconds BIGINT NOT NULL DEFAULT 0;"
        "INSERT INTO resurrections (torrent_id, user_id, required_hours, "
        "reward_sparks, seed_base_seconds) VALUES (%s, %d, 240, 5000, "
        "864000);"
        "INSERT INTO snatches (user_id, torrent_id, seeded_seconds, seeding, "
        "last_port, downloaded, uploaded, connectable, last_seen_at) VALUES "
        "(%d, %s, 864000, TRUE, 6881, 1024, 2048, -1, now()) "
        "ON CONFLICT (user_id, torrent_id) DO UPDATE SET seeded_seconds = "
        "864000, seeding = TRUE, last_port = 6881, downloaded = 1024, "
        "last_seen_at = now();"
        "SELECT 'OLD=' || count(*) FROM resurrections r WHERE r.torrent_id = "
        "%s AND r.user_id = %d AND r.status = 'open' AND r.team_id IS NULL "
        "AND EXISTS (SELECT 1 FROM snatches s WHERE s.user_id = r.user_id "
        "AND s.torrent_id = r.torrent_id AND %s AND s.seeding);"
        "SELECT 'NEW=' || count(*) FROM resurrections r WHERE r.torrent_id = "
        "%s AND r.user_id = %d AND r.status = 'open' AND r.team_id IS NULL "
        "AND EXISTS (SELECT 1 FROM snatches s WHERE s.user_id = r.user_id "
        "AND s.torrent_id = r.torrent_id AND %s AND s.seeding);"
        "UPDATE snatches SET seeded_seconds = 864000 + 240 * 3600 "
        "WHERE user_id = %d AND torrent_id = %s;"
        "SELECT 'FULL=' || count(*) FROM resurrections r WHERE r.torrent_id = "
        "%s AND r.user_id = %d AND r.status = 'open' AND r.team_id IS NULL "
        "AND EXISTS (SELECT 1 FROM snatches s WHERE s.user_id = r.user_id "
        "AND s.torrent_id = r.torrent_id AND %s AND s.seeding);"
        "ROLLBACK;")
    out = q(settle_probe % (dead, uid, uid, dead, dead, uid, old_pred,
                            dead, uid, new_pred, uid, dead, dead, uid,
                            new_pred))
    vals = dict(re.findall(r"(OLD|NEW|FULL)=(\d+)", out))
    ok("IY6 验收 A/B：历史 240h 旧谓词选中=%s（应 1）·新谓词选中=%s（应 0）"
       "·补足新增 240h 后新谓词选中=%s（应 1）"
       % (vals.get("OLD"), vals.get("NEW"), vals.get("FULL")),
       vals.get("OLD") == "1" and vals.get("NEW") == "0"
       and vals.get("FULL") == "1", out[-200:])

    mine_probe = (
        "BEGIN;"
        "ALTER TABLE resurrections ADD COLUMN IF NOT EXISTS "
        "seed_base_seconds BIGINT NOT NULL DEFAULT 0;"
        "INSERT INTO resurrections (torrent_id, user_id, required_hours, "
        "reward_sparks, seed_base_seconds) VALUES (%s, %d, 240, 5000, "
        "864000);"
        "INSERT INTO snatches (user_id, torrent_id, seeded_seconds, seeding, "
        "last_port, downloaded, uploaded, connectable, last_seen_at) VALUES "
        "(%d, %s, 900000, TRUE, 6881, 1024, 2048, -1, now()) "
        "ON CONFLICT (user_id, torrent_id) DO UPDATE SET seeded_seconds = "
        "900000;"
        "SELECT 'OLDMINE=' || COALESCE(s.seeded_seconds, 0)::bigint "
        "FROM resurrections r LEFT JOIN snatches s ON s.user_id = r.user_id "
        "AND s.torrent_id = r.torrent_id WHERE r.torrent_id = %s "
        "AND r.user_id = %d;"
        "SELECT 'NEWMINE=' || GREATEST(COALESCE(s.seeded_seconds, 0)::bigint "
        "- r.seed_base_seconds, 0) FROM resurrections r LEFT JOIN snatches s "
        "ON s.user_id = r.user_id AND s.torrent_id = r.torrent_id "
        "WHERE r.torrent_id = %s AND r.user_id = %d;"
        "ROLLBACK;")
    mout = q(mine_probe % (dead, uid, uid, dead, dead, uid, dead, uid))
    mvals = dict(re.findall(r"(OLDMINE|NEWMINE)=(\d+)", mout))
    ok("IY11 我的任务进度 A/B：旧展示=%s（应 900000）·新展示增量=%s（应 36000）"
       % (mvals.get("OLDMINE"), mvals.get("NEWMINE")),
       mvals.get("OLDMINE") == "900000" and mvals.get("NEWMINE") == "36000",
       mout[-200:])

    sh, bh = call("POST", "/resurrections/claim",
                  {"torrent_id": int(healthy)}, token=tok)
    rowh = q("SELECT count(*) FROM resurrections WHERE torrent_id = %s"
             % healthy)
    ok("IY7 活种不可认领（http=%s code=%s，落行=%s 须 0）"
       % (sh, dig(bh, "code"), rowh),
       sh == 400 and dig(bh, "code") == 1002 and rowh == "0",
       "旧镜像这条红＝缺陷本体（P-483：活种 claim 回 200 并占唯一槽位）")

    sd, bd = call("POST", "/resurrections/claim",
                  {"torrent_id": int(dead)}, token=tok)
    rowd = q("SELECT count(*) FROM resurrections WHERE torrent_id = %s "
             "AND user_id = %d" % (dead, uid))
    ok("IY8 死种正控可认领（http=%s code=%s，落行=%s 须 1）"
       % (sd, dig(bd, "code"), rowd),
       sd == 200 and dig(bd, "code") == 0 and rowd == "1",
       "若与 IY7 同时绿而此条红＝把领取口一刀切关死了")
finally:
    if uid:
        q("DELETE FROM resurrections WHERE user_id = %d" % uid)
        nres = q("SELECT count(*) FROM resurrections WHERE user_id = %d"
                 % uid)
        nsn = q("SELECT count(*) FROM snatches WHERE user_id = %d" % uid)
        q("UPDATE users SET status = 2 WHERE id = %d" % uid)
        ok("IY9 探针零残留（复活任务=%s 做种=%s 须 0/0）、成员已停用"
           % (nres, nsn), nres == "0" and nsn == "0",
           "任务行必须在 worker 下一跳前删掉（探针无做种，本就不会被选中）")

summary()
