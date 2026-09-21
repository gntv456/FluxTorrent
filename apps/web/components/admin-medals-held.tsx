"use client";

/**
 * 勋章管理·持有浏览子面板（从 components/admin-medals.tsx 按域拆出）：
 * 全站持有记录（按 UID 过滤）与勋章回收（授予入口在用户详情页）。
 */

import { api, ApiError } from "@/lib/api-client";
import type { UserMedalRow } from "./admin-medals-shared";

interface HeldPanelProps {
  held: UserMedalRow[];
  heldUid: string;
  setHeldUid: (v: string) => void;
  busy: boolean;
  flash: (m: string) => void;
  load: () => Promise<void>;
}

/** UID 过滤输入框样式 */
const UID_FILTER_CLS =
  "min-h-[32px] w-40 rounded-full border border-line px-3 text-xs";

export function MedalHeldPanel({
  held,
  heldUid,
  setHeldUid,
  busy,
  flash,
  load,
}: HeldPanelProps) {
  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex items-end gap-2">
        <h3 className="text-sm font-bold">持有浏览 / 回收</h3>
        <input
          value={heldUid}
          onChange={(e) => setHeldUid(e.target.value)}
          placeholder="按用户 UID 过滤"
          className={UID_FILTER_CLS}
        />
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">用户</td>
            <td className="colhead">勋章</td>
            <td className="colhead">来源</td>
            <td className="colhead">佩戴</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {held.map((h) => (
            <tr key={`${h.user_id}-${h.medal_id}`}>
              <td>
                <a
                  href={`/admin/users/${h.user_id}`}
                  className="font-bold text-link"
                >
                  {h.username}
                </a>
              </td>
              <td>{h.medal_name}</td>
              <td>{h.source}</td>
              <td>{h.wearing ? "佩戴中" : "—"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      await api.post("/api/v1/admin/user-medals/delete", {
                        user_id: h.user_id,
                        medal_id: h.medal_id,
                      });
                      flash("已回收");
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "回收失败");
                    }
                  }}
                >
                  回收
                </button>
              </td>
            </tr>
          ))}
          {held.length === 0 && (
            <tr>
              <td colSpan={5} className="py-4 text-center text-sub">
                暂无持有记录
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
