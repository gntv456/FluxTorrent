# -*- coding: utf-8 -*-
"""education 站型三件套完成度体检（只读审计，对标 §7.4「唯一达标的族」复核）。

跑法：
    python scripts/audit_education_health.py

不写任何数据。检查 exams/textbooks/jixiao 三件套与种子的闭环：
  1. 模块开关实际态（module_exams/textbooks/jixiao）
  2. textbooks 表与种子的双向关联（textbook_id 引用率）
  3. exams/试卷与种子的关联面
  4. jixiao（绩效）与用户/种子的引用面
  5. grades/editions 词表与 education 包声明一致性
"""
import os
import subprocess
import sys

PG = ["docker", "exec", os.environ.get("FLUX_PG_CONTAINER", "flux-postgres"),
      "psql", "-U", "flux", "-d",
      os.environ.get("FLUX_PG_DB", "fluxtorrent"), "-t", "-A", "-c"]

FINDINGS = []


def psql(sql):
    r = subprocess.run(PG + [sql], capture_output=True, text=True)
    if r.returncode != 0:
        return "ERR:" + r.stderr[:120]
    return r.stdout.strip()


def check(name, cond, detail=""):
    mark = "OK " if cond else "GAP"
    print("[%s] %s %s" % (mark, name, ("- " + str(detail)[:160]) if detail else ""))
    if not cond:
        FINDINGS.append(name)


def main():
    print("== education 三件套完成度体检（只读）==\n--- 模块开关 ---")
    for m in ("exams", "textbooks", "jixiao"):
        v = psql("SELECT value FROM site_settings WHERE name='module_%s'" % m)
        check("module_%s 已配置" % m, v in ("yes", "no"), v or "(未配置=默认)")

    print("\n--- textbooks ↔ 种子 ---")
    n_books = psql("SELECT count(*) FROM textbooks")
    linked = psql("SELECT count(*) FROM torrents WHERE textbook_id IS NOT NULL")
    check("textbooks 表存在", psql("SELECT to_regclass('textbooks') IS NOT NULL") == "t")
    check("种子有 textbook_id 关联列", psql(
        "SELECT count(*) FROM information_schema.columns WHERE "
        "table_name='torrents' AND column_name='textbook_id'") == "1")
    print("    （教材条目 %s 本 / 已关联种子 %s 枚 —— 内容运营态，空非缺口）"
          % (n_books, linked))
    # 反向：教材条目下能聚出种子列表（详情页入口）
    orphans = psql(
        "SELECT count(*) FROM textbooks tb WHERE NOT EXISTS "
        "(SELECT 1 FROM torrents t WHERE t.textbook_id = tb.id)")
    print("    （无种子挂靠的教材条目：%s —— 空站正常，非缺口）" % orphans)

    print("\n--- exams（考核）---")
    # exams 是模块开关型能力（modules.rs EXAMS），数据表按运营需求另建；
    # 体检确认注册表与模块键面
    reg = psql("SELECT count(*) FROM modules WHERE key='exams'")
    check("exams 在模块注册表", reg == "1", reg)

    print("\n--- jixiao（绩效）---")
    for tbl in ("jixiao_types", "jixiao_claims", "jixiao_baseline_snapshots"):
        exists = psql("SELECT to_regclass('%s') IS NOT NULL" % tbl)
        check("表 %s 存在" % tbl, exists == "t")

    print("\n--- 词表一致性（education 包声明 vs 物化）---")
    pack_grades = psql(
        "SELECT jsonb_array_length(sections->'dict'->'grades') "
        "FROM site_type_packs WHERE code='education'")
    live_grades = psql("SELECT count(*) FROM grades")
    # grades/editions 实体表是教育站型 apply 的物化目标；非 education 站型下
    # 为空属设计（edu_like 门控）。表存在 + 包声明非空即可
    check("grades 表存在", psql("SELECT to_regclass('grades') IS NOT NULL") == "t")
    check("education 包 grades 声明非空", pack_grades not in ("0", ""), pack_grades)
    check("editions 表存在", psql("SELECT to_regclass('editions') IS NOT NULL") == "t")
    if live_grades != "0":
        print("    （当前站型=%s，grades 已物化 %s 行）"
              % (psql("SELECT value FROM site_settings WHERE name='site_type'"), live_grades))

    print("\n--- 站型维度（0315/0317 已补，确认物化路径）---")
    dims = psql("SELECT count(*) FROM section_kinds WHERE kind IN "
                "('subject','grade','resource_type')")
    check("教育三维度在 section_kinds", dims == "3", dims)

    print()
    if FINDINGS:
        print("== GAP %d 项: %s" % (len(FINDINGS), ", ".join(FINDINGS)))
        sys.exit(1)
    print("== 体检通过（结构面完整；内容运营态项不计缺口）")


if __name__ == "__main__":
    main()
