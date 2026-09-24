"use client";

/**
 * 后台道具管理·用户背包子表（从 components/admin-props.tsx 按域拆出）：
 * 购买 + 发放记录浏览（按 UID 过滤）、即时生效类提示、背包道具回收。
 */

import { api, ApiError } from "@/lib/api-client";
import type { UserPropRow } from "./admin-props-shared";
import { kindLabelMap } from "./admin-props-shared";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

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
  const { dict, locale } = useI18n();
  const at = dict.adminProps;
  const KIND_LABEL = kindLabelMap(at.kindLabel);
  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex items-end gap-2">
        <h3 className="text-sm font-bold">{at.bagTitle}</h3>
        <input
          value={uid}
          onChange={(e) => setUid(e.target.value)}
          placeholder={at.qUid}
          className={UID_FILTER_CLS}
        />
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thOrder}</td>
            <td className="colhead">{at.bagUser}</td>
            <td className="colhead">{at.thItem}</td>
            <td className="colhead">{at.thKind}</td>
            <td className="colhead">{at.bagPrice}</td>
            <td className="colhead">{at.bagTime}</td>
            <td className="colhead text-right">{at.thAction}</td>
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
                {new Date(p.created_at).toLocaleString(dateLocale(locale))}
              </td>
              <td className="text-right">
                {[
                  "upload_credit",
                  "gift_spark",
                  "invite",
                  "temp_invite",
                ].includes(p.kind) ? (
                  <span className="text-sub">{at.instant}</span>
                ) : (
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={busy}
                    onClick={async () => {
                      try {
                        await api.del(`/api/v1/admin/user-props/${p.order_id}`);
                        flash(at.revoked);
                        await load();
                      } catch (e) {
                        flash(
                          e instanceof ApiError ? e.message : at.revokeFail,
                        );
                      }
                    }}
                  >
                    {at.revoke}
                  </button>
                )}
              </td>
            </tr>
          ))}
          {props.length === 0 && (
            <tr>
              <td colSpan={7} className="py-4 text-center text-sub">
                {at.bagEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
