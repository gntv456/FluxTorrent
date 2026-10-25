# -*- coding: utf-8 -*-
"""EJ 闸门：会员面组队口对拼错键名的判别（P-311，14 只高危里唯一会员面）

来历（2026-10-15 批一百零六）：
  `POST /social/team/create` 的 `CreateTeamReq{torrent_id, team_size_max}`
  处理器是 `body.team_size_max.unwrap_or(5).clamp(2, 10)` ⇒ 键名拼成
  `size_max`/`team_size` 时 serde 静默吞掉，队伍按 5 人建好并回成功，而
  `endangered-panel.tsx:69` 只发 `{torrent_id}`、界面也不回显这个人数。
  补 `deny_unknown_fields` 后走 path_cfg 的未知字段出口。

诚实边界（现网 `module_social = no`）：模块开关在中间件层就把本口拦成
404/4101，**位置在反序列化之前** ⇒ 开关关着的站点上「拼错键必须 400 点名」
这条永远不会绿（不是判据假，是通道到不了）。闸门因此按开关读数切两条判据：
开关开着走活体点名判据，关着就钉住「确实被 4101 拦在前面」并 WARN 记未收口。
改共享设定要站方点头，闸门不自己翻开关。
"""
import io
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SF = os.path.join(ROOT, "apps", "api", "src")
WEB = os.path.join(ROOT, "apps", "web")

# 现网 latest 不含本批提交 ⇒ 开关若开着，活体点名判据本轮必然红
KNOWN_PENDING_DEPLOY = True


def psql(sql):
    p = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode != 0:
        raise RuntimeError("   psql 失败：%s / %s" % (sql[:60], p.stderr.strip()[:180]))
    return p.stdout.strip()


def strip_comments(path):
    if not os.path.exists(path):
        return ""
    out = []
    for line in io.open(path, encoding="utf-8", errors="replace"):
        i = line.find("//")
        if i >= 0 and '"' not in line[:i] and "'" not in line[:i]:
            line = line[:i]
        out.append(line)
    return "".join(out)


tr = strip_comments(os.path.join(SF, "social_http", "team.rs"))
m = re.search(r"struct CreateTeamReq \{(.+?)\n\}", tr, re.S)
DECL = m.group(1) if m else ""
FIELDS = re.findall(r"^\s*(\w+)\s*:", DECL, re.M)

print("== S1 代码层 ==")
ok("EJ1 CreateTeamReq 声明后紧贴 deny_unknown_fields",
   bool(m) and "deny_unknown_fields" in tr[:m.start()],
   "struct 前文=%r" % tr[max(0, m.start() - 60):m.start()] if m else "无结构体")
ok("EJ2 声明的字段就是面板要发的那两个（别把 size 改成必填）",
   FIELDS == ["torrent_id", "team_size_max"], str(FIELDS))

panel = strip_comments(
    os.path.join(WEB, "components", "endangered-panel.tsx"))
pm = re.search(r"social/team/create\",\s*\{([^}]*)\}", panel, re.S)
PK = re.findall(r"(\w+)\s*:", pm.group(1)) if pm else []
ok("EJ3 前端该口的载荷键逐个在后端声明里（面板只发 torrent_id）",
   bool(pm) and all(k in FIELDS for k in PK),
   "前端键=%s 后端=%s" % (PK, FIELDS))

print("== S2 活体判别（零写入）==")
TID = psql("SELECT t.id FROM torrents t WHERE t.approval_status = 1 "
           "AND NOT EXISTS (SELECT 1 FROM snatches s WHERE s.torrent_id = t.id"
           " AND s.user_id = 1) ORDER BY t.id LIMIT 1")
ok("S0 取到 root 未下载过的过审种（探针不可能走到建队 INSERT）",
   TID.isdigit(), "torrent_id=%s" % TID)
BASE_ROWS = psql("SELECT count(*) FROM social_team")
SOS = psql("SELECT value FROM site_settings WHERE name = 'module_social'")
tok = login()
ok("S1 root 登录", bool(tok))

s, r = call("POST", "/social/team/create",
            {"torrent_id": int(TID), "size_max": 8}, token=tok)
blob = json.dumps(r, ensure_ascii=False)
if SOS == "yes":
    ok("EJ4 笔误键 size_max 必须被拒并点名（修前：走进业务分支或直接建队，"
       "客户端看不出自己拼错）",
       s == 400 and r.get("code") == 1002 and "size_max" in blob
       and "未识别" in blob, "%s %s" % (s, blob[:150]))
else:
    ok("EJ4 开关关闭 ⇒ 本口在业务与反序列化之前就被 4101 拦住（读数 %s）"
       % SOS, s == 404 and r.get("code") == 4101,
       "%s %s" % (s, blob[:120]))
    print("   WARN 未收口：module_social=%s 时「拼错键要点名」这条活体打不到，"
          "开模块后改回点名判据复跑" % SOS)

s, r = call("POST", "/social/team/create", {"torrent_id": int(TID)},
            token=tok)
blob2 = json.dumps(r, ensure_ascii=False)
ok("EJ5 正控：面板原样的合法载荷不被新属性误拒（红要落在业务/开关，"
   "不是「未识别字段」）", "未识别" not in blob2, "%s %s" % (s, blob2[:150]))

s, r = call("POST", "/social/team/create",
            {"torrent_id": int(TID), "team_size_max": 8}, token=tok)
blob3 = json.dumps(r, ensure_ascii=False)
ok("EJ6 正控：显式给正确键名也不被误拒（省略与赋值两种形态都要过）",
   "未识别" not in blob3, "%s %s" % (s, blob3[:150]))

AFTER = psql("SELECT count(*) FROM social_team")
ok("EJ7 探针零写入（social_team 行数 %s -> %s）" % (BASE_ROWS, AFTER),
   BASE_ROWS == AFTER)

if KNOWN_PENDING_DEPLOY:
    print("   WARN 钉桩可撤：latest 含本批后 KNOWN_PENDING_DEPLOY 改 False")
print("   基址 %s" % BASE)
summary()
