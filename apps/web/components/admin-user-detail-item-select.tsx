"use client";

/** 道具分组下拉（0287 从 admin-user-detail-actions-forms-more 拆出守 300 行门禁）。 */

export function ItemGroupSelect(props: {
  items: { id: number; name: string; kind: string }[];
  currency: string;
  IKL: Record<string, string>;
  itemId: string;
  setItemId: (v: string) => void;
  fieldCls: string;
  emptyLabel: string;
}) {
  const { items, currency, IKL, itemId, setItemId, fieldCls } = props;
  const groups = Object.entries(
    items.reduce<Record<string, typeof items>>((acc, it) => {
      const label = (IKL[it.kind] ?? it.kind).replaceAll(
        "CURRENCY",
        currency,
      );
      (acc[label] ??= []).push(it);
      return acc;
    }, {}),
  );
  return (
    <select
      value={itemId}
      onChange={(e) => setItemId(e.target.value)}
      className={`max-w-80 ${fieldCls}`}
    >
      <option value="">{props.emptyLabel}</option>
      {groups.map(([kind, list]) => (
        <optgroup key={kind} label={kind}>
          {list.map((it) => (
            <option key={it.id} value={it.id}>
              #{it.id} {it.name}
            </option>
          ))}
        </optgroup>
      ))}
    </select>
  );
}
