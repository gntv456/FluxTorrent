"use client";

import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import type { UserSettings } from "@/components/usercp";

/** 论坛设定面板（从 usercp.tsx 按域拆出，300 行门禁）：
 *  每页主题/帖子数 / 头像签名开关 / 悬浮末贴 / 点击主题跳转 / 签名档。 */

export function ForumTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.usercp.forum;
  return (
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.topicsPerPage}>
          <input
            type="text"
            size={10}
            className="uc-num-input"
            value={s.topics_per_page}
            onChange={(e) => patch({ topics_per_page: Number(e.target.value) || 0 })}
          />
          {t.zeroDefault}
        </Row>
        <Row head={t.postsPerPage}>
          <input
            type="text"
            size={10}
            className="uc-num-input"
            value={s.posts_per_page}
            onChange={(e) => patch({ posts_per_page: Number(e.target.value) || 0 })}
          />{" "}
          {t.zeroDefault}
        </Row>
        <Row head={t.viewAvatars}>
          <label>
            <input
              type="checkbox"
              checked={s.view_avatars}
              onChange={(e) => patch({ view_avatars: e.target.checked })}
            />
            {t.lowBandwidth}
          </label>
        </Row>
        <Row head={t.viewSignatures}>
          <label>
            <input
              type="checkbox"
              checked={s.view_signatures}
              onChange={(e) => patch({ view_signatures: e.target.checked })}
            />
            {t.lowBandwidth}
          </label>
        </Row>
        <Row head={t.hoverLastPost}>
          <label>
            <input
              type="checkbox"
              checked={s.tt_last_post}
              onChange={(e) => patch({ tt_last_post: e.target.checked })}
            />
            {t.hoverNote}
          </label>
        </Row>
        <Row head={t.clickTopic}>
          <label>
            <input
              type="radio"
              name="click_topic"
              checked={s.click_topic === "firstpage"}
              onChange={() => patch({ click_topic: "firstpage" })}
            />
            {t.firstPage}
          </label>
          <label>
            <input
              type="radio"
              name="click_topic"
              checked={s.click_topic === "lastpage"}
              onChange={() => patch({ click_topic: "lastpage" })}
            />
            {t.lastPage}
          </label>
        </Row>
        <Row head={t.signature}>
          <textarea
            className="uc-textarea"
            rows={10}
            value={s.signature ?? ""}
            onChange={(e) => patch({ signature: e.target.value })}
          />
          <br />
          {t.signatureNote}
        </Row>
      </tbody>
    </table>
  );
}
