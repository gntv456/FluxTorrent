# -*- coding: utf-8 -*-
"""P-335 防复发审计：categories.key 与包载荷 key 的对应关系（只报告，不改数据）。

0317 的 key 回填按 `(p.cat->>'id')::int = c.id` 匹配——而包里的 id 只是「空表
replace 时的建议序号」（pack_core.rs 自陈「不能拿来直接 INSERT」）。跨包切换
时旧行占着同号主键，于是 music_single 这个 key 被贴到当时 newznab_id=7020
（电子书）的那一行上，对外科号错配。

0377 迁移把值改回 3030，但那是治标：merge 模式的 apply 守旧值
（`COALESCE(categories.newznab_id, EXCLUDED.newznab_id)`，这是既定契约——
站长手改过的号不该被包应用抹掉），所以问题若复发会一直藏着。本脚本把错配
直接揪出来，让复发当天可见。

范围：只查**已物化**的分类（当前站型，或表里已经存在该 key）。未激活站的包
本就没有对应行（实测 education/software 14 条），那不是洞。
"""
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAF", "|", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise SystemExit("psql failed: %s" % (r.stderr or "")[:200])
    return (r.stdout or "").strip()


CURRENT = psql("SELECT COALESCE((SELECT value FROM site_settings "
               "WHERE name='site_type'),'')")

SQL = """
SELECT pk.code, p.cat->>'key', p.cat->>'newznab_id',
       COALESCE(c.id::text,''), COALESCE(c.newznab_id::text,'')
  FROM site_type_packs pk,
       jsonb_array_elements(pk.categories) AS p(cat)
  LEFT JOIN categories c ON c.key = p.cat->>'key'
 WHERE p.cat ? 'key'
   AND (pk.code = '%s'
        OR EXISTS (SELECT 1 FROM categories c2 WHERE c2.key = p.cat->>'key'))
 ORDER BY pk.code, p.cat->>'key';
""" % CURRENT.replace("'", "''")

rows = [ln for ln in psql(SQL).splitlines() if ln.strip()]
missing, mismatch = [], []
for ln in rows:
    f = ln.split("|")
    if len(f) < 5:
        continue
    code, key, pnewnz, cid, cnewnz = f[:5]
    if not cid:
        missing.append("%s/%s（当前站型 %s 已激活但表里没这个 key）"
                       % (code, key, CURRENT))
        continue
    if pnewnz and cnewnz and pnewnz != cnewnz:
        mismatch.append("%s/%s：包声明 %s，表里 %s（key 贴错行 → 对外科号错配）"
                        % (code, key, pnewnz, cnewnz))

print("已物化分类 %d 条（当前站型 %s）" % (len(rows), CURRENT))
if missing:
    print("[FAIL] %d 条 key 缺失：" % len(missing))
    for m in missing[:10]:
        print("   " + m)
if mismatch:
    print("[FAIL] %d 条 newznab_id 错配：" % len(mismatch))
    for m in mismatch[:10]:
        print("   " + m)
if not missing and not mismatch:
    print("== ALL PASS（key 缺失 0、newznab 错配 0）==")
    raise SystemExit(0)
raise SystemExit(1)
