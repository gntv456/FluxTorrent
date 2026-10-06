"use client";

/** 道具管理·列表表格（从 admin-props.tsx 按域拆出）。 */
import { kindLabelMap } from "./admin-props-shared";
import { useI18n } from "@/i18n/client";
import type { ShopItemRow } from "./admin-props-shared";

/** 编辑表单载荷（价格/配置在表单里是字符串） */
export interface PropsEditForm {
  name: string;
  kind: string;
  price: string;
  config: string;
  active: boolean;
  stockQuota: string;
}

export function PropsTable(props: {
  items: ShopItemRow[];
  currency: string;
  busy: boolean;
  onEdit: (id: number, f: PropsEditForm) => void;
  onDel: (id: number) => void;
}) {
  const { items, currency, busy, onEdit, onDel } = props;
  const { dict } = useI18n();
  const at = dict.adminProps;
  const KIND_LABEL = kindLabelMap(at.kindLabel);
  return (
    <table className="nexus-table text-xs">
      <thead>
        <tr>
          <td className="colhead">{at.thId}</td>
          <td className="colhead">{at.thName}</td>
          <td className="colhead">{at.thKind}</td>
          <td className="colhead">{at.thPrice}</td>
          <td className="colhead">{at.thConfig}</td>
          <td className="colhead">{at.thStock}</td>
          <td className="colhead">{at.thStatus}</td>
          <td className="colhead text-right">{at.thAction}</td>
        </tr>
      </thead>
      <tbody>
        {items.map((it) => (
          <tr key={it.id} className={it.active ? "" : "opacity-50"}>
            <td className="num">{it.id}</td>
            <td className="font-bold">{it.name}</td>
            <td>
              {(KIND_LABEL[it.kind] ?? it.kind).replaceAll(
                "CURRENCY",
                currency,
              )}
            </td>
            <td className="num">{it.price}</td>
            <td className="max-w-[220px] truncate font-mono">
              {JSON.stringify(it.config)}
            </td>
            <td className="num">
              {it.stock_quota === null
                ? "∞"
                : `${it.stock_used}/${it.stock_quota}`}
            </td>
            <td>{it.active ? at.listed : at.unlisted}</td>
            <td className="text-right">
              <button
                className="cmgmt-act"
                onClick={() =>
                  onEdit(it.id, {
                    name: it.name,
                    kind: it.kind,
                    price: String(it.price),
                    config: JSON.stringify(it.config),
                    active: it.active,
                    stockQuota:
                      it.stock_quota === null ? "" : String(it.stock_quota),
                  })
                }
              >
                {at.edit}
              </button>
              <button
                className="cmgmt-act cmgmt-act--danger"
                disabled={busy}
                onClick={() => onDel(it.id)}
              >
                {at.delOrUnlist}
              </button>
            </td>
          </tr>
        ))}
        {items.length === 0 && (
          <tr>
            <td colSpan={7} className="py-6 text-center text-sub">
              {at.tableEmpty}
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
