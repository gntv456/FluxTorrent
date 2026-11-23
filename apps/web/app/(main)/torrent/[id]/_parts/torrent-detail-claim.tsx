/**
 * 详情页头部的保种「认领」按钮（入口提级 2026-11-23）。
 * 认领此前只有保种区列表一个入口，资源库/详情页毫无通往认领的路，
 * 不点进保种区根本发现不了（用户来信「认领系统为什么没有」）。
 * 显隐双闸留在这里：保种模块开 + 种子在区才渲染；在区与否由
 * aggregate 个人段下发（尺子与保种区列表同一把，见
 * api `torrent_http/aggregate/aggregate_preserve.rs`）。
 */

import type { Dict } from "@/i18n/zh-CN";
import { ClaimButton } from "@/components/claim-button";

/** aggregate `detail.preserve`：仅 viewer 态下发；不在区（无保种行 /
 *  已移出 / 未过审）时后端省略此键。 */
export interface PreserveClaimState {
  in_zone: boolean;
  claimed_by_me: boolean;
  claimed: boolean;
  bonus_per_day: number;
}

export function TorrentDetailClaim({
  state,
  preserveOn,
  torrentId,
  dict,
}: {
  state: PreserveClaimState | undefined;
  /** 站点档案的保种模块开关（profile.modules.preserve） */
  preserveOn: boolean | undefined;
  torrentId: number;
  dict: Dict;
}) {
  if (!preserveOn || !state?.in_zone) return null;
  return (
    <ClaimButton
      torrentId={torrentId}
      claimed={state.claimed}
      label={dict.preserve.claimAction}
      claimedLabel={dict.preserve.claimedTag}
      bonus={state.bonus_per_day}
    />
  );
}
