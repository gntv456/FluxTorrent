"use client";

/** Toast 反馈协议（DS-PLAN-01 W-01 / §3.1：反馈通道唯一形态）。
 *
 *  背景：全站写操作反馈三态并存——有的弹 window.alert、有的静默刷新、
 *  有的页面私有实现（admin-user-detail / game-kit-feedback 两孤岛）。
 *  本件把「成功/失败/提示/警告」收敛为编程式调用：
 *
 *    import { toast } from "@/components/ui/toast";
 *    toast.success("已保存");            // 5s 自动消失
 *    toast.error(msg, { requestId });    // 失败可诊断：request_id 芯片
 *    toast.info(msg, { action: {...} }); // 带动作（重试/撤销）
 *
 *  设计规格（§3.1）：
 *  - 轻量事件订阅（模块级 bus + useSyncExternalStore 同款手写订阅），
 *    **不引入 redux/zustand**——330 个存量组件经不起大迁移；
 *  - Provider（./toast-provider.tsx）挂根布局一次，页面/组件任意深度
 *    编程式调用，RSC 不受影响；
 *  - aria：非 error 走 role="status" + aria-live="polite"；error
 *    独立 role="alert" + aria-live="assertive"（不被 polite 吞掉）；
 *  - 5s 自动消失，悬停/聚焦暂停（读完再走）；duration=0 常驻；
 *    Esc 关闭栈顶单条（键盘通道，见 ToastCard onKeyDown）；
 *  - 同屏上限 4 条：多的挤掉最旧（防批量写操作刷屏）；
 *  - 移动端（<768px）从顶部下滑、桌面右下角（CSS 分流，见 ui.css）；
 *  - 收编两孤岛（W-34）：admin-user-detail / game-kit-feedback 的
 *    私有反馈实现改调本协议。
 *
 *  手测说明（三形态）：
 *  1. toast.success("已保存") —— 右下角滑入绿色 ✓ 卡片，5s 后自动
 *     消失；鼠标悬停或 Tab 聚焦期间倒计时暂停，移开后继续；
 *  2. toast.error("操作失败", { requestId: "…" }) —— assertive 通道
 *     独立渲染红色卡片，附 request_id 芯片，点击一键复制后显示
 *     「已复制」；错误不自动消失前悬停同样暂停；
 *  3. toast.info("已加入队列", { action: { label: "查看",
 *     onClick } }) —— 蓝色信息卡片带动作按钮，点击动作即关闭本条。
 *  共通：右上 ✕ 关闭单条（aria-label 三语）；Esc 关闭最新一条；
 *  连续触发 5 次只保留最新 4 条；<768px 全部改为顶部下滑。
 */

import { useEffect, useState } from "react";

import { ApiError } from "@/lib/api-client";

export type ToastTone = "success" | "error" | "info" | "warn";

export interface ToastOptions {
  /** 自动消失毫秒数，默认 5000；0 = 常驻（需用户手动关） */
  duration?: number;
  /** 第二行说明（生效影响列表等次要信息；不传则只渲染一行） */
  desc?: string;
  /** 动作按钮（重试 / 撤销 / 查看），点击后 toast 关闭 */
  action?: { label: string; onClick: () => void };
  /** 后端信封 request_id：渲染为可一键复制芯片（失败可诊断） */
  requestId?: string;
}

export interface ToastItem {
  id: number;
  tone: ToastTone;
  message: string;
  desc?: string;
  duration: number;
  action?: ToastOptions["action"];
  requestId?: string;
}

// ---------- 模块级事件总线（无外部状态库） ----------
let seq = 0;
let items: ToastItem[] = [];
const listeners = new Set<(list: ToastItem[]) => void>();
/** 同屏上限 4 条：超出挤掉最旧的（防批量写操作刷屏） */
const MAX_VISIBLE = 4;

function emit() {
  for (const l of listeners) l(items);
}

function push(
  tone: ToastTone,
  message: string,
  opts?: ToastOptions,
): number {
  const id = ++seq;
  const item: ToastItem = {
    id,
    tone,
    message,
    desc: opts?.desc,
    duration: opts?.duration ?? 5000,
    action: opts?.action,
    requestId: opts?.requestId,
  };
  items = [...items, item].slice(-MAX_VISIBLE);
  emit();
  return id;
}

export const toast = {
  success: (message: string, opts?: ToastOptions) =>
    push("success", message, opts),
  error: (message: string, opts?: ToastOptions) =>
    push("error", message, opts),
  info: (message: string, opts?: ToastOptions) =>
    push("info", message, opts),
  warn: (message: string, opts?: ToastOptions) =>
    push("warn", message, opts),
  dismiss: (id: number) => {
    items = items.filter((t) => t.id !== id);
    emit();
  },
  /** 原地改写一条 toast（optimistic 变体翻转态用） */
  update: (id: number, patch: Partial<Omit<ToastItem, "id">>) => {
    items = items.map((t) => (t.id === id ? { ...t, ...patch } : t));
    emit();
  },
  /** 乐观更新（DS-PLAN-01 W-17 / §12.2「不等待」）：
   *  即时以成功态反馈，动作失败时翻转错误态（UI 回滚由调用方负责，
   *  本件只负责把「假成功」改口成「失败+原因」）。
   *
   *    const ok = await toast.optimistic("已收藏", {
   *      run: () => api.post("/api/v1/favorites", { id }),
   *      rollbackMessage: t.fav.rollback,   // 三语文案（必填）
   *    });
   *    if (!ok) setFav(false);              // 回滚 UI
   */
  optimistic: async (
    message: string,
    opts: {
      run: () => Promise<unknown>;
      /** 失败时翻转展示的文案（三语，调用方从 dict 取——必填，
       *  防模块级兜底文案绕过 i18n 门禁） */
      rollbackMessage: string;
      duration?: number;
    },
  ) => {
    const id = push("success", message, { duration: opts.duration });
    try {
      await opts.run();
      return true;
    } catch (e) {
      toast.update(id, {
        tone: "error",
        message: opts.rollbackMessage,
        requestId: e instanceof ApiError ? e.requestId : undefined,
      });
      return false;
    }
  },
};

/** 订阅 toast 队列（Provider/viewport 内部用；业务代码只 import toast） */
export function useToastQueue(): ToastItem[] {
  // 手写订阅（useState + useEffect）：与 useSyncExternalStore 等效，
  // 但不依赖 React 18 的 getServerSnapshot 分支——toast 只在 client。
  const [list, setList] = useState<ToastItem[]>(items);
  useEffect(() => {
    listeners.add(setList);
    // 订阅前可能已有入队（极少数同步场景），对齐一次
    setList(items);
    return () => {
      listeners.delete(setList);
    };
  }, []);
  return list;
}
