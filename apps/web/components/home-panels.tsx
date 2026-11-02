"use client";

import Link from "next/link";

/** 首页两个低频板块（从 home-sections.tsx 按域拆出，给它腾行数）：
 *  幸运大转盘流水 + 免责条款/友情链接。
 *
 *  手机端转盘压成一行（点开看流水，见 home-lucky-draw 的 is-compact 形态），
 *  免责与友链压进页脚链接组——它们在桌面各占一整块，在手机上不值一屏。 */

export function LuckyDrawPanel({
  items,
  title,
  goDraw,
  got,
  sparkLabel,
  empty,
}: {
  items: { user: string; kind: string; amount: number }[];
  title: string;
  goDraw: string;
  got: string;
  sparkLabel: string;
  empty: string;
}) {
  return (
    <aside className="brand-panel home-lucky-draw" aria-label={title}>
      <header className="brand-panel__head">
        <h2>
          <span aria-hidden="true">🎰</span> {title}
        </h2>
        <Link href="/games">{goDraw}</Link>
      </header>
      <ul className="home-lucky-draw__list">
        {items.map((l, i) => (
          <li key={i}>
            <b className="rainbow">{l.user}</b> {got} {sparkLabel} {l.amount}
          </li>
        ))}
        {items.length === 0 && <li className="text-sub">{empty}</li>}
      </ul>
    </aside>
  );
}

/** 免责条款 + 友情链接（手机端由 .home-native-modules 的窄屏样式压成链接组） */
export function LinksPanel({
  links,
  disclaimerTitle,
  disclaimerBody,
  linksTitle,
  applyLink,
}: {
  links: { name: string; url: string; title: string | null }[];
  disclaimerTitle: string;
  disclaimerBody: string;
  linksTitle: string;
  applyLink: string;
}) {
  return (
    <div className="home-native-modules">
      <h2>{disclaimerTitle}</h2>
      <p className="home-native-modules__text">{disclaimerBody}</p>
      <h2>
        {linksTitle}
        <small>
          {" "}
          - [<Link href="/links/apply">{applyLink}</Link>]
        </small>
      </h2>
      <p className="home-native-modules__text">
        {links.map((l) => (
          <a
            key={l.url}
            href={l.url}
            title={l.title ?? l.name}
            target="_blank"
            rel="noreferrer"
          >
            {l.name}
          </a>
        ))}
      </p>
    </div>
  );
}
