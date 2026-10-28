# -*- coding: utf-8 -*-
"""常驻闸门 KT：会员面「改别人种子元数据」的写口必须先过归属判据。

2026-10-27 起因：`POST /textbooks/link` 只验登录，任何会员在 /textbooks
页面输一个别人的种子号就能改写它的教材归属（`torrents.textbook_id`），
未过审的种子也能被挂上；而现网 `textbooks` 空库时任何一次 link 都撞外键
回 HTTP 500 code=1000（实测取证）。修完顺手把这一族钉住——**判的是顺序**：
归属判据必须排在写入之前（本仓口径「权限闸装在业务校验之前」），
写在前面就是先动手再问该不该动。

台账三条（加一条同类写口就登记一条；不登记=本闸门看不见它）：
  KT1 每条的归属判据 + 写入点都取得到（取不到＝尺子瞎，结论作废）
  KT2 归属判据排在写入之前（早于写入的 Forbidden/门槛比较才算数）
  KT3 正控：修复前那版 textbook_link 必须被 KT2 判红，且写入点照样找得到
      （用 git show HEAD 的字节级旧码跑，不手抄旧实现）
KT4 单独钉 textbook 那把：教材号不存在必须当场 1004，不许把外键 violation
    演成 Internal（「自己写错的输入变成 500」一族）。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_kt_write_guard.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# (文件, 处理函数, 归属判据必需项, 写入点标记)
SPEC = [
    ("apps/api/src/content_http/misc.rs", "textbook_link",
     ("owner_id", "edit_threshold", "DomainError::Forbidden"),
     "UPDATE torrents SET textbook_id"),
    ("apps/api/src/publish_http/auto_meta.rs", "auto_meta",
     ("owner_id", "class_id", "DomainError::Forbidden"), "enrich_by_any_id"),
    ("apps/api/src/publish_http/collections.rs", "collection_add",
     ("owner_id", "class_id", "DomainError::Forbidden"),
     "INSERT INTO torrent_collections"),
]
MISC = "apps/api/src/content_http/misc.rs"
BOOK_MARK = "EXISTS(SELECT 1 FROM textbooks"


def strip(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in text.split("\n"))


def fn_body(text, name):
    m = re.search(r"(?m)^(?:pub(?:\([^)]*\))? )?(?:async )?fn %s\b" % name,
                  text)
    if not m:
        return ""
    d, out = 0, text[m.start():]
    for i, ch in enumerate(out):
        if ch == "{":
            d += 1
        elif ch == "}":
            d -= 1
            if d == 0:
                return out[:i + 1]
    return out


def judge(rel, fn, need, writer, text=None):
    """返回 (判据位置最小值, 写入点位置)；取不到给 None。"""
    if text is None:
        text = strip(io.open(os.path.join(ROOT, rel), encoding="utf-8",
                             errors="replace").read())
    body = " ".join(fn_body(text, fn).split())
    if not body:
        return None, None
    pos = [body.find(n) for n in need]
    if any(p < 0 for p in pos):
        return None, body.find(writer)
    return min(pos), body.find(writer)


print("== KT1/KT2 三条写口：判据取到、且排在写入之前 ==")
missing = []
late = []
for rel, fn, need, writer in SPEC:
    g, w = judge(rel, fn, need, writer)
    tag = "%s::%s" % (os.path.basename(rel), fn)
    if g is None or w < 0:
        missing.append("%s（判据=%s 写入点=%s）" % (tag, g, w))
    elif g > w:
        late.append("%s（判据在 %d，写入在 %d）" % (tag, g, w))
    print("   · %-34s 判据=%-6s 写入=%-6s" % (tag, g, w))
ok("KT1 三条写口的归属判据与写入点都取得到（缺=%s）" % (missing or "无"),
   not missing, "取不到＝尺子瞎，下面结论作废：" + str(missing))
ok("KT2 归属判据一律排在写入之前（晚=%s）" % (late or "无"), not late,
   "先动手再问该不该动＝越权写窗口" + str(late))

print("== KT3 正控：HEAD 那版 textbook_link 必须判红 ==")
old = subprocess.run(["git", "-C", ROOT, "show", "HEAD:%s" % MISC],
                     capture_output=True, text=True, encoding="utf-8",
                     errors="replace")
if old.returncode != 0:
    ok("KT3 正控：旧码取得到（git show HEAD 失败）", False,
       "rc=%s" % old.returncode)
else:
    og, ow = judge(MISC, "textbook_link",
                   ("owner_id", "edit_threshold", "DomainError::Forbidden"),
                   "UPDATE torrents SET textbook_id", strip(old.stdout))
    print("   [KT3 读数] 旧版判据=%s 写入点=%s" % (og, ow))
    ok("KT3 正控：旧版取不到归属判据、但写入点照样在（尺子量得到缺陷本体）",
       og is None and ow >= 0, "判据=%s 写入=%s" % (og, ow))

print("== KT4 教材号不存在必须 1004，不许演成 500 ==")
src = strip(io.open(os.path.join(ROOT, MISC), encoding="utf-8",
                    errors="replace").read())
body = " ".join(fn_body(src, "textbook_link").split())
ok("KT4 存在性与归属同一条查询判掉，缺教材回 NotFound 而非撞外键",
   BOOK_MARK in body and "DomainError::NotFound(body.textbook_id)" in body,
   "教材存在性=%s 404 出口=%s" % (BOOK_MARK in body,
                                 "NotFound(body.textbook_id)" in body))

summary()
