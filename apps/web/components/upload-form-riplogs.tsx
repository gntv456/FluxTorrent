"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { setRipFiles } from "@/lib/upload-fields";
import { FormRow } from "@/components/upload-form-parts";

/** 抓轨日志选择行（0312 音乐站 Logchecker）。
 *
 *  与 NFO 同属「随种子一起交的文本件」：多碟可一次选多个文件，提交顺序
 *  即详情页的「碟 1 / 碟 2」。选中的文件交给 `setRipFiles`，由
 *  `appendMeta` 在组装 FormData 时以 `log` 名义并成二进制 part。
 *
 *  站点没开 `logcheck_policy` 时后端直接忽略这些 part（不发分数、不落库），
 *  所以这一行对所有站型都是「有就用、没用不亏」。 */
export function UploadRipLogs() {
  const { dict } = useI18n();
  const [names, setNames] = useState<string[]>([]);
  return (
    <FormRow label={dict.upload.ripLogs}>
      <div className="flex flex-col gap-1">
        <label className="uf-drop uf-drop--optional">
          <input
            type="file"
            name="log"
            multiple
            accept=".log,text/plain"
            className="sr-only"
            onChange={(e) => {
              const picked = Array.from(e.target.files ?? []);
              setRipFiles(picked);
              setNames(picked.map((f) => f.name));
            }}
          />
          {names.length > 0 ? `📄 ${names.join(", ")}` : dict.upload.ripLogsHint}
        </label>
      </div>
    </FormRow>
  );
}
