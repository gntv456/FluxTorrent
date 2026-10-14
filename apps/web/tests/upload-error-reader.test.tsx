import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { UploadForm } from "@/components/upload-form";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 发种口拒单的「读者」闸门（批九十三，P-303）。
 *
 * 背景：P-300 在上传边界新增「info.private 必须为 1」的拒单，但它的 code
 * 是 3003（TorrentInvalid）——这个码在 Rust 侧被十几种畸形共用，逐条原因只
 * 存在于 envelope.message（后端已把标签拼在前面：「种子文件无效: 种子未标记
 * 私有…」）。前端旧写法是 `dict.errors[code] ?? message`，字典里 3003 只有
 * 一句「种子文件无效」，于是成员上传非私有种时只看到笼统标签，照着什么都
 * 改不动——这属 P-281 的反向形态（不是丢成通用「加载失败」，而是被同族共用
 * 的粗标签吃掉具体原因）。
 *
 * 三条一起跑才叫验过优先级改对了：
 *  1) 有逐条原因 → 必须原样出现在界面（不被 3003 的粗标签顶掉）
 *  2) 1002 但信封里没有字段级错误 → 同样要露出后端那句原因
 *  3) 负控：信封没有 message 时仍须落回字典粗标签（证明我不是把字典删了
 *     或改成只看 message，那会让无文案的老码只剩「上传失败」）
 */

const REASON_PRIV =
  "种子文件无效: 种子未标记私有：.torrent 的 info.private 必须为 1。" +
  "请用客户端另存为「私有种子」后重新发布";
const REASON_NAME = "参数校验失败: 种子名不能为空";

let postReply: Record<string, unknown> = {};
const calls: string[] = [];

function envelope(payload: unknown) {
  return { code: 0, message: "ok", data: payload, request_id: "t" };
}

beforeEach(() => {
  calls.length = 0;
  postReply = { code: 3003, message: REASON_PRIV, data: null };
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      calls.push(url);
      const json = (obj: unknown) => ({
        ok: true,
        status: 200,
        json: async () => obj,
      });
      if (url.includes("/api/v1/site-profile")) {
        return json(
          envelope({
            categories: [{ id: 7, name: "剧集" }],
            metadata_sources: ["imdb"],
          }),
        );
      }
      if (url.includes("/api/v1/tags-dict")) return json(envelope([]));
      if (url.includes("/api/v1/section-dict")) return json(envelope([]));
      if (url.includes("/api/v1/torrents")) {
        return json({ request_id: "t2", ...postReply });
      }
      return json(envelope(null));
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

async function submitTorrent() {
  render(<UploadForm />);
  await waitFor(() => expect(calls.some((u) => u.includes("site-profile"))));
  const fileInput = document.querySelector(
    'input[type="file"]',
  ) as HTMLInputElement;
  expect(fileInput).not.toBeNull();
  await userEvent.upload(
    fileInput,
    new File([Uint8Array.from([1, 2, 3])], "x.torrent"),
  );
  // jsdom 不给 type=submit 的按钮合成 submit 事件（点击只派发 click），
  // 这里直接对表单派发 submit —— 走的是组件里同一个 onSubmit
  const form = document.getElementById("upload-form") as HTMLFormElement;
  expect(form).toBeInstanceOf(HTMLFormElement);
  fireEvent.submit(form);
  const st = await screen.findByRole("status");
  return st.textContent ?? "";
}

describe("发种拒单的读者（P-303）", () => {
  it("3003 的逐条原因必须露出来，不被共用码的粗标签顶掉", async () => {
    const text = await submitTorrent();
    expect(text).toContain("info.private");
    expect(text).toContain("私有");
    expect(text).not.toEqual(zhCN.errors?.[3003]);
    expect(text).not.toEqual(zhCN.upload.fail);
  });

  it("1002 没有字段级信封时同样露后端原因", async () => {
    postReply = { code: 1002, message: REASON_NAME, data: {} };
    const text = await submitTorrent();
    expect(text).toContain("种子名不能为空");
  });

  it("负控：信封没给 message 仍落回字典粗标签（没把兜底删掉）", async () => {
    postReply = { code: 3003, message: "", data: null };
    const text = await submitTorrent();
    expect(text).toEqual(zhCN.errors?.[3003]);
  });

  it("内部诊断形态的 message 不摊给用户（serde 原文只剩粗标签）", async () => {
    // 实测抽样：1002 缺字段时后端把 serde 诊断原样放进 message
    postReply = {
      code: 1002,
      message:
        "参数校验失败: Json deserialize error: missing field `email` " +
        "at line 1 column 16",
      data: {},
    };
    const text = await submitTorrent();
    expect(text).not.toContain("deserialize");
    expect(text).not.toContain("column");
    expect(text).toEqual(zhCN.errors?.[1002]);
  });
});
