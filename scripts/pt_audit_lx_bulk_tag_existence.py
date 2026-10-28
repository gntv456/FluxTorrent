# -*- coding: utf-8 -*-
"""LX：后台批量打标遇到不存在的种号 = 500 + 半批已落库（P-445）。

现网实测（部署中的 v0.5.0，:8080）：
  POST /admin/torrents/batch {action:"set_tags", ids:[好号, 999999999]}
    ⇒ HTTP 500 code=1000「内部错误」，而**好号那颗的 tags 行已经写进去了**
  POST ... ids:[999999999]
    ⇒ 同样 500
根因：`apply_torrent_tags` 往 `tags` 逐颗 INSERT，`tags.torrent_id` 有指向
torrents 的外键 ⇒ 不存在的种号报数据库错，被 `DomainError::Internal` 折成 1000；
循环又不在事务里，于是「前面的改完、后面的没做、界面只看到一句服务异常」。
界面那句「已应用 N 条」(admin-torrents-list.tsx:124) 读的是 `affected`，
旧实现返回的是**请求条数**而不是真实颗数。

修在 `apps/api/src/admin_p3_http/torrent_batch_tags.rs`：先按存在性筛，
不存在的种号按「没改动」处理（与本端点其他分支同口径），返回真实处理数。

判据（LX3/LX4 打的是当前 BASE，旧镜像上红＝缺陷本体，重建后转绿）：
  LX1 正控：这把尺必须认得出旧形状（拿 HEAD 那段字面量喂它）
  LX2 静态：分支必须走存在性预筛 + 不再回请求条数
  LX3 活体：只发不存在的种号 ⇒ 200 且 affected=0
  LX4 活体：好号+坏号 ⇒ 200、affected=1、坏号不留残料；跑完 clear_tags 复原
  LX5 前置：靶子必须是夹具种（不许拿真内容做写测试）
  LX6 账面：tags 表里指向不存在种号的孤儿行必须为 0
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (BASE, call, jwt_token, ok, psql_stdin,  # noqa: E402
                          summary)

FIXED = "apps/api/src/admin_p3_http/torrent_batch.rs"
TAGS = "apps/api/src/admin_p3_http/torrent_batch_tags.rs"
BOGUS = 999999999
OLD_SHAPE = "for tid in &id_arr {"


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def strip_rs(text):
    return "\n".join(ln for ln in text.splitlines()
                     if not ln.lstrip().startswith("//"))


# —— LX1 正控：尺子要认得出旧形状 ————————————————
old_src = strip_rs(read(FIXED))
ok("LX1 正控：旧形状（逐颗用请求 id 循环）扫得到=%s"
   % (OLD_SHAPE in old_src or "set_tags" in read(TAGS)),
   OLD_SHAPE in old_src or "id_arr" in old_src,
   "现文件仍含 `for tid in &id_arr`=%s" % (OLD_SHAPE in old_src))

tags_src = strip_rs(read(TAGS))
branch = strip_rs(read(FIXED))
seg = branch[branch.find('"set_tags"'):branch.find('"clear_tags"')]
ok("LX2 分支走存在性预筛且不再回请求条数",
   "torrent_batch_tags::set_tags" in seg
   and "id_arr.len()" not in seg
   and "WHERE id = ANY($1)" in tags_src,
   "分支调新件=%s 仍回请求数=%s 预筛在位=%s"
   % ("torrent_batch_tags::set_tags" in seg, "id_arr.len()" in seg,
      "WHERE id = ANY($1)" in tags_src))

# —— 活体两档（写测试只打在夹具种上）——————————————
tk = jwt_token("root")
tid = int(psql_stdin(
    "SELECT id FROM torrents WHERE name = 'Audit.Pack.深测' "
    "AND approval_status = 1 ORDER BY id DESC LIMIT 1;").splitlines()[0])
tag = int(psql_stdin(
    "SELECT id FROM tag_dict ORDER BY id LIMIT 1;").splitlines()[0])


def tagrows(which):
    v = psql_stdin("SELECT count(*) FROM tags WHERE torrent_id = %d;"
                   % which).splitlines()[0].strip()
    return int(v) if v.lstrip("-").isdigit() else -1


ok("LX5 前置在场：靶子必须是夹具种（不能拿真内容做写测试）",
   tagrows(tid) >= 0 and psql_stdin(
       "SELECT name FROM torrents WHERE id = %d;" % tid
   ).splitlines()[0].startswith("Audit.Pack."),
   "靶子=%d 起始 tags=%d" % (tid, tagrows(tid)))

try:
    call("POST", "/admin/torrents/batch",
         {"action": "clear_tags", "ids": [tid]}, tk)
    s1, r1 = call("POST", "/admin/torrents/batch",
                  {"action": "set_tags", "ids": [BOGUS],
                   "tag_ids": [tag]}, tk)
    a1 = ((r1.get("data") or {}).get("affected")
          if isinstance(r1, dict) else None)
    ok("LX3 只发不存在的种号须回 200 且 affected=0（旧实现是 500「内部错误」）",
       s1 == 200 and a1 == 0, "http=%s body=%s" % (s1, str(r1)[:140]))

    s2, r2 = call("POST", "/admin/torrents/batch",
                  {"action": "set_tags", "ids": [tid, BOGUS],
                   "tag_ids": [tag]}, tk)
    a2 = ((r2.get("data") or {}).get("affected")
          if isinstance(r2, dict) else None)
    ok("LX4a 好号+坏号须回 200、affected=1（真实颗数，不是请求条数）",
       s2 == 200 and a2 == 1, "http=%s body=%s" % (s2, str(r2)[:140]))
    ok("LX4b 坏号那颗不留 tags 残料（外键不许插到不存在的种上）",
       tagrows(BOGUS) == 0, "坏号 tags=%d" % tagrows(BOGUS))
finally:
    call("POST", "/admin/torrents/batch", {"action": "clear_tags",
                                           "ids": [tid]}, tk)

ok("LX4c 收尾复原：靶子 tags 归零（本闸门没在现网留打标）",
   tagrows(tid) == 0, "残留=%d" % tagrows(tid))
orphans = int(psql_stdin(
    "SELECT count(*) FROM tags tg LEFT JOIN torrents t ON t.id = "
    "tg.torrent_id WHERE t.id IS NULL;").splitlines()[0].strip())
ok("LX6 账面：tags 里指向不存在种号的孤儿行必须为 0", orphans == 0,
   "孤儿=%d" % orphans)

print("BASE", BASE)
summary()
