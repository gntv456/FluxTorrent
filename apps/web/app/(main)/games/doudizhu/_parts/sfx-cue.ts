import type { SfxCue } from "@/lib/sfx";

/** 按对局结果与动作选音效线索：终局按输赢；王炸/炸弹按牌型（PascalCase
 *  序列化，toLowerCase 归一）；过牌 pass、叫分 deal、出牌 pop。 */
export function dzCueOf(
  done: boolean,
  win: boolean,
  action: string,
  passed: boolean,
  lastKind?: string,
): SfxCue {
  if (done) return win ? "win" : "lose";
  const kind = lastKind?.toLowerCase();
  if (kind === "rocket") return "rocket";
  if (kind === "bomb") return "bomb";
  if (passed) return "pass";
  return action === "play" ? "pop" : "deal";
}
