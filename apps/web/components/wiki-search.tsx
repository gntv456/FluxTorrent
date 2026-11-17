"use client";

/** Wiki 目录页检索框（客户端即时过滤）。
 *
 *  全量目录由服务端 RSC 拿好，这里只做本地过滤——wiki 是站长手写的
 *  小语料库（几百条封顶），不值得每次击键都打一趟 /wiki/search；
 *  服务端端点留给站外/无目录上下文的调用方（OpenAPI 生态）。
 *  受控输入 + onFilter 回调，过滤逻辑留在 RSC 侧的客户端树外，
 *  键盘可达（label 关联）与触屏目标（44px）都按本仓无障碍口径。 */
import { useId, useState } from "react";

import { WIKI_SEARCH_INPUT } from "@/lib/ui-classes";

export function WikiSearch({
  placeholder,
  onFilter,
}: {
  placeholder: string;
  onFilter: (q: string) => void;
}) {
  const [q, setQ] = useState("");
  const id = useId();
  return (
    <div className="relative">
      <label htmlFor={id} className="sr-only">
        {placeholder}
      </label>
      <input
        id={id}
        type="search"
        value={q}
        placeholder={placeholder}
        onChange={(e) => {
          setQ(e.target.value);
          onFilter(e.target.value.trim());
        }}
        className={WIKI_SEARCH_INPUT}
      />
      <span
        aria-hidden
        className={"pointer-events-none absolute right-4 top-1/2 " +
          "-translate-y-1/2 text-sub"}
      >
        🔍
      </span>
    </div>
  );
}
