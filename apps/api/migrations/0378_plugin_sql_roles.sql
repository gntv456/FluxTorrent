-- 0378 插件 SQL 隔离（PT Studio 对标的 CVE 防线，本批落 P0 安全前置）
--
-- 现状（0367 批已做）：native 插件不再持 `&AppState`，只拿 `PluginCtx`，
-- DB 访问一律过 `sql_guard` 结构校验（表/列/读写模式白名单、拒多语句、
-- 拒 DDL、拒 SET/GRANT、拒 SELECT *）。这挡住了**误用**。
-- 但它挡不住**闸门漏判之后的后果**：sql_guard 是应用层字符串校验器，
-- 一旦它有漏判（解析器版本差异、编码技巧、未来新语句类型），
-- 插件手里的连接仍是站点主账号，能读 users.pass_hash、能改 torrents 任意列。
--
-- PT Studio 对此的防线在**数据库层**：每插件独立 role + 每次调用
-- `SET LOCAL ROLE` + `SET LOCAL search_path` 锁 pg_catalog。
-- 本迁移落这套基础设施。
--
-- 【重要：降级设计】
-- CREATE ROLE 需要 CREATEROLE 权限。自部署场景里站点连接常常就是表属主，
-- 没有 CREATEROLE。因此本迁移**不硬失败**：权限不足时建不出角色，
-- `flux_plugin_ensure_role` 返回 NULL，运行期自动退回「纯 sql_guard」
-- 模式（0367 的既有强度不变），并在日志里记一次告警。
-- 也就是说：DB 层隔离是**纵深防御的加成**，不是新的单点依赖——
-- 权限够就多一道，权限不够也不比现在更差。
--
-- 为什么不用 pgcrypto 的 digest()：那是扩展依赖，自部署未必装了。
-- 内置 md5() 够用——它只做命名去重，不是安全用途（安全由白名单保底）。
--
-- 纪律：
--   * 全部幂等（DO $$ … IF NOT EXISTS … $$）；
--   * 角色 NOINHERIT / NOLOGIN / 无管理权；
--   * 表名与角色名都过标识符白名单，绝不直接拼插件名。

-- ① 插件名 → 角色名。严格白名单化 + 截断 + md5 后缀防碰撞。
--    角色名会进 `SET ROLE`，是 SQL 标识符注入面，绝不能直接拼插件名。
CREATE OR REPLACE FUNCTION flux_plugin_role_name(p_name TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
AS $$
    SELECT 'flux_plugin_'
        || left(
             regexp_replace(lower(coalesce(p_name, '')), '[^a-z0-9_]', '_', 'g'),
             40)
        || '_'
        || substr(md5(coalesce(p_name, '')), 1, 8);
$$;

COMMENT ON FUNCTION flux_plugin_role_name(TEXT) IS
    '插件名 → PostgreSQL 角色名。角色名会进 SET ROLE（SQL 标识符注入面），故只留 [a-z0-9_]、截断 40，并附 md5 前 8 位防截断碰撞。md5 仅用于命名去重，不作安全用途。';

-- ② 按需建角色（幂等）。权限不足 → 返回 NULL（调用方降级，不抛错）。
CREATE OR REPLACE FUNCTION flux_plugin_ensure_role(p_name TEXT)
RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    v_role TEXT := flux_plugin_role_name(p_name);
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = v_role) THEN
        BEGIN
            EXECUTE format(
                'CREATE ROLE %I NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB '
                'NOCREATEROLE NOREPLICATION',
                v_role);
        EXCEPTION WHEN insufficient_privilege THEN
            -- 自部署常见：站点连接无 CREATEROLE。降级而非失败。
            RAISE NOTICE
                'flux_plugin_ensure_role: 无 CREATEROLE 权限，插件 % 的 DB '
                '角色隔离降级为纯 sql_guard 模式（0367 强度不变）', p_name;
            RETURN NULL;
        END;
    END IF;
    RETURN v_role;
END;
$$;

COMMENT ON FUNCTION flux_plugin_ensure_role(TEXT) IS
    '幂等建插件角色（NOLOGIN+NOINHERIT+无管理权）。无 CREATEROLE 时返回 NULL 让调用方降级——DB 层隔离是纵深防御加成，不该成为新的单点故障。';

-- ③ 表级授权（幂等）。sensitive 能力走这里显式开口子。
--    DB 层给表级、列级留给 sql_guard：两套列清单双份维护，且插件新增列
--    时 DB 层会静默失权（比误放行更难发现）。表级 + 应用层列级互补。
CREATE OR REPLACE FUNCTION flux_plugin_grant(
    p_name   TEXT,
    p_tables TEXT[],
    p_mode   TEXT DEFAULT 'read'
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    v_role TEXT := flux_plugin_ensure_role(p_name);
    t       TEXT;
    v_priv  TEXT;
BEGIN
    IF p_mode NOT IN ('read', 'write') THEN
        RAISE EXCEPTION 'flux_plugin_grant: mode 须为 read/write，收到 %', p_mode;
    END IF;
    IF v_role IS NULL THEN
        RETURN;   -- 降级：无角色可授，调用方照常退回 sql_guard 模式
    END IF;
    v_priv := CASE WHEN p_mode = 'write' THEN 'SELECT, INSERT, UPDATE, DELETE'
                   ELSE 'SELECT' END;
    FOREACH t IN ARRAY p_tables LOOP
        -- 表名白名单，防 GRANT 注入
        IF t !~ '^[a-z_][a-z0-9_]*$' THEN
            RAISE EXCEPTION 'flux_plugin_grant: 非法表名 %', t;
        END IF;
        EXECUTE format('GRANT %s ON TABLE %I TO %I', v_priv, t, v_role);
    END LOOP;

    INSERT INTO plugin_sql_roles (plugin, role_name, tables, mode, updated_at)
    VALUES (p_name, v_role, p_tables, p_mode, now())
    ON CONFLICT (plugin) DO UPDATE
      SET role_name = EXCLUDED.role_name,
          tables     = EXCLUDED.tables,
          mode       = EXCLUDED.mode,
          updated_at = now();
END;
$$;

COMMENT ON FUNCTION flux_plugin_grant(TEXT, TEXT[], TEXT) IS
    '按声明表给插件角色授表级权限，并同步 plugin_sql_roles 清单。表名过 ^[a-z_][a-z0-9_]*$ 白名单防 GRANT 注入。幂等；无角色时静默跳过（降级）。';

-- ④ 撤权（插件下线/能力收窄时用；幂等）
CREATE OR REPLACE FUNCTION flux_plugin_revoke(p_name TEXT)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    v_role TEXT := flux_plugin_role_name(p_name);
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = v_role) THEN
        EXECUTE format(
            'REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA public FROM %I',
            v_role);
    END IF;
    DELETE FROM plugin_sql_roles WHERE plugin = p_name;
END;
$$;

COMMENT ON FUNCTION flux_plugin_revoke(TEXT) IS
    '撤销插件角色的表权限并从清单摘除。插件下线或 DbCapability 收窄时调用。幂等。';

-- ⑤ 角色清单：谁被切成了哪个角色、能碰哪几张表（DB 侧可查可审计）
CREATE TABLE IF NOT EXISTS plugin_sql_roles (
    plugin      TEXT        PRIMARY KEY,
    role_name   TEXT        NOT NULL,
    tables      TEXT[]      NOT NULL DEFAULT '{}',
    mode        TEXT        NOT NULL DEFAULT 'read'
                            CHECK (mode IN ('read', 'write')),
    granted_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

COMMENT ON TABLE plugin_sql_roles IS
    '插件 → DB 角色映射（0378）。flux_plugin_grant 同步维护，让「谁能读写哪张表」在 DB 侧可查、可审计、可与代码里的 DbCapability 声明对拍。';
