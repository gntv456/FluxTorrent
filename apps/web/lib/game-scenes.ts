/**
 * 玩法 → 机台画面映射（单一来源）：大厅卡面与快捷入口共用。
 * scene 组件本体在 components/game/scene-a|b|c.tsx（行数门禁拆分）。
 */
import type { ComponentType } from "react";
import {
  SceneBigsmall,
  SceneScratch,
  SceneJgg,
} from "@/components/game/scene-a";
import {
  SceneGacha,
  SceneFarm,
  ScenePet,
  SceneFishing,
} from "@/components/game/scene-b";
import { SceneCapsule, SceneWheel } from "@/components/game/scene-c";

export const SCENES: Record<string, ComponentType<{ className?: string }>> = {
  bigsmall: SceneBigsmall,
  scratch: SceneScratch,
  jgg: SceneJgg,
  capsule: SceneCapsule,
  wheel: SceneWheel,
  gacha: SceneGacha,
  farm: SceneFarm,
  pet: ScenePet,
  fishing: SceneFishing,
};
