-- M25 装扮中心（旧站 zhuangshi 口径）
-- 装扮 = 头像框/用户名样式/动态头像等可购买可佩戴的权益。
-- 商品本体复用 shop_items（kind），此表只记「拥有 + 佩戴」状态。
CREATE SEQUENCE IF NOT EXISTS user_dressups_id_seq;

CREATE TABLE IF NOT EXISTS user_dressups (
    id BIGINT PRIMARY KEY DEFAULT nextval('user_dressups_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    item_id BIGINT NOT NULL REFERENCES shop_items(id),
    source TEXT NOT NULL DEFAULT 'buy',       -- buy | gift | admin
    wearing BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, item_id)
);

-- 同槽位装扮同时只能佩戴一件（头像位/用户名位各一件）：
-- 互斥口径是 config->>'slot'（15 头像框与 17 动态头像同为 avatar 槽）。
CREATE OR REPLACE FUNCTION dressup_one_per_slot() RETURNS trigger AS $$
DECLARE
    new_slot TEXT;
BEGIN
    SELECT config->>'slot' INTO new_slot FROM shop_items WHERE id = NEW.item_id;
    IF new_slot IS NULL THEN
        RETURN NEW; -- 非装扮商品不校验
    END IF;
    IF NEW.wearing THEN
        PERFORM 1 FROM user_dressups ud
        JOIN shop_items si ON si.id = ud.item_id
        WHERE ud.user_id = NEW.user_id
          AND ud.id <> NEW.id
          AND ud.wearing
          AND si.config->>'slot' = new_slot;
        IF FOUND THEN
            RAISE EXCEPTION 'duplicate wearing for slot %', new_slot;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS dressup_one_per_kind_trg ON user_dressups;
DROP FUNCTION IF EXISTS dressup_one_per_kind();
DROP TRIGGER IF EXISTS dressup_one_per_slot_trg ON user_dressups;
CREATE TRIGGER dressup_one_per_slot_trg
    BEFORE INSERT OR UPDATE OF wearing ON user_dressups
    FOR EACH ROW EXECUTE FUNCTION dressup_one_per_slot();

-- 装扮类商品标记：给现有四件装扮商品的 config 加 slot 说明
UPDATE shop_items SET config = config || '{"dressup": true, "slot": "avatar"}'::jsonb
    WHERE kind IN ('avatar_frame', 'animated_avatar');
UPDATE shop_items SET config = config || '{"dressup": true, "slot": "username"}'::jsonb
    WHERE kind IN ('rainbow_id', 'rainbow_name');
