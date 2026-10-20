"use client";

import { PANEL_CENTER } from "@/lib/ui-classes";

/**
 * 站点设定分组导航 + 卡片表单（从 app/(main)/admin/settings/settings-client.tsx
 * 按域拆出）：SettingsGroupNav 左侧分区导航、SettingsSearchResults 搜索命中列表、
 * SettingsGroupCards 当前分区卡片分组渲染。字段值/错误通过 renderField 回调注入。
 */

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { enumOptions } from "@/components/setting-field-controls";
import { AutoReviewStatBar } from "./auto-review-stat-bar";
import type {
  SettingCard,
  SettingFieldMeta,
  SettingGroup,
  SettingsSchema,
} from "@/components/setting-field";

/** 左导航（≤768px 变横向滚动 Chip 条）：分区切换 + 深链 ?group= 写入 */
export function SettingsGroupNav({
  schema,
  active,
  searching,
  q,
  onQ,
  onActive,
}: {
  schema: SettingsSchema | null;
  active: string;
  searching: boolean;
  q: string;
  onQ: (v: string) => void;
  onActive: (key: string) => void;
}) {
  const { dict, currency } = useI18n();
  const s = dict.settingsAdmin;
  const groups = schema?.groups ?? [];
  // md+ 吸顶偏移 168 = 保存条(top-84, 高 70, 实测四档宽度均单行) + 14 间隙；
  // 100 会与保存条重叠 54px（双吸顶同列，保存条 z-10 盖住分组导航顶部）。
  const NAV_CLS =
    "flex gap-2 overflow-x-auto border border-line bg-[var(--surface-card)]" +
    " p-2 md:w-56 md:flex-none md:flex-col md:overflow-visible" +
    " md:rounded-[var(--r-md)] md:shadow-[var(--shadow-card)] md:sticky" +
    " md:top-[168px] md:max-h-[calc(100vh-192px)] md:self-start" +
    " md:overflow-y-auto";
  return (
    <nav className={NAV_CLS}>
      {/* 栏头（对齐样图 § 站点设置左栏：「设定分区 · N 分区」） */}
      <p className="mb-1 hidden px-1 text-xs font-bold text-ink md:block">
        {s.navTitle} · {groups.length} 分区
      </p>
      <div className="hidden md:block">
        <input
          value={q}
          onChange={(e) => onQ(e.target.value)}
          placeholder={s.search}
          className="min-h-[44px] w-full rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 text-xs"
        />
      </div>
      {groups.map((g) => (
        <button
          key={g.key}
          type="button"
          onClick={() => {
            onActive(g.key);
            if (typeof window !== "undefined") {
              const u = new URL(window.location.href);
              u.searchParams.set("group", g.key);
              window.history.replaceState(null, "", u.toString());
            }
          }}
          aria-current={!searching && active === g.key}
          className={`flex min-h-[44px] flex-none items-center gap-2 rounded-[var(--r-sm)] px-3 text-xs font-bold md:w-full ${
            !searching && active === g.key
              ? "bg-sky-deep text-white"
              : "border border-line bg-[var(--surface-card)] text-sub md:border-0 md:text-ink"
          }`}
        >
          <span className="whitespace-nowrap">
            {(dict.admin.settingGroups[g.key] ?? g.label).replace(
              "{magic}",
              currency,
            )}
          </span>
          <span className="ml-auto text-[10px] font-normal">{g.count}</span>
        </button>
      ))}
    </nav>
  );
}

/** 搜索命中项（从 settings-client 拆出，保 300 行门禁）：标注所属分区 */
export interface SettingsHit {
  group: string;
  groupLabel: string;
  field: SettingFieldMeta;
}

/** 全字段搜索：label / label_en / name / hint / 分区名 命中即收 */
export function searchSettings(
  groups: SettingGroup[],
  kw: string,
): SettingsHit[] {
  if (!kw) return [];
  const out: SettingsHit[] = [];
  const hit = (f: SettingFieldMeta, groupLabel: string) =>
    f.label.toLowerCase().includes(kw) ||
    (f.label_en ?? "").toLowerCase().includes(kw) ||
    f.name.toLowerCase().includes(kw) ||
    (f.hint ?? "").toLowerCase().includes(kw) ||
    groupLabel.toLowerCase().includes(kw);
  for (const g of groups) {
    for (const c of g.cards) {
      for (const f of c.fields) {
        if (hit(f, g.label)) {
          out.push({ group: g.key, groupLabel: g.label, field: f });
        }
      }
    }
  }
  return out;
}

/** 搜索命中列表：每条标注所属分区，字段渲染走 renderField */
export function SettingsSearchResults({
  matches,
  renderField,
}: {
  matches: SettingsHit[];
  renderField: (f: SettingFieldMeta) => React.ReactNode;
}) {
  const { dict } = useI18n();
  const s = dict.settingsAdmin;
  return (
    <section className="flex flex-col gap-3">
      <p className="text-xs text-sub">
        {fmt(s.searchHit, { n: matches.length })}
      </p>
      {matches.length === 0 && <p className={PANEL_CENTER}>{s.searchEmpty}</p>}
      {matches.map((m) => (
        <div key={`${m.group}-${m.field.name}`}>
          <p className="mb-1 text-[11px] font-bold text-sub">{m.groupLabel}</p>
          {renderField(m.field)}
        </div>
      ))}
    </section>
  );
}

/** 当前分区卡片分组：开关类字段（yesno/短 enum）紧凑多列；其余纵向大卡片。
 *  顶部先渲染分区信息条（对齐样图 § 站点设置：字段来源 + 保存端点）。 */
export function SettingsGroupCards({
  group,
  renderField,
}: {
  group: SettingGroup;
  renderField: (f: SettingFieldMeta) => React.ReactNode;
}) {
  const { dict, currency } = useI18n();
  const s = dict.settingsAdmin;
  const groupLabel = (
    dict.admin.settingGroups[group.key] ?? group.label
  ).replace("{magic}", currency);
  return (
    <section className="flex flex-col gap-3">
      <p
        className="rounded-[var(--r-md)] border border-line
          bg-[var(--surface-card)] px-3 py-2 text-xs text-sub"
      >
        {fmt(s.groupBar, { group: groupLabel, n: group.count })}
      </p>
      {/* 自动审种（0400 A11）：分区顶部实时示例条——改阈值前先看效果 */}
      {group.key === "auto_review" && <AutoReviewStatBar />}
      {group.cards.map((c) => {
        const isSwitch = (f: SettingFieldMeta) =>
          f.type === "yesno" ||
          (f.type === "enum" &&
            enumOptions(f.options).length > 0 &&
            enumOptions(f.options).length <= 4);
        const switches = c.fields.filter(isSwitch);
        const rest = c.fields.filter((f) => !isSwitch(f));
        return (
          <div
            key={c.key}
            className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3 shadow-[var(--shadow-card)]"
          >
            <h2 className="mb-2 flex items-baseline gap-2 text-sm font-bold text-ink">
              {(dict.admin.settingGroups[c.key] ?? c.key).replace(
                "{magic}",
                currency,
              )}
              <span className="text-[11px] font-normal text-sub">
                {c.fields.length}
              </span>
            </h2>
            {switches.length > 0 && (
              <div className="mb-2 grid grid-cols-1 gap-1.5 sm:grid-cols-2 lg:grid-cols-3">
                {switches.map((f) => renderField(f))}
              </div>
            )}
            {rest.length > 0 && (
              <div className="flex flex-col gap-2">
                {rest.map((f) => renderField(f))}
              </div>
            )}
          </div>
        );
      })}
      {group.cards.length === 0 && (
        <p className={PANEL_CENTER}>{s.searchEmpty}</p>
      )}
    </section>
  );
}
