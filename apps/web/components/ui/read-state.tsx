import { EmptyState } from "./empty-state";
import type { ReactNode } from "react";
import type { Dict } from "@/i18n/server";

/** 读口的三态：失败 / 空 / 有数据。
 *
 *  为什么单独一个件：全站十几个列表页都要分这三档，而「取档失败」和
 *  「确实没有」在界面上必须是两句话——把失败渲染成空态，用户会以为
 *  自己没有记录，于是去发一条新的、或者放弃这一页，这比一句报错糟得多
 *  （弹窗普查那批里 13 处读失败演假状态就是这么来的）。
 *
 *  loading 不在这里：RSC 页的加载态由同目录 loading.tsx 负责。 */
export function ReadState({
  failed,
  empty,
  dict,
  emptyTitle,
  emptyDesc,
  emptyIcon = "info",
  emptyAction,
  children,
}: {
  /** 数据层取档失败（通常是 helper 回了 null） */
  failed?: boolean;
  /** 取到了，但是零行 */
  empty?: boolean;
  dict: Dict;
  emptyTitle: string;
  emptyDesc?: string;
  emptyIcon?: string;
  emptyAction?: ReactNode;
  children: ReactNode;
}) {
  const t = dict.readState;
  if (failed) {
    return (
      <EmptyState
        icon="info"
        tone="warn"
        title={t.failTitle}
        desc={t.failDesc}
      />
    );
  }
  if (empty) {
    return (
      <EmptyState
        icon={emptyIcon}
        title={emptyTitle}
        desc={emptyDesc}
        action={emptyAction}
      />
    );
  }
  return <>{children}</>;
}
