"use client";

/** 道具管理·列表表格（从 admin-props.tsx 按域拆出）。 */
import { KIND_LABEL } from "./admin-props-shared";
import type { ShopItemRow } from "./admin-props-shared";

/** 编辑表单载荷（价格/配置在表单里是字符串） */
export interface PropsEditForm {
  name: string;
  kind: string;
  price: string;
  config: string;
  active: boolean;
}

export function PropsTable(props: {
  items: ShopItemRow[];
  currency: string;
  busy: boolean;
  onEdit: (id: number, f: PropsEditForm) => void;
  onDel: (id: number) => void;
}) {
  const { items, currency, busy, onEdit, onDel } = props;
  return (
    <table className="nexus-table text-xs">
      <thead>
        <tr>
          <td className="colhead">ID</td>
          <td className="colhead">名称</td>
          <td className="colhead">类型</td>
          <td className="colhead">价格</td>
          <td className="colhead">config</td>
          <td className="colhead">状态</td>
          <td className="colhead text-right">操作</td>
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
            <td>{it.active ? "上架" : "下架"}</td>
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
                  })
                }
              >
                编辑
              </button>
              <button
                className="cmgmt-act cmgmt-act--danger"
                disabled={busy}
                onClick={() => onDel(it.id)}
              >
                删除/下架
              </button>
            </td>
          </tr>
        ))}
        {items.length === 0 && (
          <tr>
            <td colSpan={7} className="py-6 text-center text-sub">
              暂无道具
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}
