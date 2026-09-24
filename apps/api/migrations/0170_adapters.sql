-- 0170: 生态商店 M4 —— 适配器登记表（策划案 §5 / §8 M4）
--
-- 适配器 = 进程内 WASM 沙箱（wasmtime）跑的窄能力插件：元数据源/支付网关/
-- Indexer 三品类。策划案口径：
--   - 部署面恒定（不变式 1）：适配器是库表里的一行 wasm 字节，不改 docker compose；
--   - 宿主 API v1 仅四函数（http_fetch/cache_get/cache_set/secret/log 的 wasm 侧
--     绑定），出网白名单与限流在 manifest，宿主强制执行；
--   - 熔断（A4）：连续 trap/超时/越权 → 自动停用（enabled=false，strike 计数留痕）；
--   - 我方审核签发（R3）：本期无签名链，先以内置自营 + sysop 上传双轨过渡，
--     字段（checksum/signature）预留。

CREATE TABLE IF NOT EXISTS adapters (
    id            BIGSERIAL PRIMARY KEY,
    adapter_id    TEXT NOT NULL UNIQUE,       -- 如 metadata.douban
    kind          TEXT NOT NULL CHECK (kind IN ('metadata', 'payment', 'indexer')),
    name          TEXT NOT NULL,
    version       TEXT NOT NULL DEFAULT '1.0.0',
    core_compat   TEXT NOT NULL DEFAULT '*',
    -- 出网白名单（glob，宿主逐请求校验）：["https://*.douban.com/**"]
    http_allow    JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- 可读 secret 命名空间（需在 site_settings 中以 adapter_secret_<名> 存在）
    secrets_read  JSONB NOT NULL DEFAULT '[]'::jsonb,
    rate_limit_per_min INT NOT NULL DEFAULT 30,
    wasm          BYTEA NOT NULL,             -- 模块字节（≤ 2MiB 由端点校验）
    checksum      TEXT NOT NULL DEFAULT '',   -- sha256(wasm)，展示与审计用（签名链 M5+）
    enabled       BOOLEAN NOT NULL DEFAULT FALSE,
    -- 熔断：连续失败计数 + 最近错误摘要；成功清零；≥3 → enabled=false
    strikes       INT NOT NULL DEFAULT 0,
    last_error    TEXT NOT NULL DEFAULT '',
    installed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    installed_by  BIGINT REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_adapters_kind_enabled ON adapters (kind, enabled);
