import React from "react";

/**
 * 安全 Markdown 子集渲染器（零依赖）。
 * 关键：不使用 dangerouslySetInnerHTML —— 全部经 React 元素输出，天然防 XSS。
 * 支持：``` 代码块 / # 标题 / > 引用 / - 无序 / 1. 有序 / --- 分隔线 /
 *       行内 **粗** *斜* ~~删~~ `代码` [文字](链接) @提及 / 段落软换行。
 * 链接仅允许 http(s)/mailto/站内绝对路径，其余当纯文本（防 javascript: 等）。
 */

const SAFE_URL = /^(https?:\/\/|mailto:|\/)/i;

const INLINE_RE =
  /(`[^`\n]+`)|(\*\*[^*\n]+\*\*)|(__[^_\n]+__)|(~~[^~\n]+~~)|(\*[^*\n]+\*)|(_[^_\n]+_)|(\[[^\]\n]+\]\([^)\n]+\))|(@[A-Za-z0-9_\-\u4e00-\u9fa5]{2,})/g;

function renderInline(text: string, keyBase: string): React.ReactNode[] {
  const nodes: React.ReactNode[] = [];
  let last = 0;
  let i = 0;
  for (const m of text.matchAll(INLINE_RE)) {
    const idx = m.index ?? 0;
    if (idx > last) nodes.push(text.slice(last, idx));
    const raw = m[0];
    const key = `${keyBase}-i${i++}`;
    if (raw.startsWith("`")) {
      nodes.push(
        <code
          key={key}
          className="rounded bg-[var(--surface-sunken)] px-1 py-0.5 font-mono text-[0.85em]"
        >
          {raw.slice(1, -1)}
        </code>,
      );
    } else if (raw.startsWith("**") || raw.startsWith("__")) {
      nodes.push(<strong key={key}>{raw.slice(2, -2)}</strong>);
    } else if (raw.startsWith("~~")) {
      nodes.push(<del key={key}>{raw.slice(2, -2)}</del>);
    } else if (raw.startsWith("*") || raw.startsWith("_")) {
      nodes.push(<em key={key}>{raw.slice(1, -1)}</em>);
    } else if (raw.startsWith("[")) {
      const mm = raw.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      if (mm) {
        const href = mm[2].trim();
        if (SAFE_URL.test(href)) {
          nodes.push(
            <a
              key={key}
              href={href}
              target="_blank"
              rel="noopener noreferrer nofollow"
              className="text-sky-deep hover:underline"
            >
              {mm[1]}
            </a>,
          );
        } else {
          nodes.push(mm[1]);
        }
      } else {
        nodes.push(raw);
      }
    } else if (raw.startsWith("@")) {
      nodes.push(
        <span key={key} className="font-bold text-sky-deep">
          {raw}
        </span>,
      );
    } else {
      nodes.push(raw);
    }
    last = idx + raw.length;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return nodes;
}

function heading(level: number, children: React.ReactNode, key: string) {
  const cls =
    level <= 2
      ? "mt-4 mb-2 font-display text-xl text-ink"
      : "mt-3 mb-1.5 font-display text-base text-ink";
  switch (level) {
    case 1:
      return (
        <h1 key={key} className={cls}>
          {children}
        </h1>
      );
    case 2:
      return (
        <h2 key={key} className={cls}>
          {children}
        </h2>
      );
    case 3:
      return (
        <h3 key={key} className={cls}>
          {children}
        </h3>
      );
    case 4:
      return (
        <h4 key={key} className={cls}>
          {children}
        </h4>
      );
    case 5:
      return (
        <h5 key={key} className={cls}>
          {children}
        </h5>
      );
    default:
      return (
        <h6 key={key} className={cls}>
          {children}
        </h6>
      );
  }
}

const isFence = (l: string) => /^\s*(```|~~~)/.test(l);
const isHeading = (l: string) => /^\s*#{1,6}\s+/.test(l);
const isQuote = (l: string) => /^\s*>\s?/.test(l);
const isUl = (l: string) => /^\s*[-*+]\s+/.test(l);
const isOl = (l: string) => /^\s*\d+[.)]\s+/.test(l);
const isHr = (l: string) => /^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(l);

function parseBlocks(src: string): React.ReactNode[] {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const out: React.ReactNode[] = [];
  let i = 0;
  let k = 0;

  while (i < lines.length) {
    const line = lines[i];

    const fence = line.match(/^\s*(```|~~~)(.*)$/);
    if (fence) {
      const marker = fence[1];
      const buf: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trimStart().startsWith(marker)) {
        buf.push(lines[i]);
        i++;
      }
      i++; // 跳过结束围栏
      const key = `b${k++}`;
      out.push(
        <pre
          key={key}
          className="my-3 overflow-x-auto rounded-[var(--r-sm)] bg-[var(--surface-sunken)] p-3 text-xs"
        >
          <code className="font-mono">{buf.join("\n")}</code>
        </pre>,
      );
      continue;
    }

    if (line.trim() === "") {
      i++;
      continue;
    }

    if (isHr(line)) {
      out.push(<hr key={`b${k++}`} className="my-4 border-line" />);
      i++;
      continue;
    }

    const h = line.match(/^\s*(#{1,6})\s+(.*)$/);
    if (h) {
      out.push(heading(h[1].length, renderInline(h[2], `b${k}`), `b${k++}`));
      i++;
      continue;
    }

    if (isQuote(line)) {
      const buf: string[] = [];
      while (i < lines.length && isQuote(lines[i])) {
        buf.push(lines[i].replace(/^\s*>\s?/, ""));
        i++;
      }
      out.push(
        <blockquote
          key={`b${k++}`}
          className="my-3 border-l-4 border-line pl-3 text-sub"
        >
          {renderInline(buf.join(" "), `b${k}`)}
        </blockquote>,
      );
      continue;
    }

    if (isUl(line)) {
      const items: string[] = [];
      while (i < lines.length && isUl(lines[i])) {
        items.push(lines[i].replace(/^\s*[-*+]\s+/, ""));
        i++;
      }
      const key = `b${k++}`;
      out.push(
        <ul key={key} className="my-2 list-disc space-y-0.5 pl-5 text-sm">
          {items.map((t, j) => (
            <li key={j}>{renderInline(t, `${key}-${j}`)}</li>
          ))}
        </ul>,
      );
      continue;
    }

    if (isOl(line)) {
      const items: string[] = [];
      while (i < lines.length && isOl(lines[i])) {
        items.push(lines[i].replace(/^\s*\d+[.)]\s+/, ""));
        i++;
      }
      const key = `b${k++}`;
      out.push(
        <ol key={key} className="my-2 list-decimal space-y-0.5 pl-5 text-sm">
          {items.map((t, j) => (
            <li key={j}>{renderInline(t, `${key}-${j}`)}</li>
          ))}
        </ol>,
      );
      continue;
    }

    // 段落：连续非空、非块级起始行合并；软换行保留为 <br/>
    const buf: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !isFence(lines[i]) &&
      !isHeading(lines[i]) &&
      !isQuote(lines[i]) &&
      !isUl(lines[i]) &&
      !isOl(lines[i]) &&
      !isHr(lines[i])
    ) {
      buf.push(lines[i]);
      i++;
    }
    if (buf.length) {
      const key = `b${k++}`;
      out.push(
        <p
          key={key}
          className="my-1.5 whitespace-pre-wrap break-words text-sm leading-relaxed"
        >
          {buf.map((t, j) => (
            <React.Fragment key={j}>
              {j > 0 && <br />}
              {renderInline(t, `${key}-${j}`)}
            </React.Fragment>
          ))}
        </p>,
      );
    }
  }
  return out;
}

export function MarkdownRenderer({ source }: { source: string }) {
  if (!source) return null;
  return <div className="text-ink">{parseBlocks(source)}</div>;
}
