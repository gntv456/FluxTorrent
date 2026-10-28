# -*- coding: utf-8 -*-
"""MA：数值设定的现网值必须落在自己卡片的 min/max 之内（P-449 防线）。

形状：设置页保存走 `validate_field`，number 型按 `settings_meta.min/max` 夹。
若某键的出厂值/后续迁移改值越出它自己声明的界，站长一保存就被自己的面板拒
（或必须先把值改回界内），而"出厂值本来就违规"这件事任何单测都看不见。
现网实测：250 张 number 卡里 **209 张有界且现网有值**，越界 0、非数字 0。

闸门价值在**防腐**：以后加迁移把某个默认值调大/调小却没同步 min/max 就红。
MA4 是谓词正控——不写库，用内联 VALUES 造一条越界行喂给同一个谓词，
抓不到就说明谓词坏了（那 MA2 的 0 是假绿）。
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

COVERED_MIN = 150          # 分母下限：低于这个数说明连接/类型筛选坏了


def one(sql):
    return psql_stdin(sql).splitlines()[0].strip()


covered = one(
    "SELECT count(*) FROM settings_meta m JOIN site_settings s "
    "ON s.name = m.name WHERE m.type = 'number' "
    "AND (m.min IS NOT NULL OR m.max IS NOT NULL) AND s.value <> '';")
ok("MA1 反空转：有界且现网有值的 number 键须 > %d（实测=%s，"
   "分母太小＝这条判据在空转）" % (COVERED_MIN, covered),
   int(covered) > COVERED_MIN, "实测=%s" % covered)

over = one(
    "SELECT count(*) FROM settings_meta m JOIN site_settings s "
    "ON s.name = m.name WHERE m.type = 'number' AND s.value <> '' "
    "AND ((m.min IS NOT NULL AND s.value::numeric < m.min) "
    "  OR (m.max IS NOT NULL AND s.value::numeric > m.max));")
BAD_SQL = (
    "SELECT m.name || '=' || s.value || ' bound[' "
    "|| COALESCE(m.min::text, '-') || ',' || COALESCE(m.max::text, '-') "
    "|| ']' FROM settings_meta m JOIN site_settings s ON s.name = m.name "
    "WHERE m.type = 'number' AND s.value <> '' "
    "AND ((m.min IS NOT NULL AND s.value::numeric < m.min) "
    "OR (m.max IS NOT NULL AND s.value::numeric > m.max)) ORDER BY 1 LIMIT 8;"
)
bad_list = psql_stdin(BAD_SQL) if over != "0" else ""
ok("MA2 现网值越出自己卡片 min/max 的键必须为 0（越界＝站长保存被自己拒）",
   over == "0", "违例=%s 明细=%s" % (over, bad_list.replace("\n", " | ")))

nonnum = one(
    "SELECT count(*) FROM settings_meta m JOIN site_settings s "
    "ON s.name = m.name WHERE m.type = 'number' AND s.value <> '' "
    "AND s.value !~ '^-?[0-9]+(\\.[0-9]+)?$';")
ok("MA3 非数字值住在 number 卡里的键必须为 0（validate_field 直接拒，"
   "界面读出来是空/0）", nonnum == "0", "违例=%s" % nonnum)

# 谓词正控：内联造一行越界值，同一个比较式必须数到它
ctl = one(
    "WITH t(v, lo, hi) AS (VALUES ('10', 5, 20), ('99', 5, 20), ('7', 1, 3)) "
    "SELECT count(*) FROM t WHERE v::numeric < lo OR v::numeric > hi;")
ok("MA4 正控：同一把谓词对人造越界行须数到 2（数不到＝MA2 是假绿）",
   ctl == "2", "实得=%s" % ctl)

summary()
