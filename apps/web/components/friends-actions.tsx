"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";

/** 好友操作面板（审计修复 P1 闭环补全）：后端 /friends + /friends/action 早已存在，
 *  前端却只有只读列表——加好友/拉黑/接受申请全程无入口。
 *  覆盖：①按用户名加好友（发 pending 申请）②接受/拒绝收到的申请 ③拉黑/取消拉黑。 */
interface FriendRow {
  username: string;
  /** friend | black | incoming(他人发给我的 pending 申请) */
  list: string;
}

export function FriendsActions() {
  const { dict } = useI18n();
  const router = useRouter();
  const t = dict.friendsActions ?? {
    addTitle: "添加好友",
    addPlaceholder: "对方用户名",
    addBtn: "发送申请",
    incoming: "收到的好友申请",
    accept: "接受",
    reject: "拒绝",
    blackTitle: "拉黑用户",
    blackBtn: "拉黑",
    unblackBtn: "取消拉黑",
    ok: "操作成功",
  };
  const [rows, setRows] = useState<FriendRow[] | null>(null);
  const [addName, setAddName] = useState("");
  const [blackName, setBlackName] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.get<FriendRow[]>("/api/v1/friends"));
    } catch {
      // P-314：好友列表拉取失败此前 setRows([]) ⇒ 演成「你一个好友都没有」；
      // 置回 null（本 state 类型本就是 FriendRow[] | null）并用本文件写操作
      // 一直在用的全局 toast 报一句，净增 0 行。
      setRows(null); toast.error(dict.common.loadFailed);
    }
  }, [dict]);

  useEffect(() => {
    void load();
  }, [load]);

  async function act(path: string, body: unknown, okText?: string) {
    setBusy(true);
    try {
      await api.post(path, body);
      // A1 反馈协议：写操作结果走 toast（失败带 requestId 芯片）
      toast.success(okText ?? t.ok);
      await load();
      // 页面上半部的好友/黑名单是 RSC 渲染的：不刷新的话，接受申请后
      // 对方从「收到的申请」消失却不出现在「好友」区，拉黑同理。
      router.refresh();
    } catch (e) {
      toast.error(
        e instanceof ApiError ? e.message : dict.common.networkError,
        { requestId: e instanceof ApiError ? e.requestId : undefined },
      );
    } finally {
      setBusy(false);
    }
  }

  // /friends 只认 username；接受申请 = 再次 POST /friends（后端互 pending 自动转正）；
  // 拒绝 = /friends/action remove；拉黑 = /friends/action black。
  const incoming = (rows ?? []).filter((r) => r.list === "incoming");
  const blacks = (rows ?? []).filter((r) => r.list === "black");

  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-sm">
          <span className="text-sub">{t.addTitle}</span>
          <input
            className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm"
            value={addName}
            onChange={(e) => setAddName(e.target.value)}
            placeholder={t.addPlaceholder}
          />
        </label>
        <button
          type="button"
          disabled={busy || !addName.trim()}
          className="min-h-[36px] rounded-full bg-sky px-3 text-xs font-bold text-white disabled:opacity-50"
          onClick={() =>
            void act("/api/v1/friends", {
              username: addName.trim(),
              action: "add",
            })
          }
        >
          {t.addBtn}
        </button>
        <label className="flex flex-col gap-1 text-sm">
          <span className="text-sub">{t.blackTitle}</span>
          <input
            className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm"
            value={blackName}
            onChange={(e) => setBlackName(e.target.value)}
            placeholder={t.addPlaceholder}
          />
        </label>
        <button
          type="button"
          disabled={busy || !blackName.trim()}
          className="min-h-[36px] rounded-full bg-line px-3 text-xs font-bold text-ink disabled:opacity-50"
          onClick={() =>
            void act("/api/v1/friends/action", {
              username: blackName.trim(),
              action: "black",
            })
          }
        >
          {t.blackBtn}
        </button>
      </div>

      {blacks.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <span className="text-sub">{t.blackTitle}：</span>
          {blacks.map((b) => (
            <span key={b.username} className="flex items-center gap-1">
              <span className="pill">{b.username}</span>
              <button
                type="button"
                className="font-bold text-sky"
                disabled={busy}
                onClick={() =>
                  void act("/api/v1/friends/action", {
                    username: b.username,
                    action: "unblack",
                  })
                }
              >
                ✕
              </button>
            </span>
          ))}
        </div>
      )}

      {incoming.length > 0 && (
        <div className="flex flex-col gap-1 rounded-[var(--r-sm)] border border-sun/40 bg-sun/10 p-2">
          <span className="text-xs font-bold">{t.incoming}</span>
          {incoming.map((r) => (
            <div key={r.username} className="flex items-center gap-2 text-sm">
              <span className="font-bold">{r.username}</span>
              <button
                type="button"
                disabled={busy}
                className="min-h-[28px] rounded-full bg-mint px-2 text-xs font-bold text-white"
                onClick={() =>
                  void act("/api/v1/friends/action", {
                    username: r.username,
                    action: "accept",
                  })
                }
              >
                {t.accept}
              </button>
              <button
                type="button"
                disabled={busy}
                className="min-h-[28px] rounded-full bg-line px-2 text-xs font-bold text-ink"
                onClick={() =>
                  void act("/api/v1/friends/action", {
                    username: r.username,
                    action: "reject",
                  })
                }
              >
                {t.reject}
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
