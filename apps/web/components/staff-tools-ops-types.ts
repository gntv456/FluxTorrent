/** 运营域共享类型（staff-tools-ops 系） */
export interface SitePromo {
  id: number;
  scope: string;
  kind: string;
  category_id: number | null;
  category_name: string | null;
  starts_at: string;
  ends_at: string;
}
