/**
 * 列表形态切换（方案阶段二）：编排（不聚合 / 作品聚合）
 * × 展示（列表/卡片/海报墙）+ 每页条数（20/50/100）。
 *
 * 两个轴都写进 URL（可分享、可回退、刷新不丢），与筛选参数同一套
 * withParam。换形态/编排/条数时清掉 cursor 与 p：游标是「(排序键,id)」
 * 位置、p 是作品聚合的页码，换了数据集继续用只会让人以为「翻页丢内容」。
 * 无 hooks，保持 server component（纯链接，走整页 SSR 与筛选一致）。
 */

import type { Dict } from "@/i18n/zh-CN";
import {
  PAGE_SIZES,
  TORRENT_VIEWS,
  type TorrentView,
  type TorrentsSP,
} from "./torrents-utils";
import { TorrentsViewSelect } from "./torrents-view-select";

const GROUP_CLS =
  "flex items-center gap-0.5 rounded-full border border-line " +
  "bg-[var(--surface-card)] p-0.5";
const PILL_CLS = "rounded-full px-3 py-1 text-xs transition-colors";
const ON_CLS = "bg-[var(--sky-soft)] font-semibold text-sky-deep";
const OFF_CLS = "text-sub hover:text-ink";

export function TorrentsViewSwitch({
  dict,
  sp,
  view,
  grouped,
  pageSize,
  withParam,
}: {
  dict: Dict;
  sp: TorrentsSP;
  view: TorrentView;
  grouped: boolean;
  pageSize: number;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
}) {
  const t = dict.torrents;
  const labels: Record<TorrentView, string> = {
    table: t.viewTable,
    card: t.viewCard,
    poster: t.viewPoster,
  };
  // 换形态/编排 = 回到第一页：先把 cursor 与 p 从参数集里剔除再拼链接
  const base = Object.fromEntries(
    Object.entries(sp).filter(([k]) => k !== "cursor" && k !== "p"),
  ) as TorrentsSP;
  const href = (key: string, value: string | undefined) =>
    withParam(base, key, value);
  const groupHref = (on: boolean) =>
    href("group", on ? "1" : undefined);

  return (
    <>
      {/* 桌面/平板：两组药丸直选（编排 × 展示） */}
      <div className="hidden flex-wrap items-center justify-between gap-2 md:flex">
        <div className="flex flex-wrap items-center gap-2">
          <div
            className={GROUP_CLS}
            role="group"
            aria-label={t.groupLabel}
          >
            {[false, true].map((on) => (
              <a
                key={String(on)}
                href={groupHref(on)}
                aria-current={on === grouped ? "page" : undefined}
                className={`${PILL_CLS} ${
                  on === grouped ? ON_CLS : OFF_CLS
                }`}
              >
                {on ? dict.works.viewGroup : t.groupOff}
              </a>
            ))}
          </div>
          <div
            className={GROUP_CLS}
            role="group"
            aria-label={t.viewLabel}
          >
            {TORRENT_VIEWS.map((v) => {
              const on = v === view;
              return (
                <a
                  key={v}
                  href={href(
                    "view",
                    v === "table" ? undefined : v,
                  )}
                  aria-current={on ? "page" : undefined}
                  className={`${PILL_CLS} ${
                    on ? ON_CLS : OFF_CLS
                  }`}
                >
                  {labels[v]}
                </a>
              );
            })}
          </div>
        </div>
        <div className="flex items-center gap-1 text-xs text-sub">
          {PAGE_SIZES.map((n) => {
            const on = n === pageSize;
            return (
              <a
                key={n}
                href={href(
                  "limit",
                  n === 20 ? undefined : String(n),
                )}
                aria-current={on ? "page" : undefined}
                className={`num rounded-full px-2 py-1 transition-colors ${
                  on ? ON_CLS : "hover:text-ink"
                }`}
                title={t.perPage.replace("{n}", String(n))}
              >
                {n}
              </a>
            );
          })}
        </div>
      </div>
      {/* 手机：药丸收成三个下拉（首屏优先给内容，窄屏允许换行） */}
      <div className="flex flex-wrap items-center gap-1.5 md:hidden">
        <TorrentsViewSelect
          label={t.groupLabel}
          current={groupHref(grouped)}
          options={[false, true].map((on) => ({
            href: groupHref(on),
            label: on ? dict.works.viewGroup : t.groupOff,
          }))}
        />
        <TorrentsViewSelect
          label={t.viewLabel}
          current={href(
            "view",
            view === "table" ? undefined : view,
          )}
          options={TORRENT_VIEWS.map((v) => ({
            href: href("view", v === "table" ? undefined : v),
            label: labels[v],
          }))}
        />
        <TorrentsViewSelect
          label={t.perPage.replace("{n}", "N")}
          current={href(
            "limit",
            pageSize === 20 ? undefined : String(pageSize),
          )}
          options={PAGE_SIZES.map((n) => ({
            href: href("limit", n === 20 ? undefined : String(n)),
            label: t.perPage.replace("{n}", String(n)),
          }))}
        />
      </div>
    </>
  );
}
