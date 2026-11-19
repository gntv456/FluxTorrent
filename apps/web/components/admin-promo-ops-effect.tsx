import type { PromoEffect } from "./admin-promo-ops";

/** 促销效果归因表（0503 抽件——admin-promo-ops 300 行门禁）。 */
export function PromoEffectTable({ rows }: { rows: PromoEffect[] | null }) {
  return (
    <div className="brand-panel p-4">
      <h2 className="mb-3 font-display text-lg">
        促销效果归因（近 10 场 · 窗口内完成下载）
      </h2>
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-line text-left text-xs text-sub">
            <th className="py-2">促销</th>
            <th className="py-2">范围</th>
            <th className="py-2">完成下载</th>
            <th className="py-2">独立用户</th>
          </tr>
        </thead>
        <tbody>
          {(rows ?? []).length === 0 && (
            <tr><td colSpan={4} className="py-6 text-center text-sub">暂无促销记录</td></tr>
          )}
          {(rows ?? []).map((e) => (
            <tr key={e.id} className="border-b border-line/50">
              <td className="py-2 font-mono text-xs">#{e.id} {e.kind}</td>
              <td className="py-2">{e.scope}</td>
              <td className="py-2 font-mono">{e.downloads.toLocaleString()}</td>
              <td className="py-2 font-mono">{e.users.toLocaleString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
