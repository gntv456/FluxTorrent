# -*- coding: utf-8 -*-
"""常驻闸门：SQL 里 AS 出来的列名必须都是响应结构体的字段。

版主视角：举报队列里「已被 X 认领」那枚徽章永远不出现。
`admin_http/reports.rs` 的 SQL 早就写了 `cu.username AS claimed_name`，
但结构体 `ReportQueueRow` 漏了这个字段——**serde 只序列化结构体字段**，
于是面板 `staff-tools-reports.tsx:23/136` 读到的 `claimed_name` 是 undefined。
后果不是少个装饰：0285 起有「认领互斥」（别人认领过的单你 resolve 会被拒），
可队列上看不出这单在谁名下，版主只能点上去撞一下才知道。
2026-10-21 活体 A/B（同一条 status=2 的单）：
  现网(:8080) JSON 的键里没有 claimed_name；侧进程(:8099) 补字段后 = "root"。

判据（尺只判「AS 别名 ⊆ 结构体字段」这一条通用形状，不特判举报）：
  S1 每个目标文件里 AS 出来的别名，必须能在同文件的某个结构体里找到同名字段
  S2 正控：目标文件确实扫得到 ≥3 个别名（扫不到＝尺在看空文件，结论无意义）
  S3 面板侧真的读这个键（跨层钉：后端补字段是为了界面，界面不读就是死代码）
自测：--selftest 从结构体里删掉 claimed_name 那行 ⇒ S1 必须红；还原后必须绿。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_sql_alias_fields.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOT, "apps", "api", "src")
WEB = os.path.join(ROOT, "apps", "web")
# (后端文件, 面板读的键, 面板文件) —— 面板键为空串表示不跨层钉
TARGETS = [
    (os.path.join(API, "admin_http", "reports.rs"),
     "claimed_name",
     os.path.join(WEB, "components", "staff-tools-reports.tsx")),
]
# SQL 里 AS 给表达式起的名字里，这些不是「要回传给前端的列」
SQL_NOISE = {"text", "int", "int4", "int8", "bigint", "bool", "float8",
             "numeric", "jsonb", "timestamptz", "date", "interval", "uuid"}
ALIAS = re.compile(r"\bAS\s+([a-z_][a-z0-9_]*)\b")
STRUCT = re.compile(r"struct\s+\w+\s*\{([\s\S]*?)\n\}")
FIELD = re.compile(r"^\s*(?:#\[sqlx\(default\)\]\s*)?([a-z_][a-z0-9_]*)\s*:",
                   re.M)


def aliases_and_fields(path):
    src = io.open(path, encoding="utf-8", errors="replace").read()
    fields = set()
    for body in STRUCT.findall(src):
        fields.update(FIELD.findall(body))
    # 只认「AS 小写名」形状；CAST/类型名走 SQL_NOISE 挡掉
    al = {m for m in ALIAS.findall(src) if m not in SQL_NOISE}
    return al, fields


def main():
    for path, panel_key, panel_file in TARGETS:
        rel = os.path.relpath(path, ROOT).replace("\\", "/")
        al, fields = aliases_and_fields(path)
        missing = sorted(al - fields)
        ok("S1 %s：AS 别名 %d 个都落在结构体字段里（漏 %s）"
           % (rel, len(al), missing or "无"), not missing,
           "漏掉的别名会被 serde 静默丢掉，面板读到 undefined")
        ok("S2 正控：%s 确实扫到 %d 个别名（>3 才有意义）" % (rel, len(al)),
           len(al) > 3, "别名太少＝尺在看空实现，S1 的绿不算数")
        if panel_key:
            ps = io.open(panel_file, encoding="utf-8", errors="replace").read()
            ok("S3 面板 %s 真的读 %s（否则补字段是死代码）"
               % (os.path.basename(panel_file), panel_key),
               panel_key in ps, "后端有值、界面不读 ⇒ 这一层仍未接通")
    summary()


def selftest():
    path, _k, _f = TARGETS[0]
    src = io.open(path, encoding="utf-8").read()
    victim = "    claimed_name: Option<String>,\n"
    if src.count(victim) != 1:
        raise SystemExit("自测锚点不唯一：%d" % src.count(victim))
    with io.open(path, "w", encoding="utf-8", newline="") as f:
        f.write(src.replace(victim, ""))
    cmd = [sys.executable, os.path.abspath(__file__)]
    rc_flip = None
    try:
        r = subprocess.run(cmd, capture_output=True, text=True,
                           encoding="utf-8", errors="replace")
        rc_flip = r.returncode
        print("[自测] 抹掉结构体字段后 rc=%s（必须非 0）" % rc_flip)
        for l in [x for x in (r.stdout or "").splitlines() if "S1" in x][:2]:
            print("   ", l.strip()[:200])
    finally:
        with io.open(path, "w", encoding="utf-8", newline="") as f:
            f.write(src)
    back = subprocess.run(cmd, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    print("[自测] 还原后 rc=%s（必须 0）" % back.returncode)
    return 0 if (rc_flip not in (None, 0) and back.returncode == 0) else 2


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else main())
