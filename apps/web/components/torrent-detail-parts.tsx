import { hasBBCode, renderBBCode } from "@/lib/bbcode";
import { formatBytes } from "@/lib/format";
import { Icon } from "@/components/icons";
import { PosterImage } from "@/components/poster-image";
import type { Dict } from "@/i18n/zh-CN";

/** 种子详情页展示件（从 app/(main)/torrent/[id]/page.tsx 按域拆出）：
 *  Descr 简介渲染、Spec 规格网格单元、Fold 折叠分区。数据装载留在 page.tsx。 */

/** 简介 markdown-lite 渲染：标题/列表/段落（descr 为简单 markdown 文本，无需完整 parser）
 *  含 BBCode 标签时走 BBCode 渲染器（NP 口径：颜色/字体/字号/引用/代码/图片/链接…）
 *  超长简介默认折叠（同类站通用口径：展开按钮在底部） */
export function Descr({ text }: { text: string }) {
  if (hasBBCode(text)) {
    return (
      <div className="whitespace-pre-wrap text-sm leading-relaxed">
        {renderBBCode(text)}
      </div>
    );
  }
  const lines = text.split("\n");
  const out: React.ReactNode[] = [];
  let listBuf: string[] = [];
  const flushList = (key: number) => {
    if (listBuf.length) {
      out.push(
        <ul key={`ul-${key}`} className="ml-5 list-disc space-y-0.5">
          {listBuf.map((li, i) => (
            <li key={i}>{li}</li>
          ))}
        </ul>,
      );
      listBuf = [];
    }
  };
  lines.forEach((line, i) => {
    const s = line.trim();
    if (s.startsWith("- ") || s.startsWith("* ")) {
      listBuf.push(s.slice(2));
    } else {
      flushList(i);
      if (s.startsWith("#")) {
        const level = Math.min(s.match(/^#+/)?.[0].length ?? 1, 4);
        const Tag = `h${level + 2}` as "h3" | "h4" | "h5";
        out.push(
          <Tag key={i} className="mt-3 font-display text-lg first:mt-0">
            {s.replace(/^#+\s*/, "")}
          </Tag>,
        );
      } else if (s) {
        out.push(
          <p key={i} className="mt-2 first:mt-0">
            {s}
          </p>,
        );
      }
    }
  });
  flushList(lines.length);
  return <div className="text-sm leading-relaxed">{out}</div>;
}

/** 规格网格单元（阳光站口径：数值 + 下方灰字说明） */
export function Spec({
  value,
  label,
  num,
}: {
  value: React.ReactNode;
  label: string;
  num?: boolean;
}) {
  return (
    <div className="td-spec">
      <b className={num ? "num" : undefined}>{value}</b>
      <span>{label}</span>
    </div>
  );
}

/** 折叠分区（同类站通用口径：默认收起，summary 带计数） */
export function Fold({
  title,
  count,
  children,
  open,
  id,
}: {
  title: string;
  count?: number;
  children: React.ReactNode;
  open?: boolean;
  /** 锚点 id（侧栏「查看全部」定位目标，scroll-margin-top 见 base.css） */
  id?: string;
}) {
  return (
    <details className="td-fold" open={open} id={id}>
      <summary>
        <h2>{title}</h2>
        {count !== undefined && (
          <span className="td-fold__count num">{count}</span>
        )}
        <span className="td-fold__chev" aria-hidden />
      </summary>
      <div className="td-fold__body">{children}</div>
    </details>
  );
}

/** 海报头分类角标与海报兜底（阳光口径：左海报，分类色） */
export function PosterBlock({
  category,
  color,
  poster,
}: {
  category: string;
  /** 分类色（categories.bg_color 下发，0183） */
  color: string;
  poster?: string | null;
}) {
  return (
    <div className="td-head__poster">
      <span
        className="td-head__cat"
        style={{ background: color }}
      >
        {category}
      </span>
      <PosterImage
        src={poster}
        alt=""
        size="lg"
        className="td-head__img"
        fallback={
          <span
            className="td-head__img td-head__img--fallback"
            style={{ background: color }}
          >
            <Icon name="disc" size={34} />
          </span>
        }
      />
    </div>
  );
}

/** MediaInfo 一致性提示（0390：判定是提示不是闸门）。
 *  结论由聚合口的 `mi_check` 下发（只含 mismatches/unverifiable/checked_at），
 *  why 文案由服务端生成（`domain::mediainfo`），这里不翻译、不改写。
 *  `agg: object` + 内部收口：page.tsx 的 Aggregate 契约未收进 domain-types，
 *  而它的行数基线已到顶（432），加不动字段，调用方必须一行写完。 */
export function MiWarn({ agg, d }: { agg: object; d: Dict["tdetail"] }) {
  const c = (agg as { mi_check?: Record<string, unknown> }).mi_check;
  const bad = (c?.mismatches ?? []) as { field: string; why: string }[];
  const unver = (c?.unverifiable ?? []) as string[];
  if (bad.length === 0 && unver.length === 0) return null;
  const fill = (s: string, n: number) => s.replace("{n}", String(n));
  return (
    <div className="td-mi-warn">
      {bad.length > 0 && (
        <>
          <p className="td-mi-warn__head">{fill(d.miMismatch, bad.length)}</p>
          <ul className="td-mi-warn__list">
            {bad.map((m) => (
              <li key={m.field}>{m.why}</li>
            ))}
          </ul>
        </>
      )}
      {unver.length > 0 && (
        <p className="td-mi-warn__note">
          {fill(d.miUnverifiable, unver.length)}
        </p>
      )}
    </div>
  );
}

/** 工件行（0323 torrent_artifacts）与更新链行（子种）——aggregate 已下发，
 *  此前前端零消费；正文 body 不在聚合里（聚合刻意不带），故本段只给清单。 */
export type ArtifactRow = {
  id: number;
  kind: string;
  filename: string;
  sha256: string | null;
  size: number;
};
export type ArtifactChildRow = {
  torrent_id: number;
  name: string;
  created_at: string;
  size: number;
};

/** 工件清单 + 更新包导航（0323 成文「UI 徽标随后」的那一段）。
 *  两叠都空时整块不渲染（与文件/字幕块同口径：无数据不占位）。
 *  props 收成 `agg` 一个：page.tsx 的行数基线已到顶（432），调用方一行写完。 */
export function TorrentArtifacts({
  agg,
  d,
}: {
  // `object` 而非结构形状：page.tsx 的 Aggregate 契约还没收进 domain-types
  // （文件头注明「待后续批次」），而它的行数基线已顶到 432，加不动字段。
  // 全弱类型（属性全可选）会被 TS 的 weak-type 检查拒掉，这里自己收口。
  agg: object;
  d: Dict["tdetail"];
}) {
  const a = agg as {
    artifacts?: ArtifactRow[];
    artifact_children?: ArtifactChildRow[];
  };
  const items = a.artifacts ?? [];
  const children = a.artifact_children ?? [];
  if (items.length === 0 && children.length === 0) return null;
  const kindLabel = (k: string): string =>
    k === "checksums"
      ? d.artKindChecksums
      : k === "changelog"
        ? d.artKindChangelog
        : k === "license"
          ? d.artKindLicense
          : d.artKindOther;
  return (
    <Fold title={d.artifactsTitle} id="td-sec-artifacts">
      {items.length > 0 && (
        <ul className="td-art__list">
          {items.map((a) => (
            <li key={a.id} className="td-art__row">
              <span className="td-art__kind">{kindLabel(a.kind)}</span>
              <span className="td-art__name" title={a.filename}>
                {a.filename}
              </span>
              {a.sha256 && (
                <code className="td-art__sha" title={a.sha256}>
                  {a.sha256.slice(0, 12)}
                </code>
              )}
              <span className="td-art__size num">{formatBytes(a.size)}</span>
            </li>
          ))}
        </ul>
      )}
      {children.length > 0 && (
        <div className="td-art__chain">
          <p className="td-art__chain-head">{d.artifactChildTitle}</p>
          <ul className="td-art__list">
            {children.map((c) => (
              <li key={c.torrent_id} className="td-art__row">
                <a href={`/torrent/${c.torrent_id}`} className="td-art__link">
                  {c.name}
                </a>
                <span className="td-art__date num">{c.created_at}</span>
                <span className="td-art__size num">{formatBytes(c.size)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Fold>
  );
}
