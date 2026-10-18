"use client";

/** 注册页·四项基础字段（2026-10-18 从 register/page.tsx 按域拆出，300 行
 *  门禁）：邀请码 / 用户名 / 邮箱 / 密码。错误按码映射到字段内联渲染，
 *  提示文案取 dict.register（邀请码在开放注册下可空）。 */

import { INPUT_LG } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { FormField } from "@/components/ui/form-field";

export interface RegisterBasicValues {
  inviteCode: string;
  username: string;
  email: string;
  password: string;
}

export function RegisterBasicFields({
  values,
  onChange,
  first,
  openMode,
}: {
  values: RegisterBasicValues;
  /** 字段级更新（只写变化的键） */
  onChange: (patch: Partial<RegisterBasicValues>) => void;
  /** 取某字段的首条错误 */
  first: (field: string) => string | undefined;
  /** 开放注册：邀请码非必填 */
  openMode: boolean;
}) {
  const { dict } = useI18n();
  return (
    <>
      <FormField
        name="invite_code"
        label={dict.register.inviteCode}
        required={!openMode}
        error={first("invite_code")}
        hint={openMode ? dict.register.inviteOptional : undefined}
      >
        <input
          value={values.inviteCode}
          onChange={(e) => onChange({ inviteCode: e.target.value })}
          required={!openMode}
          maxLength={32}
          className={INPUT_LG}
        />
      </FormField>
      <FormField
        name="username"
        label={dict.register.username}
        required
        error={first("username")}
        hint={dict.register.usernameHint}
      >
        <input
          value={values.username}
          onChange={(e) => onChange({ username: e.target.value })}
          autoComplete="username"
          required
          minLength={3}
          maxLength={24}
          className={INPUT_LG}
        />
      </FormField>
      <FormField
        name="email"
        label={dict.register.email}
        required
        error={first("email")}
        hint={
          values.inviteCode ? dict.register.emailMustMatch : undefined
        }
      >
        <input
          type="email"
          value={values.email}
          onChange={(e) => onChange({ email: e.target.value })}
          autoComplete="email"
          required
          className={INPUT_LG}
        />
      </FormField>
      <FormField
        name="password"
        label={dict.register.password}
        required
        error={first("password")}
        hint={dict.register.passwordHint}
      >
        <input
          type="password"
          value={values.password}
          onChange={(e) => onChange({ password: e.target.value })}
          autoComplete="new-password"
          required
          minLength={8}
          className={INPUT_LG}
        />
      </FormField>
    </>
  );
}
