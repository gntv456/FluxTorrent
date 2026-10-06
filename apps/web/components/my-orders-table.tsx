import type { MyOrdersEnvelope } from "@/lib/data";
import type { Dict } from "@/i18n/zh-CN";

/** 我的订单表（商城订单中心 P2-1）。服务端取数 + 纯展示，
 *  分页先用「第 N / 共 M 页」链接翻页（?page=），够用不过度设计。 */
export function MyOrdersTable({
  rows,
  total,
  page,
  perPage,
  dict,
  currency,
}: {
  rows: MyOrdersEnvelope["rows"];
  total: number;
  page: number;
  perPage: number;
  dict: Dict["myOrders"];
  currency: string;
}) {
  const pages = Math.max(1, Math.ceil(total / perPage));
  return (
    <div className="baozi-panel">
      <div className="baozi-panel__head">
        <h2>{dict.title}</h2>
        <span className="text-xs text-sub">
          {dict.total.replace("{n}", total.toLocaleString())}
        </span>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table profilet">
          <tbody>
            <tr>
              <td className="colhead">{dict.colItem}</td>
              <td className="colhead">{dict.colKind}</td>
              <td className="colhead">{dict.colPrice}</td>
              <td className="colhead">{dict.colStatus}</td>
              <td className="colhead">{dict.colAt}</td>
            </tr>
            {rows.map((r) => (
              <tr key={r.id}>
                <td>
                  <span className="profilet__label">{dict.colItem}</span>
                  {r.item_name ?? `#${r.item_id}`}
                </td>
                <td>
                  <span className="profilet__label">{dict.colKind}</span>
                  <span className="pill">{r.kind}</span>
                </td>
                <td className="num">
                  <span className="profilet__label">{dict.colPrice}</span>
                  {r.price.toLocaleString()} {currency}
                </td>
                <td>
                  <span className="profilet__label">{dict.colStatus}</span>
                  <span
                    className={
                      r.effect_applied ? "text-success" : "text-danger"
                    }
                  >
                    {r.effect_applied ? dict.applied : dict.pending}
                  </span>
                </td>
                <td className="text-xs text-sub">
                  <span className="profilet__label">{dict.colAt}</span>
                  {r.created_at.slice(0, 19).replace("T", " ")}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {dict.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {pages > 1 && (
        <div className="flex items-center justify-center gap-3 p-3 text-sm">
          {page > 1 && (
            <a
              className="btn btn-sm btn-ghost"
              href={`/my/orders?page=${page - 1}`}
            >
              ‹ {dict.prev}
            </a>
          )}
          <span className="text-xs text-sub">
            {page} / {pages}
          </span>
          {page < pages && (
            <a
              className="btn btn-sm btn-ghost"
              href={`/my/orders?page=${page + 1}`}
            >
              {dict.next} ›
            </a>
          )}
        </div>
      )}
    </div>
  );
}
