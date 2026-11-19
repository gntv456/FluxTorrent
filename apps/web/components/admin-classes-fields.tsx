"use client";

import { INPUT_MD } from "@/lib/ui-classes";

/** 等级规则表格的三个编辑原语（0501 从 admin-classes.tsx 拆出：行数门禁）。
 *  每个原语都带单位后缀——没有量纲提示，管理员面对裸输入框无从判断
 *  「下载量填 50」是 GB 还是 MB（站长反馈 2026-11-19）。 */

interface FieldCtx {
  /** 该行是否锁定（无规则档/职务档只读） */
  locked: boolean;
  /** 读草稿值 */
  get: () => number | null | undefined;
  /** 写草稿值（null = 清除该维度） */
  set: (v: number | null) => void;
  /** 后端仍是必填数值的旧四维用 0 兜底 */
  setLegacy: (v: number) => void;
}

/** 旧四维：必填数字（0 = 门槛为零），scale 换算（字节维度 GB 编辑） */
export function ClassNumCell({
  ctx,
  scale = 1,
  unit = "",
}: {
  ctx: FieldCtx;
  scale?: number;
  unit?: string;
}) {
  if (ctx.locked) return <span className="text-xs text-sub">—</span>;
  const raw = ctx.get();
  return (
    <span className="inline-flex items-center gap-1">
      <input
        type="number"
        className={`${INPUT_MD} w-24`}
        value={Number(raw ?? 0) / scale}
        onChange={(e) =>
          ctx.setLegacy(Math.round(Number(e.target.value) * scale))}
      />
      {unit && <span className="text-xs text-sub">{unit}</span>}
    </span>
  );
}

/** 0501 可空维度：空输入 = null（该维度不参与判定） */
export function ClassOptNumCell({
  ctx,
  scale = 1,
  unit = "",
  placeholder = "—",
}: {
  ctx: FieldCtx;
  scale?: number;
  unit?: string;
  placeholder?: string;
}) {
  if (ctx.locked) return <span className="text-xs text-sub">—</span>;
  const raw = ctx.get();
  return (
    <span className="inline-flex items-center gap-1">
      <input
        type="number"
        className={`${INPUT_MD} w-24`}
        placeholder={placeholder}
        value={raw === null || raw === undefined ? "" : Number(raw) / scale}
        onChange={(e) => {
          const t = e.target.value.trim();
          ctx.set(t === "" ? null : Math.round(Number(t) * scale));
        }}
      />
      {unit && <span className="text-xs text-sub">{unit}</span>}
    </span>
  );
}
