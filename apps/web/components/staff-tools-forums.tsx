"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

import { CategoryManager } from "./staff-tools-forums-cats";
import { ForumEditForm } from "./staff-tools-forums-form";

/** 论坛版块管理面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  forummanage 口径——三档门槛 + 受保护 + 版主任免 + 分区管理。
 *  分区/节点管理拆至 ./staff-tools-forums-cats.tsx；
 *  新建/编辑表单拆至 ./staff-tools-forums-form.tsx。 */

export interface ForumAdminForum {
  id: number;
  name: string;
  descr: string | null;
  minclassread: number;
  minclasswrite: number;
  minclasscreate: number;
  protected: boolean;
  topics: number;
  category_id?: number | null;
  category_name?: string | null;
}
interface ForumCategory {
  id: number;
  name: string;
  sort: number;
  visible: boolean;
}
export interface ForumAdminData {
  forums: ForumAdminForum[];
  mods: [number, number, string][];
  categories?: ForumCategory[];
}

// 表格内的输入/小按钮
const MOD_INPUT =
  "ml-1 min-h-[28px] w-24 rounded-full border border-line " +
  "bg-cloud px-2 text-xs outline-none focus:border-sky";
const MOD_ADD_BTN =
  "ml-1 min-h-[28px] rounded-full bg-sky px-2 " +
  "text-xs font-bold text-white";
const ROW_BTN_SKY =
  "min-h-[28px] rounded-full border border-line px-3 " + "font-bold text-sky";
const ROW_BTN_DANGER =
  "ml-1 min-h-[28px] rounded-full border border-line px-3 " +
  "font-bold text-danger";
const MOD_TAG =
  "mr-1 inline-flex items-center gap-1 rounded-full " +
  "border border-line px-2 py-0.5";

export function StaffForumsPanel({ flash }: { flash: (m: string) => void }) {
  const { dict } = useI18n();
  const [forumData, setForumData] = useState<ForumAdminData | null>(null);
  const [fEditId, setFEditId] = useState<number | null>(null);
  const [fModName, setFModName] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    api
      .get<ForumAdminData | null>("/api/v1/admin/forums")
      .then(setForumData)
      .catch(() => setForumData(null));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-1 text-base font-bold">论坛版块管理</h2>
      <p className="mb-3 text-xs text-sub">
        三档门槛需满足 读 ≤ 回 ≤ 发（0 = 所有人）；受保护版块 2
        楼起正文对普通用户隐藏；版主任免无需等级，仅在本版块有效。
      </p>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">版块</td>
            <td className="colhead w-28">分区</td>
            <td className="colhead w-36">门槛 读/回/发</td>
            <td className="colhead w-14">保护</td>
            <td className="colhead w-14">主题</td>
            <td className="colhead">版主</td>
            <td className="colhead w-24" />
          </tr>
        </thead>
        <tbody>
          {(forumData?.forums ?? []).map((f) => {
            const mods = (forumData?.mods ?? []).filter((m) => m[0] === f.id);
            return (
              <tr key={f.id}>
                <td>
                  <Link href={`/forums/${f.id}`} className="font-bold text-sky">
                    {f.name}
                  </Link>
                  {f.descr && <p className="text-sub">{f.descr}</p>}
                </td>
                <td className="text-sub">{f.category_name ?? "—"}</td>
                <td className="num">
                  {f.minclassread} / {f.minclasswrite} / {f.minclasscreate}
                </td>
                <td className="text-center">{f.protected ? "🛡" : "—"}</td>
                <td className="num">{f.topics}</td>
                <td>
                  {mods.length === 0 ? (
                    <span className="text-sub">—</span>
                  ) : (
                    mods.map((m) => (
                      <span key={m[1]} className={MOD_TAG}>
                        {m[2]}
                        <button
                          className="font-bold text-danger"
                          title="移除版主"
                          onClick={() =>
                            guard(async () => {
                              await api.del(
                                `/api/v1/admin/forums/${f.id}/mods/${m[1]}`,
                              );
                              setForumData(
                                await api.get("/api/v1/admin/forums"),
                              );
                            }, "OK")
                          }
                        >
                          ×
                        </button>
                      </span>
                    ))
                  )}
                  <input
                    value={fModName}
                    onChange={(e) => setFModName(e.target.value)}
                    placeholder="用户名"
                    className={MOD_INPUT}
                  />
                  <button
                    className={MOD_ADD_BTN}
                    onClick={() =>
                      guard(async () => {
                        await api.post(`/api/v1/admin/forums/${f.id}/mods`, {
                          username: fModName.trim(),
                        });
                        setFModName("");
                        setForumData(await api.get("/api/v1/admin/forums"));
                      }, "已任命")
                    }
                  >
                    +版主
                  </button>
                </td>
                <td>
                  <button
                    className={ROW_BTN_SKY}
                    onClick={() => setFEditId(f.id)}
                  >
                    编辑
                  </button>
                  <button
                    className={ROW_BTN_DANGER}
                    onClick={() =>
                      guard(async () => {
                        await api.del(`/api/v1/admin/forums/${f.id}`);
                        setForumData(await api.get("/api/v1/admin/forums"));
                      }, "已删除")
                    }
                  >
                    删除
                  </button>
                </td>
              </tr>
            );
          })}
          {(forumData?.forums ?? []).length === 0 && (
            <tr>
              <td colSpan={7} className="py-4 text-center text-sub">
                {forumData === null ? "Load 失败或无权限" : "暂无版块"}
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <ForumEditForm
        forumData={forumData}
        setForumData={setForumData}
        busy={busy}
        guard={guard}
        fEditId={fEditId}
        setFEditId={setFEditId}
      />

      <CategoryManager
        forumData={forumData}
        setForumData={setForumData}
        busy={busy}
        guard={guard}
      />
    </section>
  );
}
