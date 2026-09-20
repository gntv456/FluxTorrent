import { FlatCompat } from "@eslint/eslintrc";

/**
 * ESLint 9 扁平配置（桥接 eslint-config-next 的 legacy 形态）。
 *
 * 演进说明：`next lint` 已废弃（Next 16 移除），改为直接跑 ESLint CLI；
 * eslint-config-next@15 仍是 eslintrc 形态，用官方 @eslint/eslintrc 的
 * FlatCompat 转接。规则基线 = next/core-web-vitals（next build 内置检查的超集）。
 * 存量告警不阻断 CI：warning 照报、error 才挂（与 cargo clippy「信息性」门禁
 * 同一演进策略——先可视化存量，再逐步收紧为 -W error）。
 */
const compat = new FlatCompat({ baseDirectory: import.meta.dirname });

const config = [
  ...compat.extends("next/core-web-vitals"),
  {
    ignores: [".next/**", "node_modules/**", "next-env.d.ts"],
  },
];

export default config;
