/**
 * 发种表单的元数据装配（0288 从 upload-form.tsx 拆出，300 行门禁）。
 *
 * 拆出的另一半原因是**通道**：这些字段过去逐个拼进 URL 的 query string，
 * 于是简介的真实上限由 HTTP 请求行决定而不是业务规则（实测 8192 汉字 → 400 且
 * 响应体为空、16384 字 → 431，用户只看到「网络错误」）。现在统一写成 multipart
 * 文本字段，服务端两条通道都收（`/open/torrents` 等老客户端仍可走 query）。
 */

export interface SecKindDef {
  kind: string;
  field_type?: string;
}

/** 多维属性打包（B2 六类型）：枚举维度发整数（旧格式，后端零改动兼容），
 *  自由值维度按 field_type 发对象 `{"text":…}/{"number":…}/{"date":…}/{"bool":…}`，
 *  multiselect 发 `{"dict_ids":[…]}`。后端按**值的 JSON 类型**分派。 */
export function packSections(
  secVals: Record<string, string>,
  secKinds: SecKindDef[],
): Record<string, unknown> {
  const sections: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(secVals)) {
    if (!v) continue;
    const type = secKinds.find((x) => x.kind === k)?.field_type ?? "select";
    if (type === "select") {
      const n = Number(v);
      if (n > 0) sections[k] = n;
    } else if (type === "multiselect") {
      const ids = v
        .split(",")
        .map((s) => Number(s.trim()))
        .filter((n) => n > 0);
      if (ids.length > 0) sections[k] = { dict_ids: ids };
    } else if (type === "number") {
      const n = Number(v);
      if (!Number.isNaN(n)) sections[k] = { number: n };
    } else if (type === "bool") {
      sections[k] = { bool: v === "true" };
    } else if (type === "date") {
      if (v.trim()) sections[k] = { date: v.trim() };
    } else if (v.trim()) {
      sections[k] = { text: v.trim() };
    }
  }
  return sections;
}

export interface MetaValues {
  category_id: number;
  anonymous: boolean;
  name: string;
  imdb: string;
  price: number;
  small_descr: string;
  descr: string;
  poster: string;
  mediainfo: string;
  sections: Record<string, unknown>;
  tags: number[];
  pos_state: number;
  pos_state_until: string;
  pick_type: number;
}

/** 发种表单当前选中的抓轨日志（0312），由 <UploadRipLogs /> 写入。
 *  放模块层而非主表单的 ref：主表单已在行数基线上，加不起一对 ref 声明 +
 *  属性透传；appendMeta 只有发种提交一个调用方，不会串台。 */
let ripFiles: File[] = [];

export function setRipFiles(files: File[]) {
  ripFiles = files;
}

/** 组装成 FormData（空串一律不提交，避免把「没填」写成「填了空值」） */
export function appendMeta(fd: FormData, v: MetaValues) {
  const put = (k: string, val: string) => {
    if (val !== "") fd.append(k, val);
  };
  fd.append("category_id", String(v.category_id));
  fd.append("anonymous", String(v.anonymous));
  put("name", v.name.trim());
  put("imdb", v.imdb.trim());
  if (v.price > 0)
    put("price", String(Math.min(1_000_000, Math.max(0, v.price))));
  put("small_descr", v.small_descr.trim());
  put("descr", v.descr);
  put("poster", v.poster.trim());
  put("mediainfo", v.mediainfo.trim());
  if (Object.keys(v.sections).length > 0)
    put("sections", JSON.stringify(v.sections));
  if (v.tags.length > 0) put("tags", JSON.stringify(v.tags));
  if (v.pos_state > 0) {
    put("pos_state", String(v.pos_state));
    if (v.pos_state_until) put("pos_state_until", v.pos_state_until);
  }
  if (v.pick_type > 0) put("pick_type", String(v.pick_type));
  // 抓轨日志（0312）：二进制 part，与上面的文本字段不同通道。文本通道按
  // 256 KiB 截断并做 UTF-8 lossy，而 EAC 日志常是 GBK 码页、判分依据又
  // 正好在尾部（No errors occurred / End of status report）。
  for (const f of ripFiles) fd.append("log", f);
}
