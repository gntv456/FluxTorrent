/** Wiki 管理面板共享件（0359，staff-wiki.tsx 拆出——300 行门禁）：
 *  行类型 + 草稿初值 + 输入框样式常量。 */

export interface WikiPage {
  id: number;
  slug: string;
  title: string;
  body: string;
  visible: boolean;
  sort: number;
  section: string;
  updated_at: string;
}

export interface WikiRevision {
  id: number;
  page_id: number;
  title: string;
  body: string;
  sort: number;
  edited_by_name: string | null;
  created_at: string;
}

export const WIKI_FLD =
  "min-h-[38px] w-full rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-2 text-sm outline-none focus:border-sky";

export interface Draft {
  slug: string;
  title: string;
  body: string;
  visible: boolean;
  sort: number;
  /** 0495 分组：start/site/rules/api，空串=未分组 */
  section: string;
}

export const emptyDraft = (): Draft => ({
  slug: "",
  title: "",
  body: "",
  visible: true,
  sort: 100,
  section: "",
});
