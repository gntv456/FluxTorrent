import Link from "next/link";

/** 筛选药丸组（分段控件外观）：当前项标 aria-current。
 *
 *  与 `Tabs` 的分工：Tabs 是「同一页换面板」，本件是「同一个列表换
 *  筛选条件」。两档模式：
 *    链接档 —— 选项带 href（SSR 页，点一下换 URL）；
 *    回调档 —— 选项不带 href、改传 onChange（客户端页自己拉数据）。
 *  外观沿用版块页排序/类型那两组药丸的既有分段样式（`.seg-*`），
 *  新页面直接复用，不再各写一份 rounded-full。 */
export interface SegOption {
  /** 查询参数的值；undefined = 「全部」那一档 */
  value?: string;
  label: string;
  /** 链接档才给；不给则本项渲染成按钮 */
  href?: string;
}

export function FilterLinks({
  label,
  value,
  options,
  onChange,
  className = "",
}: {
  /** 无障碍组名（读屏器念这一组是干什么的） */
  label: string;
  /** 当前选中值；undefined/空串都落到 value 为 undefined 的那一档 */
  value?: string;
  options: SegOption[];
  /** 回调档：选项没有 href 时用它接管点击 */
  onChange?: (value: string | undefined) => void;
  className?: string;
}) {
  const cur = value && value.length > 0 ? value : undefined;
  return (
    <div className={`seg-row ${className}`} role="group" aria-label={label}>
      {options.map((o) => {
        const on = o.value === cur;
        const cls = `seg-item${on ? " seg-item--on" : ""}`;
        if (o.href) {
          return (
            <Link
              key={o.value ?? "__all"}
              href={o.href}
              aria-current={on ? "page" : undefined}
              className={cls}
            >
              {o.label}
            </Link>
          );
        }
        return (
          <button
            key={o.value ?? "__all"}
            type="button"
            aria-pressed={on}
            onClick={() => onChange?.(o.value)}
            className={cls}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
