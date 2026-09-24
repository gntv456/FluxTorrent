import { hasBBCode, renderBBCode } from "@/lib/bbcode";
import { Icon } from "@/components/icons";

/** 种子详情页展示件（从 app/(main)/torrent/[id]/page.tsx 按域拆出）：
 *  Descr 简介渲染、Spec 规格网格单元、Fold 折叠分区。数据装载留在 page.tsx。 */

/** 简介 markdown-lite 渲染：标题/列表/段落（descr 为简单 markdown 文本，无需完整 parser）
 *  含 BBCode 标签时走 BBCode 渲染器（NP 口径：颜色/字体/字号/引用/代码/图片/链接…）
 *  超长简介默认折叠（馒头/阳光口径：展开按钮在底部） */
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

/** 折叠分区（馒头口径：默认收起，summary 带计数） */
export function Fold({
  title,
  count,
  children,
  open,
}: {
  title: string;
  count?: number;
  children: React.ReactNode;
  open?: boolean;
}) {
  return (
    <details className="td-fold" open={open}>
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
      {poster ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img src={poster} alt="" className="td-head__img" />
      ) : (
        <span
          className="td-head__img td-head__img--fallback"
          style={{ background: color }}
        >
          <Icon name="disc" size={34} />
        </span>
      )}
    </div>
  );
}
