# -*- coding: utf-8 -*-
"""闸门 FF：论坛已读标记链（0078 NP readposts）＋ 界面缺通道钉桩。

后端每打开一次主题就往 topic_reads 记一行「读到哪楼」，版块列表据此算
`has_unread` / `read_last_post_id`。10-21 01:05 实测：root 打开主题 475 之前
has_unread=true，之后=false 且标记推进到 644 ⇒ **API 这一层是通的**。
但 apps/web 全仓对这两把键的引用数是 0（页面/组件/测试都算）——「有新回复」
提示从来没进界面，用户读没读过在列表上长得一模一样。

四条判据（各钉一件事）：
  FF1 活体链：找一枚当前未读且 root 原本没有标记行的主题 → 打开 → 复读必须
     熄灭且标记推进；收尾只删本批造出来的那一行（原本有标记的主题不许碰，
     免得抹掉真实浏览史），并数残留。
  FF2 活体口径：列表每行必须仍带这两把键（哪天后端不算了，FF3 的「没人读」
     就不成立，两笔账要对上）。
  FF3 钉桩（未收口清单）：前端读者数必须恰为 0。接上界面那天这条会红——
     销桩姿势：删掉本条断言，换一条「渲染层真的画出未读标记」的判据。
  FF4 待裁形状：受保护版块把 2 楼起的正文换成提示（class≥90／楼主／本版版主
     之外看不见），而写标记那句取的是**本页全部帖子**的 max(id) ⇒ 看不见
     内容的人被记成「已读到最新楼」，此后不再有任何提醒。本条只钉形状不判
     对错：要么标记不越过看不见的楼，要么角标一直亮着——两个都是产品口径。

跑法（仓库根目录）：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_ff_forum_readmark.py
侧进程：FLUX_API_BASE=http://127.0.0.1:8099/api/v1 python -u \\
    scripts/pt_audit_ff_forum_readmark.py
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import call, login, ok, psql_stdin, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KEYS = ("has_unread", "read_last_post_id")


def q(sql):
    return psql_stdin(sql).strip()


tok = login()
print("== FF0 地基：root 会话与一块有未读主题的版面 ==")
if not tok:
    ok("FF0 拿到 root 会话", False, "login() 返回空 token")
    summary()
    sys.exit(1)

sample = None  # (fid, tid, 打开前的行)
for fid in range(1, 13):
    st, body = call("GET", "/forums/%d/topics?limit=20" % fid, token=tok)
    if st != 200:
        continue
    rows = ((body or {}).get("data") or {}).get("topics") or []
    cands = [t for t in rows if t.get("has_unread") is True
             and t.get("read_last_post_id") is None]
    if cands:
        sample = (fid, cands[0].get("id"), cands[0])
        break
if not sample:
    ok("FF1 取样：找到一枚「未读且 root 无标记行」的主题", False,
       "前 12 块版面 20 行内没有样本（数据依赖：换版面或先清 root 标记）")
    summary()
    sys.exit(1)
FID, TID, BEFORE = sample
print("   样本：版块 %d / 主题 %d /%s/ 打开前 has_unread=%s 标记=%s"
      % (FID, TID, str(BEFORE.get("title"))[:26], BEFORE.get("has_unread"),
         BEFORE.get("read_last_post_id")))

st2, b2 = call("GET", "/forums/topics/%d" % TID, token=tok)
ok("FF1 打开主题这一跳本身成功（http=%s code=%s）" % (st2, (b2 or {}).get("code")),
   st2 == 200 and (b2 or {}).get("code") == 0, str(b2)[:120])

st3, b3 = call("GET", "/forums/%d/topics?limit=20" % FID, token=tok)
after = [t for t in (((b3 or {}).get("data") or {}).get("topics") or [])
         if t.get("id") == TID]
A = after[0] if after else {}
marked = q("SELECT COALESCE(last_post_id, -1) FROM topic_reads "
           "WHERE user_id = (SELECT id FROM users WHERE username = 'root') "
           "AND topic_id = %d" % TID)
ok("FF1 复读列表：has_unread 必须熄灭（实测 %s）且后端标记 > 0（实测 %s）"
   % (A.get("has_unread"), marked),
   A.get("has_unread") is False and int(marked) > 0,
   "行=%s 库=%s" % (str(A)[:110], marked))

keys = sorted(A.keys())
ok("FF2 列表每行仍带这两把键（口径在册）", all(k in keys for k in KEYS),
   ",".join(keys))

# ---- FF3 钉桩：前端读者数恰为 0（接上界面那天必须红一次） ----
readers = []
for dp, dn, fn in os.walk(os.path.join(ROOTDIR, "apps", "web")):
    dn[:] = [d for d in dn if d not in (".next", "node_modules")]
    for f in fn:
        if not f.endswith((".ts", ".tsx")):
            continue
        p = os.path.join(dp, f)
        t = io.open(p, encoding="utf-8", errors="replace").read()
        if any(k in t for k in KEYS):
            readers.append(os.path.relpath(p, ROOTDIR).replace("\\", "/"))
ok("FF3 钉桩：论坛未读标记仍未接进界面（apps/web 读者数=0；现=%d）"
   % len(readers), not readers,
   "销桩姿势：接上界面后删掉本条断言，换一条渲染层判据；命中=%s"
   % (readers[:4],))

# ---- FF4 待裁形状：隐藏正文在前、推进标记在后 ----
def strip_rs(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    out = []
    for line in text.split("\n"):
        if "http://" in line or "https://" in line:
            out.append(line)
            continue
        cut = re.search(r"(?<!:)//", line)
        out.append(line[:cut.start()] if cut else line)
    return "\n".join(out)


PR = strip_rs(io.open(os.path.join(ROOTDIR, "apps", "api", "src",
                                  "community_http", "post_read.rs"),
                      encoding="utf-8").read())
i_hide = PR.find("if protected {")
i_mark = PR.find("posts.iter().map(|p| p.id).max()")
ok("FF4 待裁形状在册：受保护版块先隐藏正文、随后仍按整页 max(id) 推进标记"
   "（隐藏位=%d 标记位=%d）" % (i_hide, i_mark),
   i_hide >= 0 and i_mark > i_hide,
   "改这一处时先看 FF4 的两种口径，别默认「推进」是对的；"
   "看不见的楼也记成已读 ⇒ 该读者此后不会有任何提示")
if i_hide >= 0 and i_mark > i_hide:
    print("   WARN FF4b 现网读数：root 名下 topic_reads 行数=%s，"
          "其中受保护版面的标记无法区分「真读过」与「被隐藏」"
          % q("SELECT count(*) FROM topic_reads WHERE user_id = "
              "(SELECT id FROM users WHERE username = 'root')"))

# ---- 收尾：只删本批造出来的那一行，并数残留 ----
q("DELETE FROM topic_reads WHERE user_id = "
  "(SELECT id FROM users WHERE username = 'root') AND topic_id = %d" % TID)
left = q("SELECT count(*) FROM topic_reads WHERE user_id = "
         "(SELECT id FROM users WHERE username = 'root') AND topic_id = %d"
         % TID)
ok("FF9 收尾：本批给主题 %d 造的标记行已删净（残留=%s）" % (TID, left),
   left == "0", "root 其余浏览史未动")

summary()
