"use client";

import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import {
  BTN_SKY,
  type ForumAdminForum,
} from "./forum-structure-types";

/** 右栏：选中分区下的版块表（对齐样图五列）。
 *
 *  样图列：版块 | 说明 | 主题 / 回复 | 状态 | 操作（编辑 / 排序）。
 *  从本表移除的列没有丢功能，各自搬了家：
 *   - 三档门槛 / 受保护 / 排序值 / 规则 → 编辑抽屉（本就在那里）
 *   - 版主任免 → 编辑抽屉新增「版主」区
 *   - 删除版块 → 编辑抽屉底部危险按钮（带非空确认）
 *
 *  「排序」= 与下一个版块交换：交换后按新序号重排落库。常规数据
 *  （sort 稠密）只发两条 PUT；历史脏数据（sort 全同）会顺带重排整列。
 */
export function ForumBoardTable({
  forums,
  catName,
  busy,
  run,
  onEdit,
  onNew,
}: {
  forums: ForumAdminForum[];
  catName: string;
  busy: boolean;
  run: (fn: () => Promise<void>, ok: string) => void;
  onEdit: (f: ForumAdminForum) => void;
  onNew: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminForums;

  /** 整行 payload（PUT 是全量语义：省略的 gates 会被写成 0，必须带原值） */
  function payload(f: ForumAdminForum, sort: number) {
    return {
      name: f.name,
      descr: f.descr,
      rules: f.rules ?? null,
      minclassread: f.minclassread,
      minclasswrite: f.minclasswrite,
      minclasscreate: f.minclasscreate,
      protected: f.protected,
      category_id:
        typeof f.category_id === "number" ? f.category_id : null,
      sort,
    };
  }

  /** 与下一个版块交换顺序；末位禁用（想上移就点上面那一行的排序）。
   *
   *  常规状态（sort 已稠密、各自等于下标）只需发两条 PUT；历史脏数据
   *  （sort 全同 / 跳号 / 负值）按下标赋值会错序，此时按新顺序把整列
   *  重排——重排后 sort === 下标，之后又回到两条的快路径。 */
  function swapNext(i: number) {
    const order = [...forums];
    [order[i], order[i + 1]] = [order[i + 1], order[i]];
    const dense = forums.every((b, idx) => b.sort === idx);
    const moves = order
      .map((b, idx) => ({ b, idx }))
      .filter(({ b, idx }) => !dense || b.sort !== idx);
    if (moves.length === 0) return;
    run(
      async () => {
        for (const { b, idx } of moves) {
          await api.put(`/api/v1/admin/forums/${b.id}`, payload(b, idx));
        }
      },
      t.boardSorted,
    );
  }

  const num = (n: number) => n.toLocaleString();

  return (
    <div className="brand-panel flex min-w-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-line px-3.5
        py-2.5">
        <b className="text-sm text-ink">
          {fmt(t.catPanelTitle, { name: catName })}
        </b>
        <span className="flex-1" />
        <button
          type="button"
          className={`${BTN_SKY} min-h-[30px] px-3 text-xs`}
          onClick={onNew}
        >
          {t.newBoardBtn}
        </button>
      </div>

      <div className="overflow-x-auto">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{t.colBoard}</td>
              <td className="colhead">{t.colDescr}</td>
              <td className="colhead w-32">{t.colStats}</td>
              <td className="colhead w-20">{t.colStatus}</td>
              <td className="colhead w-28">{t.colOps}</td>
            </tr>
          </thead>
          <tbody>
            {forums.map((f, i) => {
              const open =
                f.minclassread === 0 &&
                f.minclasswrite === 0 &&
                f.minclasscreate === 0;
              return (
                <tr key={f.id}>
                  <td>
                    <Link
                      href={`/forums/${f.id}`}
                      className="font-bold text-sky"
                    >
                      {f.name}
                    </Link>
                  </td>
                  <td className="text-sub">{f.descr || "—"}</td>
                  <td className="num">
                    {num(f.topics)} / {num(f.posts ?? 0)}
                  </td>
                  <td>
                    <span
                      className={`rounded-full px-2 py-0.5 text-xs font-bold ${
                        open
                          ? "bg-mint-soft text-mint"
                          : "bg-sun-soft text-ink"
                      }`}
                    >
                      {open ? t.statusOpen : t.statusLimited}
                    </span>
                  </td>
                  <td>
                    <button
                      type="button"
                      disabled={busy}
                      className="font-bold text-sky"
                      onClick={() => onEdit(f)}
                    >
                      {t.editBtn}
                    </button>
                    <button
                      type="button"
                      disabled={busy || i === forums.length - 1}
                      title={t.sortTip}
                      className="ml-2 font-bold text-sky
                        disabled:opacity-40"
                      onClick={() => swapNext(i)}
                    >
                      {t.sortBtn}
                    </button>
                  </td>
                </tr>
              );
            })}
            {forums.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {t.boardEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
