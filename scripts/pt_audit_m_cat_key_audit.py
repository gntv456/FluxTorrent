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

# ---- M5：内置包分类数 vs 资源库导航上限（LIB_CAT_CAP）----
# 0455 把综合站加到 13 类，而上限当时是 12 ⇒ 「教育资源」从导航里被截掉，
# 只能去列表页筛。内置分类被自己的常量截掉不是降级设计，是产品缺角。
# 上限读前端常量（不在此写死第二个数），两边任一改动都会在这里对上。
import os  # noqa: E402
import re  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
nav_src = open(os.path.join(ROOT, "apps", "web", "lib", "nav-menu.ts"),
               encoding="utf-8", errors="replace").read()
mcap = re.search(r"LIB_CAT_CAP\s*=\s*(\d+)", nav_src)
cap = int(mcap.group(1)) if mcap else 0
packs = []
PACK_COUNTS_SQL = ("SELECT code || '|' || jsonb_array_length(categories) "
                   "FROM site_type_packs ORDER BY "
                   "jsonb_array_length(categories) DESC, code")
for ln in psql(PACK_COUNTS_SQL).splitlines():
    if "|" in ln:
        code, n = ln.split("|")
        packs.append((code.strip(), int(n)))
over = ["%s=%d 类 > 上限 %d" % (c, n, cap) for c, n in packs if n > cap]
if not mcap:
    over.append("nav-menu.ts 里找不到 LIB_CAT_CAP（常量被改名/删除，判据失效）")
print("M5 导航上限 %s；最大内置包 %s" % (
    cap, ("%s=%d" % packs[0]) if packs else "?"))

# ---- M6：categories 用到的每个 newznab 号都必须在 cat_map.rs 有名 ----
# caps 按库里的号逐条播（recon.rs::torznab_caps），未命中的号落
# `_ => "Other"`。现网实测曾同时播 `<cat id="7010" name="Other">`、
# `<cat id="7030" name="Other">`、`<cat id="8000" name="Other">`——三条
# 同名：工具按名字挑目录会挑错，站长按名字也看不出映射漏了。
# 判据只从代码里取号（不在此抄第二份名单），两边任一边改动都会对上。
cm_path = os.path.join(ROOT, "apps", "api", "src", "torrents", "cat_map.rs")
cm_src = open(cm_path, encoding="utf-8", errors="replace").read()
named = {int(x) for x in
         re.findall(r"^\s+(\d{3,7}) => \"", cm_src, re.M)}
used = {int(x) for x in psql(
    "SELECT DISTINCT newznab_id FROM categories "
    "WHERE newznab_id IS NOT NULL").split() if x.isdigit()}
nameless = sorted(used - named)
print("M6 库里在用分类号 %d 个；cat_map.rs 有名 %d 个"
      % (len(used), len(used & named)))

# ---- M7：包声明与表实存逐字段对账（name/sort/icon/bg/newznab）----
# 0455 重写电子书包 payload 时把「杂志 / 漫画」的 newznab_id 压成 7020，
# 而表里仍是 0325 起的 7010 / 7030。M1–M4 只比 newznab 一个维度就抓到了
# 这一条，但 name / sort / icon_key / bg_color 分叉同样会让「包说的」和
# 「站点实际长出来的」两套口径各走各的——下一次按包重物化（切站型、保存
# 包、内容包导入）会把表覆盖成包里的值，错的那份转正。
FIELD_SQL = """
SELECT pk.code, p.cat->>'key',
       COALESCE(c.name, ''), COALESCE(p.cat->>'name', ''),
       COALESCE(c.sort::text, ''), COALESCE(p.cat->>'sort', ''),
       COALESCE(c.icon_key, ''), COALESCE(p.cat->>'icon_key', ''),
       COALESCE(c.bg_color, ''), COALESCE(p.cat->>'bg_color', ''),
       COALESCE(c.newznab_id::text, ''), COALESCE(p.cat->>'newznab_id', '')
  FROM site_type_packs pk,
       jsonb_array_elements(pk.categories) AS p(cat)
  JOIN categories c ON c.key = p.cat->>'key'
 ORDER BY pk.code, (p.cat->>'sort')::int;
"""
FIELDS = ["name", "sort", "icon_key", "bg_color", "newznab_id"]
diverge = []
for ln in psql(FIELD_SQL).splitlines():
    f = ln.split("|")
    if len(f) < 12:
        continue
    bad = []
    for i, fld in enumerate(FIELDS):
        # 列序是成对排布：2/3 name、4/5 sort、6/7 icon、8/9 bg、10/11 号
        cval = (f[2 + 2 * i] or "").strip()
        pval = (f[3 + 2 * i] or "").strip()
        if cval and pval and cval != pval:
            bad.append("%s：表=%s／包=%s" % (fld, cval, pval))
    if bad:
        diverge.append("%s/%s：%s" % (f[0], f[1], "；".join(bad)))

# 0457 修的就是 ebook 包这两行；迁移只在 api 启动时跑，镜像没带上之前
# 台账不可能有 457——此时报 FAIL 会把「等部署」误报成「产品洞」，所以按
# 在册与否分档（同 OF 闸门的 PENDING 口径）。豁免只认这两条点名的行，
# 分叉出第三条照样红。
PENDING_0457 = {"ebook/ebook_mag", "ebook/ebook_comic"}
mig_max = int(psql("SELECT COALESCE(max(version),0) FROM _sqlx_migrations"))
fix_in = mig_max >= 457


def split_pending(items):
    """0457 未应用时把点名的两条挪进 NOTE，其余照旧算 FAIL。"""
    if fix_in:
        return items, []
    live, note = [], []
    for it in items:
        (note if it.split("：")[0] in PENDING_0457 else live).append(it)
    return live, note


mismatch, mismatch_pending = split_pending(mismatch)
diverge, diverge_pending = split_pending(diverge)
pending = mismatch_pending + diverge_pending

if missing:
    print("[FAIL] %d 条 key 缺失：" % len(missing))
    for m in missing[:10]:
        print("   " + m)
if mismatch:
    print("[FAIL] %d 条 newznab_id 错配：" % len(mismatch))
    for m in mismatch[:10]:
        print("   " + m)
if over:
    print("[FAIL] %d 个包的分类数超过导航上限（该包分类在导航里被截）："
          % len(over))
    for o in over[:10]:
        print("   " + o)
if nameless:
    print("[FAIL] %d 个分类号在 cat_map.rs 名字表里没有名（caps 里全播成 "
          "Other）：" % len(nameless))
    for n in nameless[:10]:
        print("   %d" % n)
if diverge:
    print("[FAIL] %d 条包声明与表实存字段分叉：" % len(diverge))
    for d in diverge[:10]:
        print("   " + d)
if pending:
    print("[NOTE] %d 条由 0457 修（台账最大 %d，尚未应用；0457 落地后"
          "本条须归零，仍报即为新分叉）：" % (len(pending), mig_max))
    for p in pending[:10]:
        print("   " + p)
if not missing and not mismatch and not over and not nameless \
        and not diverge:
    print("== ALL PASS（key 缺失 0、newznab 错配 0、导航截断 0、"
          "无名分类号 0、包↔表分叉 0、待 0457 %d 条）==" % len(pending))
    raise SystemExit(0)
raise SystemExit(1)
