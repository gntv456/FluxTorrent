/**
 * 资源页「作品聚合」视图（样图 design/资源页-影视资料库样图.html 的 group 模式）：
 * 按作品归堆——每部作品一张卡（封面 + 片名 + 年份 + 分类 + 标签 + 简介 +
 * 统计），卡下直接挂该作品的种子行，每部最多 8 条，多余的走「查看全部」。
 *
 * 取数独立端点 `/api/v1/works`：作品聚合要按归一化标题全表聚合，塞进
 * `/torrents` 会拖慢普通列表（后端注释见 works_sql.rs）。
 *
 * 与列表页其余视图共用 URL 参数：`p`（页码）、`search`、`category_id`。
 */

import Link from "next/link";
import { api } from "@/lib/api-client";
import { ErrorPanel } from "@/components/ui/error-panel";
import { normalizeError } from "@/lib/render-error";
import { WorkCard } from "@/components/work-card";
import type { WorkPage } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";
import type { Locale } from "@/i18n/config";
import type { TorrentsSP } from "./torrents-utils";

/** 每页作品数（样图是作品卡 + 卡内种子，粒度比种子列表粗，12 已够一屏） */
const WORKS_PER_PAGE = 12;

export async function TorrentWorks({
  dict,
  locale,
  sp,
  cats,
  withParam,
}: {
  dict: Dict;
  locale: Locale;
  sp: TorrentsSP;
  cats: { id: number; label: string }[];
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
}) {
  const pageNo = Math.max(1, Number(sp.p ?? "1") || 1);
  const qs = new URLSearchParams({
    limit: String(WORKS_PER_PAGE),
    page: String(pageNo),
  });
  if (sp.search) qs.set("q", sp.search);
  // 作品层只吃单分类（多选分类在作品粒度下语义含糊，取第一个）
  const cat = (sp.category_id ?? "").split(",").filter(Boolean)[0];
  if (cat) qs.set("category_id", cat);
  if (sp.sort === "seeders" || sp.sort === "size") qs.set("sort", sp.sort);

  let data: WorkPage | null = null;
  let err: unknown = null;
  try {
    data = await api.get<WorkPage>(`/api/v1/works?${qs.toString()}`);
  } catch (e) {
    err = e;
  }

  if (err !== null) {
    return (
      <ErrorPanel
        error={normalizeError(err)}
        desc={dict.torrents.loadFailedHint}
        retryHref={withParam(sp, "p", String(pageNo))}
      />
    );
  }
  if (!data || data.items.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-sub">
        {dict.works.empty}
      </p>
    );
  }

  const catName = (id: number) =>
    cats.find((c) => c.id === id)?.label ?? "";
  const totalPages = Math.max(1, Math.ceil(data.total / WORKS_PER_PAGE));

  return (
    <div className="flex flex-col gap-2">
      <p className="wk-lead">
        {dict.works.countWorks.replace("{n}", String(data.total))}
      </p>
      {data.items.map((w) => (
        <WorkCard
          key={w.key}
          work={w}
          dict={dict}
          locale={locale}
          catName={catName(w.category_id)}
        />
      ))}
      {totalPages > 1 && (
        <nav className="flex items-center justify-center gap-3 py-1">
          {pageNo > 1 && (
            <Link
              href={withParam(sp, "p", String(pageNo - 1))}
              className="wk-pg"
            >
              {dict.common.prevPage}
            </Link>
          )}
          <span className="text-sm text-sub">
            {dict.common.pageX.replace("{n}", String(pageNo))}
            {" / "}
            {totalPages}
          </span>
          {pageNo < totalPages && (
            <Link
              href={withParam(sp, "p", String(pageNo + 1))}
              className="wk-pg"
            >
              {dict.common.nextPage}
            </Link>
          )}
        </nav>
      )}
    </div>
  );
}
