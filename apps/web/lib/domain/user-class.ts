// 用户等级域常量（硬编码审计 P1 收敛单源）：
// 此前 `class_id >= 90` 散落 4 处业务判断 + 3 处默认值，后端调门槛时
// 前端口径不会跟（要么多吃 403，要么藏起合法入口）。
// 90 的语义 = 「管理组」（staff）的等级下限，与后端
// apps/api classes_admin.rs 的 STAFF_CLASS_FLOOR 同口径——改等级体系
// 时两端一起动。
export const STAFF_CLASS_MIN = 90;

/** 是否管理组（staff）——IP 完整可见、管理入口显隐等业务判断统一走这里 */
export function isStaffClass(classId: number | null | undefined): boolean {
  return (classId ?? 0) >= STAFF_CLASS_MIN;
}
