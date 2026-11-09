/** 教育资源四级导航（0473）：主菜单「教育资源」→ 学段 → 年级 → 科目。
 *
 *  数据口径：
 *  - 学段 = categories 子分类（101 幼儿园 / 102 小学 / 103 初中 / 105 高中 /
 *    107 大学——107 由迁站回填脚本新建）；教育影音 106 与职高 104 不挂导航。
 *  - 年级 = grades 表 id（0-12），筛选走 /torrents?grade_id=（ListQuery 原生）。
 *  - 科目 = section_dict kind='subject'（0315 迁移固定 id），筛选走
 *    /torrents?sec_subject=<id>。id 与 grades id 是迁移种子常量，硬编码在此；
 *    显示名走 i18n（labels 由调用方传 dict.nav 的 edu* 键）。
 *
 *  幼儿园不分年级（grade 0 一档）；大学暂无年级层（直接科目）。 */

import type { NavItem } from "./nav-menu";

export type EduStage = {
  /** categories 子分类 id */
  cat: number;
  /** i18n 键（nav.eduSt*） */
  labelKey: string;
  /** 年级列表（grades 表 id）；空 = 无年级层（幼儿园/大学直接科目） */
  grades: number[];
};

export const EDU_SUBJECT_KEYS: { id: number; labelKey: string }[] = [
  { id: 100, labelKey: "eduSubjMath" },
  { id: 102, labelKey: "eduSubjChinese" },
  { id: 98, labelKey: "eduSubjEnglish" },
  { id: 97, labelKey: "eduSubjPhysics" },
  { id: 99, labelKey: "eduSubjChemistry" },
  { id: 105, labelKey: "eduSubjBiology" },
  { id: 103, labelKey: "eduSubjPolitics" },
  { id: 104, labelKey: "eduSubjHistory" },
  { id: 96, labelKey: "eduSubjGeography" },
];

export const EDU_STAGES: EduStage[] = [
  { cat: 101, labelKey: "eduStNursery", grades: [0] },
  { cat: 102, labelKey: "eduStPrimary", grades: [1, 2, 3, 4, 5, 6] },
  { cat: 103, labelKey: "eduStJunior", grades: [7, 8, 9] },
  { cat: 105, labelKey: "eduStSenior", grades: [10, 11, 12] },
  { cat: 107, labelKey: "eduStUni", grades: [] },
];

const GRADE_KEY: Record<number, string> = {
  0: "eduGr0",
  1: "eduGr1",
  2: "eduGr2",
  3: "eduGr3",
  4: "eduGr4",
  5: "eduGr5",
  6: "eduGr6",
  7: "eduGr7",
  8: "eduGr8",
  9: "eduGr9",
  10: "eduGr10",
  11: "eduGr11",
  12: "eduGr12",
};

type Labels = Record<string, string>;

/** 教育资源下拉（sub 结构同资源库：学段组 → 年级叶子 → 科目 children） */
export function eduSub(labels: Labels): NonNullable<NavItem["sub"]> {
  const L = (k: string) => labels[k] ?? k;
  return EDU_STAGES.map((st) => {
    const base = `/torrents?category_id=${st.cat}`;
    const items: NavItem[] = [
      { href: base, label: L("eduAll") + L(st.labelKey) },
    ];
    for (const g of st.grades) {
      items.push({
        href: `${base}&grade_id=${g}`,
        label: L(GRADE_KEY[g]),
        children: EDU_SUBJECT_KEYS.map((s) => ({
          href: `${base}&grade_id=${g}&sec_subject=${s.id}`,
          label: L(s.labelKey),
        })),
      });
    }
    if (st.grades.length === 0) {
      for (const s of EDU_SUBJECT_KEYS) {
        items.push({
          href: `${base}&sec_subject=${s.id}`,
          label: L(s.labelKey),
        });
      }
    }
    return { label: L(st.labelKey), href: base, items };
  });
}

export const EDU_RES_CAT = 13;

/** 教育资源一级项（categories 含 13 时由 buildNav 挂出） */
export function eduPrimary(label: string, labels: Labels): NavItem {
  return {
    href: "/torrents?category_id=13",
    label,
    edu: true,
    sub: eduSub(labels),
  };
}

/** 站点是否具备教育板块（顶级分类里有教育资源 13） */
export function hasEduCat(cats: { id: number; name: string }[]): boolean {
  return cats.some((c) => c.id === EDU_RES_CAT);
}
