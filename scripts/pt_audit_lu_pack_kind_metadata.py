# -*- coding: utf-8 -*-
"""LU：站型包「另存快照 / 回滚基线」必须带走维度元数据（九列同形）。

缺陷本体（P-440，2026-10-27 实测）：`pack_snapshot::collect()` 自己拼
`json!({"kind","label","sort"})`，只 SELECT 三列。`settings_http/pack_kinds.rs`
头注写的是**四处同形**纪律（导出、快照、导入解析、落库），07b246f8 那一批
把分类学包两处（pack_format/pack_store）收敛进了 `kind_to_json`，站型包
快照这第四处漏了。后果链：`kind_from_json` 对**缺键**静默补 `select`
（对**非法值**才 Err），`apply_pack_kinds` 对已存在的行又不改 field_type
⇒ 只有「新站点/新库 apply」和「回滚把被删维度重建」两条路上，text/number/
bool/multiselect 维度被降级成 select；而 select 的取值必须命中 options，
这些维度本就靠自由值（`torrent_sections`）存 ⇒ 表单与筛选当场全部失明。

现网 A/B（同一支脚本，只换 FLUX_API_BASE）：
  :8080（部署中的 v0.5.0 镜像）→ 另存包 55 个维度里 **0** 个带 field_type；
  :8099（本批侧进程，含修复）  → **55/55 带且与库中类型逐一对齐**。
所以 LU4 在旧镜像上是红的，红的就是缺陷本体（同 P-433/HZ 先例）。

顺带查清并**排除**的一条候选：预置包（site_type_packs）也有维度不带
field_type，但它们在该包自己的 `sections.dict` 里都带选项 ⇒ 默认 select
正是作者意图，不是降级。LU6 把这条正确的判据钉住（不带 field_type 就必须
带选项），免得下一轮又有人拿「省略即缺陷」来翻案。

跑法：python scripts/pt_audit_lu_pack_kind_metadata.py
（LU4 要现网 API：默认 :8080，侧进程用 FLUX_API_BASE 覆盖；探针包写完即删，
 收尾断言残料=0。）
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (BASE, call, jwt_token, ok, psql_stdin,  # noqa: E402
                          summary)

SNAP = "apps/api/src/staff_http/pack_snapshot.rs"
KINDS = "apps/api/src/settings_http/pack_kinds.rs"
APPLY = "apps/api/src/staff_http/pack_types.rs"
NINE = ["kind", "label", "sort", "field_type", "required", "multiple",
        "enabled", "icon_key", "bg_color"]
CODE = "custom_lu4probe"


def read(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read()


def strip_rs_comments(text):
    """剥整行 // 与 /// 注释再扫——我自己写的说明行里就有「kind/label/sort
    三列」字样，不剥会把自己判成违例（同 AM 闸门那把尺）。"""
    return "\n".join(ln for ln in text.splitlines()
                     if not ln.lstrip().startswith("//"))


# 旧形状的字面量标志：三列手拼。用精确串而不是「有没有 field_type」来判——
# 后者会被同文件里 SQL 串中的列名洗白（全文 find 的经典假绿）。
HAND = '"kind": kind, "label": label, "sort": sort'


def hand_rolled(text):
    return HAND in strip_rs_comments(text)


def rs_files(root="apps/api/src"):
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in ("target", ".next")]
        out += [os.path.join(dirpath, f).replace("\\", "/")
                for f in filenames if f.endswith(".rs")]
    return out


def q(sql):
    """单值查询：取第一行第一列。"""
    out = psql_stdin(sql).strip()
    return out.splitlines()[0].split("|")[0].strip() if out else ""


# —— LU1 尺子自己要先能抓到旧形状（正控）—————————————
OLD = '''
        json!({
            "kinds": kinds
                .iter()
                .map(|(kind, label, sort)| {
                    json!({"kind": kind, "label": label, "sort": sort})
                })
                .collect::<Vec<_>>(),
            "dict": dict,
        })
'''
ok("LU1a 正控：这把尺扫得到三列手拼的旧形状",
   hand_rolled(OLD), "命中=%s" % hand_rolled(OLD))
ok("LU1b 采集侧不再手拼三列 kinds",
   not hand_rolled(read(SNAP)), "现扫=%s" % hand_rolled(read(SNAP)))

# —— LU2 采集侧走共享形状，且真的 SELECT 九列 ——————————
snap_src = read(SNAP)
ok("LU2a 快照走 pack_kinds::kind_to_json（四处只调用不复制）",
   "kind_to_json" in snap_src and "KindRow" in snap_src,
   "kind_to_json=%s KindRow=%s" % ("kind_to_json" in snap_src,
                                   "KindRow" in snap_src))
sel = strip_rs_comments(snap_src)
_at = sel.find("FROM section_kinds")
kinds_sql = sel[_at - 400:_at] if _at != -1 else ""
missing = [c for c in NINE if c not in kinds_sql]
ok("LU2b 快照的 kinds 查询把九列都选出来",
   "FROM section_kinds" in sel and not missing,
   "缺列=%s 取到片段=%s" % (missing, kinds_sql[-90:].replace("\n", " ")))

# —— LU3 三个「包 JSON 生产者」同形，全站不留手拼 ————————
producers = {
    "站型包快照": SNAP,
    "分类学包格式化": "apps/api/src/settings_http/pack_format.rs",
    "分类学包入库": "apps/api/src/settings_http/pack_store.rs",
}
bad_sites = [n for n, p in producers.items()
             if "kind_to_json" not in read(p) or hand_rolled(read(p))]
ok("LU3a 三个 kinds JSON 生产点都只调 kind_to_json",
   not bad_sites, "违例=%s" % bad_sites)
srcs = rs_files()
hits = [p for p in srcs if hand_rolled(read(p))]
ok("LU3b 全仓不再残留三列手拼的维度 JSON",
   not hits, "命中=%s 基数=%d 个 .rs" % (hits, len(srcs)))

# —— LU4 活体：另存快照真的带元数据（红=缺陷本体在旧镜像）——
tk = jwt_token("root")
psql_stdin("DELETE FROM site_type_packs WHERE code = '%s';" % CODE)
try:
    st, resp = call("POST", "/admin/site-type-packs/save",
                    {"code": CODE, "name": "LU 探针包"}, tk)
    ok("LU4a 另存快照接口可用（200）", st == 200,
       "http=%s code=%s msg=%s" % (st, (resp or {}).get("code"),
                                   str((resp or {}).get("message"))[:60]))
    ROW = """
SELECT count(*) AS n,
       count(*) FILTER (WHERE e ? 'field_type')   AS has_ft,
       count(*) FILTER (WHERE e ? 'enabled')      AS has_en,
       count(*) FILTER (WHERE e->>'field_type' = k.field_type) AS ft_ok
FROM jsonb_array_elements(
       (SELECT COALESCE(sections->'kinds', '[]'::jsonb)
          FROM site_type_packs WHERE code = '%s')) e
LEFT JOIN section_kinds k ON k.kind = e->>'kind';
""" % CODE
    n, has_ft, has_en, ft_ok = psql_stdin(ROW).strip().splitlines()[0] \
        .split("|")
    nonselect = q("SELECT count(*) FROM section_kinds "
                  "WHERE field_type <> 'select';")
    ok("LU4b 快照带出的维度数=库中维度数（不半途丢行）",
       n.isdigit() and int(n) > 0 and int(n) == int(q(
           "SELECT count(*) FROM section_kinds;")),
       "快照=%s 库=%s" % (n, q("SELECT count(*) FROM section_kinds;")))
    # 反空转：现网必须有非 select 维度，否则「全带 field_type」可以靠
    # 清库达成——29/55 是本轮实测读数。
    ok("LU4c 前置在场：现网确有非 select 维度（否则下面三条可空转）",
       int(nonselect) > 0, "非 select=%s" % nonselect)
    ok("LU4d 每个维度都带 field_type/enabled",
       has_ft == n and has_en == n,
       "n=%s has_ft=%s has_en=%s" % (n, has_ft, has_en))
    ok("LU4e 带出的 field_type 与库中逐一对齐（不是补了个常量）",
       ft_ok == n, "对齐=%s/%s" % (ft_ok, n))
finally:
    psql_stdin("DELETE FROM site_type_packs WHERE code = '%s';" % CODE)
residue = q("SELECT count(*) FROM site_type_packs WHERE code = '%s';" % CODE)
ok("LU4f 探针包收尾清零（按专属 code 精确取，不用 LIKE）",
   residue == "0", "残料=%s" % residue)

# —— LU5 消费侧纪律：非法值仍拒收（宁拒收不降级） —————————
kinds_src = strip_rs_comments(read(KINDS))
guard = kinds_src.find("valid_field_type")
ok("LU5a kind_from_json 仍对非法 field_type 直接 Err",
   guard != -1 and "return Err" in kinds_src[guard:guard + 400],
   "guard 位置=%s" % guard)
apply_src = strip_rs_comments(read(APPLY))
ok("LU5b apply 落库仍写九列（不是三列回插）",
   apply_src.count("INSERT INTO section_kinds") >= 1
   and all(c in apply_src[apply_src.find("INSERT INTO section_kinds"):
                          apply_src.find("INSERT INTO section_kinds") + 320]
           for c in ["field_type", "required", "multiple", "enabled"]),
   "INSERT 段=%s" % apply_src[apply_src.find("INSERT INTO section_kinds"):
                              apply_src.find("INSERT INTO section_kinds")
                              + 180].replace("\n", " "))

# —— LU6 预置包作者纪律：省略 field_type 的维度必须带选项 ————
P = """
SELECT count(*)
FROM site_type_packs p
CROSS JOIN LATERAL jsonb_array_elements(
       COALESCE(p.sections->'kinds', '[]'::jsonb)) e
WHERE NOT (e ? 'field_type')
  AND jsonb_array_length(COALESCE(p.sections->'dict'->(e->>'kind'),
                                  '[]'::jsonb)) = 0;
"""
bare = q(P)
ok("LU6 预置包里「不带 field_type 又不带选项」的维度=0（那样会退化成无解下拉）",
   bare == "0", "违例=%s（现网 30 个缺 field_type 的维度全部带选项，"
                "默认 select 即作者意图）" % bare)

print("BASE", BASE)
summary()
