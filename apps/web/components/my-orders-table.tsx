import { fmt } from "@/i18n/config";
import type { Dict } from "@/i18n/zh-CN";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";

/** GET /me/orders 单行（2026-10-18 契约：0383 履约态三件套随行走，
 *  kind 改可空——SKU 被删时 LEFT JOIN 给 NULL）。
 *  本类型与 `@/lib/data` 的旧信封并行存在：那份还没带上 fulfill 参数，
 *  这一页要按履约态筛选，先把契约写在自己的取数点上，避免跨页改共享层。 */
export interface OrderRow {
  id: number;
  item_id: number;
  item_name: string | null;
  kind: string | null;
  price: number;
  effect_applied: boolean;
  fulfill_state: string | null;
  fulfilled_at: string | null;
  fail_reason: string | null;
  created_at: string;
}

/** 履约态筛选令牌（后端认这三个，其余含缺省 = 全部） */
export type FulfillKey = "" | "pending" | "done" | "failed";

export interface OrdersEnvelope {
  rows: OrderRow[];
  total: number;
  page: number;
  per_page: number;
  fulfill: string | null;
}

/** 钱到哪一步（fulfill_state）→ 徽章档位。
 *  与效果有没有落地（effect_applied）是两个正交判据：
 *  前者说明扣款/退款，后者说明道具发没发到账号上，各占一行不合并。 */
function fulfillBadge(state: string | null, t: Dict["myOrders"]) {
  switch (state) {
    case "done":
      return { tone: "success" as const, label: t.stateDone };
    case "reserving":
    case "frozen":
      return { tone: "warn" as const, label: t.statePending };
    case "failed":
      return { tone: "danger" as const, label: t.stateFailed };
    case "refunded":
      return { tone: "danger" as const, label: t.stateRefunded };
    default:
      return null;
  }
}

/** 我的订单表（商城订单中心 P2-1 + 履约态对齐样图）。
 *  纯展示：翻页条归页面（Pager 链接档），这里只出表。 */
export function MyOrdersTable({
  rows,
  total,
  fulfill,
  dict,
  currency,
}: {
  rows: OrderRow[];
  total: number;
  fulfill: FulfillKey;
  dict: Dict["myOrders"];
  currency: string;
}) {
  const t = dict;
  if (rows.length === 0) {
    return (
      <div className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.panelTitle}</h2>
        </div>
        <EmptyState
          icon="shop"
          title={fulfill ? t.emptyFiltered : t.empty}
          desc={fulfill ? t.empty : undefined}
        />
      </div>
    );
  }
  return (
    <div className="baozi-panel">
      <div className="baozi-panel__head">
        <h2>{t.panelTitle}</h2>
        <span className="text-xs text-sub">
          {fmt(t.total, { n: total.toLocaleString() })}
        </span>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table profilet">
          <tbody>
            <tr>
              <td className="colhead">{t.colNo}</td>
              <td className="colhead">{t.colItem}</td>
              <td className="colhead">{t.colKind}</td>
              <td className="colhead">{t.colPrice}</td>
              <td className="colhead">{t.colStatus}</td>
              <td className="colhead">{t.colAt}</td>
            </tr>
            {rows.map((r) => {
              const badge = fulfillBadge(r.fulfill_state, t);
              return (
                <tr key={r.id}>
                  <td className="num">
                    <span className="profilet__label">{t.colNo}</span>#
                    {r.id}
                  </td>
                  <td>
                    <span className="profilet__label">{t.colItem}</span>
                    {r.item_name ?? `#${r.item_id}`}
                  </td>
                  <td>
                    <span className="profilet__label">{t.colKind}</span>
                    {r.kind ? <span className="pill">{r.kind}</span> : "—"}
                  </td>
                  <td className="num">
                    <span className="profilet__label">{t.colPrice}</span>
                    {r.price.toLocaleString()} {currency}
                  </td>
                  <td>
                    <span className="profilet__label">{t.colStatus}</span>
                    <div className="flex flex-col items-start gap-1">
                      {badge ? (
                        <Badge tone={badge.tone}>{badge.label}</Badge>
                      ) : null}
                      <span
                        className={
                          r.effect_applied
                            ? "text-xs text-success"
                            : "text-xs text-danger"
                        }
                      >
                        {r.effect_applied ? t.applied : t.pending}
                      </span>
                    </div>
                  </td>
                  <td className="text-xs text-sub">
                    <span className="profilet__label">{t.colAt}</span>
                    {r.created_at.slice(0, 19).replace("T", " ")}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
