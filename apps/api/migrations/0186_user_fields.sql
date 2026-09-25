-- 0186：用户自定义字段（一审 R4.1——建站系统对标 NP 生态的刚需）。
--
-- user_field_defs：站长定义的字段（key/标签/类型/必填/可见性/注册页展示/
--   受控选项集）；user_field_values：按用户的值（JSONB 存值，类型解释在 def 侧）。
-- 类型（type）：text / number / select / multiselect / date / bool。
-- 可见性（visibility）：public（公开档案展示）/ private（仅本人与管理组）。
-- 展示位（show_on_register）：注册页是否渲染（registration_mode 关闭时
--   字段仍可在 usercp 资料编辑页填写）。

CREATE TABLE IF NOT EXISTS user_field_defs (
    key            TEXT PRIMARY KEY,          -- 小写字母/数字/下划线 ≤40
    label          TEXT NOT NULL,             -- 显示名
    type           TEXT NOT NULL CHECK (type IN ('text','number','select','multiselect','date','bool')),
    required       BOOLEAN NOT NULL DEFAULT FALSE,
    visibility     TEXT NOT NULL DEFAULT 'public' CHECK (visibility IN ('public','private')),
    show_on_register BOOLEAN NOT NULL DEFAULT FALSE,
    options        JSONB NOT NULL DEFAULT '[]'::jsonb,  -- select/multiselect 的受控选项 [{value,label}]
    sort           INT NOT NULL DEFAULT 100,
    enabled        BOOLEAN NOT NULL DEFAULT TRUE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS user_field_values (
    user_id   BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    field_key TEXT NOT NULL REFERENCES user_field_defs(key) ON DELETE CASCADE,
    value     JSONB NOT NULL,                 -- text/number/date: 标量；multiselect: 数组；bool: 布尔
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, field_key)
);

-- 字典下拉数据化：usercp/admin 动态渲染用
CREATE INDEX IF NOT EXISTS idx_user_field_defs_sort ON user_field_defs (sort, key);
