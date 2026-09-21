"use client";

/**
 * 后台道具管理·用户背包子表（从 components/admin-props.tsx 按域拆出）：
 * 购买 + 发放记录浏览（按 UID 过滤）、即时生效类提示、背包道具回收。
 */

import { api, ApiError } from "@/lib/api-client";
import type { UserPropRow } from "./admin-props-shared";
import { KIND_LABEL } from "./admin-props-shared";

/** UID 过滤输入框样式 */
const UID_FILTER_CLS =
  "min-h-[32px] w-40 rounded-full border border-line px-3 text-xs";

interface PropsPanelProps {
  props: UserPropRow[];
  currency: string;
  uid: string;
  setUid: (v: string) => void;
  busy: boolean;
  flash: (m: string) => void;
  load: () => Promise<void>;
}

export function PropsPanel({
  props,
  currency,
  uid,
  setUid,
  busy,
  flash,
  load,
}: PropsPanelProps) {
  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex items-end gap-2">
        <h3 className="text-sm font-bold">用户背包（购买 + 发放）</h3>
        <input
          value={uid}
          onChange={(e) => setUid(e.target.value)}
          placeholder="按用户 UID 过滤"
          className={UID_FILTER_CLS}
        />
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">单号</td>
            <td className="colhead">用户</td>
            <td className="colhead">道具</td>
            <td className="colhead">类型</td>
            <td className="colhead">价格</td>
            <td className="colhead">时间</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {props.map((p) => (
            <tr key={p.order_id}>
              <td className="num">{p.order_id}</td>
              <td>
                <a
                  href={`/admin/users/${p.user_id}`}
                  className="font-bold text-link"
                >
                  {p.username}
                </a>
              </td>
              <td>{p.item_name}</td>
              <td>
                {(KIND_LABEL[p.kind] ?? p.kind).replaceAll(
                  "CURRENCY",
                  currency,
                )}
              </td>
              <td className="num">{p.price}</td>
              <td className="text-sub">
                {new Date(p.created_at).toLocaleString()}
              </td>
              <td className="text-right">
                {[
                  "upload_credit",
                  "gift_spark",
                  "invite",
                  "temp_invite",
                ].includes(p.kind) ? (
                  <span className="text-sub">即时生效</span>
                ) : (
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={busy}
                    onClick={async () => {
                      try {
                        await api.del(`/api/v1/admin/user-props/${p.order_id}`);
                        flash("已回收");
                        await load();
                      } catch (e) {
                        flash(e instanceof ApiError ? e.message : "回收失败");
                      }
                    }}
                  >
                    回收
                  </button>
                )}
              </td>
            </tr>
          ))}
          {props.length === 0 && (
            <tr>
              <td colSpan={7} className="py-4 text-center text-sub">
                暂无持有记录
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
