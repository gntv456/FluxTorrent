"use client";

import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import type { UserSettings } from "@/components/usercp";
import { BANDWIDTH, COUNTRIES, ISPS } from "./usercp-personal-shared";

/** 个人资料面板（从 usercp.tsx 按域拆出，300 行门禁）：
 *  停用账户 / 私信 / 性别 / 国家 / 带宽 ISP / 头像 / 自我介绍。
 *  带宽/ISP/国家选项表拆至 ./usercp-personal-shared.ts。 */

export function PersonalTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.usercp.personal;
  return (
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.parked}>
          <label>
            <input
              type="checkbox"
              checked={s.parked}
              onChange={(e) => patch({ parked: e.target.checked })}
            />
            {t.parkedLabel}
          </label>
          <br />
          <span className="uc-note">
            <b>{dict.usercp.note}</b>
            {t.parkedNote}
          </span>
        </Row>
        <Row head={t.pm}>
          {t.pmAccept}
          <label>
            <input
              type="radio"
              name="accept_pm"
              checked={s.accept_pm === "yes"}
              onChange={() => patch({ accept_pm: "yes" })}
            />
            {t.pmAll}
          </label>
          <label>
            <input
              type="radio"
              name="accept_pm"
              checked={s.accept_pm === "friends"}
              onChange={() => patch({ accept_pm: "friends" })}
            />
            {t.pmFriends}
          </label>
          <label>
            <input
              type="radio"
              name="accept_pm"
              checked={s.accept_pm === "no"}
              onChange={() => patch({ accept_pm: "no" })}
            />
            {t.pmStaff}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.delete_pm}
              onChange={(e) => patch({ delete_pm: e.target.checked })}
            />{" "}
            {t.pmDelete}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.save_pm}
              onChange={(e) => patch({ save_pm: e.target.checked })}
            />{" "}
            {t.pmSave}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.comment_pm}
              onChange={(e) => patch({ comment_pm: e.target.checked })}
            />{" "}
            {t.pmComment}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.notify_topic_reply}
              onChange={(e) => patch({ notify_topic_reply: e.target.checked })}
            />{" "}
            {t.pmTopicReply}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.notify_hr}
              onChange={(e) => patch({ notify_hr: e.target.checked })}
            />{" "}
            {t.pmHr}
          </label>
        </Row>
        <Row head={t.gender}>
          <label>
            <input
              type="radio"
              name="gender"
              checked={s.gender === 0}
              onChange={() => patch({ gender: 0 })}
            />
            {t.genderNA}
          </label>
          <label>
            <input
              type="radio"
              name="gender"
              checked={s.gender === 1}
              onChange={() => patch({ gender: 1 })}
            />
            {t.genderMale}
          </label>
          <label>
            <input
              type="radio"
              name="gender"
              checked={s.gender === 2}
              onChange={() => patch({ gender: 2 })}
            />
            {t.genderFemale}
          </label>
        </Row>
        <Row head={t.trackerUrl}>
          <select value={s.country >= 0 ? "0" : "0"} onChange={() => undefined}>
            <option value="0">---- {t.notSelected} ----</option>
            <option value="1">{t.trackerAnnounceFallback}</option>
          </select>
          <br />
          <span className="uc-note">
            <b>{dict.usercp.note}</b>
            {t.trackerNote}
          </span>
        </Row>
        <Row head={t.country}>
          <select
            value={String(s.country)}
            onChange={(e) => patch({ country: Number(e.target.value) })}
          >
            <option value="0">---- {t.notSelected} ----</option>
            {COUNTRIES.map((c, i) => (
              <option key={c} value={i + 1}>
                {c}
              </option>
            ))}
          </select>
        </Row>
        <Row head={t.bandwidth}>
          <b>{t.downBand}</b>:{" "}
          <select
            value={String(s.download_speed)}
            onChange={(e) => patch({ download_speed: Number(e.target.value) })}
          >
            <option value="0">---- {t.notSelected} ----</option>
            {BANDWIDTH.map((b, i) => (
              <option key={b} value={i + 1}>
                {b}
              </option>
            ))}
          </select>{" "}
          <b>{t.upBand}</b>:{" "}
          <select
            value={String(s.upload_speed)}
            onChange={(e) => patch({ upload_speed: Number(e.target.value) })}
          >
            <option value="0">---- {t.notSelected} ----</option>
            {BANDWIDTH.map((b, i) => (
              <option key={b} value={i + 1}>
                {b}
              </option>
            ))}
          </select>{" "}
          <b>{t.isp}</b>:{" "}
          <select
            value={String(s.isp)}
            onChange={(e) => patch({ isp: Number(e.target.value) })}
          >
            <option value="0">---- {t.notSelected} ----</option>
            {ISPS.map((c, i) => (
              <option key={c} value={i === 6 ? 20 : i + 1}>
                {c}
              </option>
            ))}
          </select>
        </Row>
        <Row head={t.avatar}>
          {s.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={s.avatar_url} alt="" className="uc-avatar-preview" />
          ) : (
            <span className="uc-avatar-preview uc-avatar-preview--empty" />
          )}
          <br />
          <input
            type="text"
            className="uc-input-wide"
            placeholder="https://"
            value={s.avatar_url ?? ""}
            onChange={(e) => patch({ avatar_url: e.target.value })}
          />
          <br />
          {t.avatarNote}
        </Row>
        <Row head={t.info}>
          <textarea
            className="uc-textarea"
            rows={10}
            value={s.info ?? ""}
            onChange={(e) => patch({ info: e.target.value })}
          />
          <br />
          {t.infoNote}
        </Row>
      </tbody>
    </table>
  );
}
