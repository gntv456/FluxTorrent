"use client";

/**
 * 甜梦奇境·样图组件三件：stat-row（三格数据宝石）/ 历史行列表 /
 * cta-row（单抽 + 十连）。供九宫格 / 刮刮乐 / 大转盘等专注页复用，
 * 文案由页面用 i18n 合成后传入，组件保持哑件。
 */

export function SwStatRow({
  items,
}: {
  items: { lb: string; vl: string; tone?: "gold" | "green" }[];
}) {
  return (
    <div className="sw-stat-row">
      {items.map((it) => (
        <div key={it.lb} className="sw-stat">
          <div className="sw-stat-lb">{it.lb}</div>
          <div className={`sw-stat-vl num${it.tone ? ` ${it.tone}` : ""}`}>
            {it.vl}
          </div>
        </div>
      ))}
    </div>
  );
}

export interface SwHistRow {
  t: string;
  txt: string;
  net: number;
}

export function SwHist({ rows, empty }: { rows: SwHistRow[]; empty: string }) {
  if (rows.length === 0) {
    return <p className="sw-hist-empty">{empty}</p>;
  }
  return (
    <div className="sw-hist">
      {rows.map((r, i) => (
        <div key={i} className="sw-hrow">
          <span className="sw-htime num">{r.t}</span>
          <span className="sw-htxt">{r.txt}</span>
          <span className={`sw-hamt num${r.net >= 0 ? " win" : " lose"}`}>
            {r.net >= 0 ? `+${r.net}` : r.net}
          </span>
        </div>
      ))}
    </div>
  );
}

export function SwCtaRow({
  primaryLabel,
  goldLabel,
  onPrimary,
  onGold,
  primaryDisabled,
  goldDisabled,
  goldBusy,
  primaryGold,
}: {
  primaryLabel: string;
  /** 不传则只渲染主按钮（如转盘的中心 GO 已是主行动） */
  goldLabel?: string;
  onPrimary: () => void;
  onGold?: () => void;
  primaryDisabled?: boolean;
  goldDisabled?: boolean;
  goldBusy?: boolean;
  /** 主钮走香槟金（样图的大金开始键，转盘用） */
  primaryGold?: boolean;
}) {
  return (
    <div className="sw-cta-row">
      <button
        type="button"
        className={`sw-cta ${primaryGold ? "cta-gold" : "cta-primary"}`}
        onClick={onPrimary}
        disabled={primaryDisabled}
      >
        {primaryLabel}
      </button>
      {goldLabel && (
        <button
          type="button"
          className="sw-cta cta-gold"
          onClick={onGold}
          disabled={goldDisabled}
        >
          {goldBusy ? "…" : goldLabel}
        </button>
      )}
    </div>
  );
}

/** 十连结果浮层：十张结果 + 合计净收 */
export function SwTenModal({
  open,
  title,
  rows,
  totalLabel,
  onClose,
}: {
  open: boolean;
  title: string;
  rows: { label: string; net: number }[];
  totalLabel: string;
  onClose: () => void;
}) {
  if (!open) return null;
  const total = rows.reduce((s, r) => s + r.net, 0);
  return (
    <div className="sw-ten-overlay" role="presentation">
      <div className="sw-ten" role="dialog" aria-label={title}>
        <div className="sw-ten-hd">
          <h4>{title}</h4>
          <button type="button" className="sw-ten-x" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="sw-ten-list">
          {rows.map((r, i) => (
            <div key={i} className={`sw-ten-row${r.net > 0 ? " win" : ""}`}>
              <span>{r.label}</span>
              <span className="num">{r.net >= 0 ? `+${r.net}` : r.net}</span>
            </div>
          ))}
        </div>
        <div className="sw-ten-total num">
          {totalLabel.replace("{net}", String(total))}
        </div>
      </div>
    </div>
  );
}
