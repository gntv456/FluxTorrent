# -*- coding: utf-8 -*-
"""闸门 JB：档位型设置的「实现 / 面板 / 现值」三本账必须自洽（批二百四十四）。

起因（本轮巡检红账的共同根因，实测）：两支**未入库**的审链脚本把
`upload_dup_policy` 写成 `warn`——一个代码根本不认的档位。读侧
`publish_http/upload_precheck.rs::dup_policy` 旧写法是
「只判 suggest，其余一路走到兜底 Err(「本站重复发布策略为拦下」)」，
于是整站从那一刻起按最严档拒绝同内容重发，而后台面板显示的仍是 `warn`：
**行为与设置两本账**。本轮 23 支发种类闸门集体卡在
`400 参数校验失败: 同内容种子已存在…策略为「拦下」` 上，就是这条。

判据分层（先自证尺，再谈结论）：
  JB1 尺自证：options 有三种历史形状（[{v|value|key}] 数组、
      {value: label} 对象映射、{"options": [...]} 包装）。认不出的形状
      必须点名并计入 skipped——把「尺瞎了」当「越界」是上一批踩过的坑。
  JB2 现值越界集合必须为空（多一把红、少一把也红；已登记的键改不回就红）。
  JB3 静态：dup_policy 必须有「未登记档位 ⇒ 放行 + 出声」的分支（P-510 的钉，
      退回旧写法即红）。
  JB4 实现档位集合 == 面板 options 集合（面板加档而实现没接 = 假开关；
      实现有档而面板没登记 = 站长摸不到）。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_jb_setting_enum.py
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

PRECHECK = "apps/api/src/publish_http/upload_precheck.rs"
DUP_KEY = "upload_dup_policy"
FACTORY = "suggest"  # 0286 迁移的出厂档


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-F", "|",
         "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("psql 失败：%s" % (r.stderr or "").strip()[:250])
    return (r.stdout or "").strip()


def cut(src):
    """剥 // 行注释与 /* */ 块注释（注释里写的档位名不算实现接了）。"""
    s = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


# ── 面板侧：把三种历史形状统一抽成「值集合」──────────────
def opt_values(raw):
    """返回 (values, shape)。认不出的形状返回空集合 + shape='unknown'。"""
    import json
    try:
        j = json.loads(raw)
    except Exception:  # noqa: BLE001
        return [], "unparsable"
    if isinstance(j, dict) and isinstance(j.get("options"), list):
        j = j["options"]
    if isinstance(j, dict):
        return sorted(j.keys()), "map"
    if isinstance(j, list):
        vals = []
        for o in j:
            if isinstance(o, dict):
                v = o.get("v") or o.get("value") or o.get("key")
                if v:
                    vals.append(v)
                else:
                    return [], "array-nov"
            else:
                return [], "array-scalar"
        return sorted(vals), "array"
    return [], "unknown"


rows = psql(
    "SELECT m.name || '#' || s.value || '#' || m.options::text "
    "FROM settings_meta m JOIN site_settings s USING (name) "
    "WHERE m.type = 'enum' AND m.options IS NOT NULL "
    "ORDER BY m.name")
panel, skipped, stray = {}, [], []
for line in rows.split("\n"):
    if not line.strip() or line.count("#") < 2:
        continue
    name, value, opts = line.split("#", 2)
    vals, shape = opt_values(opts)
    if shape in ("unparsable", "unknown", "array-nov", "array-scalar"):
        skipped.append((name, shape))
        continue
    panel[name] = (vals, value, shape)
    if value not in vals:
        stray.append((name, value, vals))

shapes = {}
for _n, (_v, _cur, sh) in panel.items():
    shapes[sh] = shapes.get(sh, 0) + 1
print("   [尺自证] 认得形状的 enum 键 %d 把（%s）；认不出形状 %d 把 %s"
      % (len(panel), shapes, len(skipped), skipped[:6]))
ok("JB1 普查尺认得面板 options 的全部历史形状（认不出的必须点名）",
   len(panel) > 0 and not skipped,
   "skipped=%s ⇒ 这些键的越界判定根本没跑" % skipped)
ok("JB2 现值必须落在面板登记的档位集合内（在册清单为空）",
   not stray, "越界=%s" % stray)

# ── JB3 读侧兜底：未登记档位不得静默升到最严档 ────────────
src = cut(io.open(os.path.join(ROOT, PRECHECK), encoding="utf-8",
                  errors="replace").read())
m = re.search(r"async fn dup_policy\(.*?\n\}\n", src, re.S)
body = m.group(0) if m else ""
guarded = (re.search(r'matches!\s*\(\s*policy\.as_str\(\)\s*,', body)
           is not None and "tracing::warn" in body)
# 兜底 Err 只应在 block 档到达：suggest/group 都在前面 return 掉了
returns = body.count("return Ok(DupVerdict")
ok("JB3 dup_policy 对未登记档位必须「按出厂档放行 + 出声」（P-510 钉）",
   body != "" and guarded and returns >= 3,
   "函数体抽到=%s；matches!+warn=%s；显式放行分支=%d"
   % (bool(body), guarded, returns))

# ── JB4 实现档位集合 == 面板档位集合 ─────────────────────
impl = set(re.findall(r'policy == "(\w+)"', body))
for chunk in re.findall(r'matches!\s*\(\s*policy\.as_str\(\)\s*,\s*([^)]+)\)',
                        body):
    impl |= set(re.findall(r'"(\w+)"', chunk))
impl |= {FACTORY} if FACTORY in body else set()
dup_panel, dup_cur, dup_shape = panel.get(DUP_KEY, (sorted(impl), "", "?"))
print("   [读数] %s：实现认得=%s 面板登记=%s 现值=%s（形状=%s）"
      % (DUP_KEY, sorted(impl), dup_panel, dup_cur, dup_shape))
ok("JB4 %s 的实现档位集合 == 面板 options 集合（缺一边都是账）" % DUP_KEY,
   impl and sorted(impl) == sorted(dup_panel) and dup_cur in dup_panel,
   "实现=%s 面板=%s 现值=%s" % (sorted(impl), dup_panel, dup_cur))

summary()
