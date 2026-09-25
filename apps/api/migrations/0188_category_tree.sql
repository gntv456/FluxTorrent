-- 0188：分类层级（一审 R4.3）——categories 平表加 parent_id 树形。
--
-- 现状是平表无层级；站长要「电影 > BluRay/Remux」两级分类时无处表达。
-- 设计：parent_id NULL = 顶级；种子挂叶子（也可挂父级，列表筛选级联包含）；
-- 列表筛选 category_id 命中父级时递归包含子孙（后端查询侧展开，不改表结构）。
-- 防环：parent_id 不允许指向自身/后代（写入端点校验 + 触发器兜底）。

ALTER TABLE categories ADD COLUMN IF NOT EXISTS parent_id INT
    REFERENCES categories(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_categories_parent ON categories (parent_id);

-- 防环触发器：parent 链最长 8 层（防误指成环把查询挂死）
CREATE OR REPLACE FUNCTION categories_guard_parent() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    p INT;
    depth INT := 0;
BEGIN
    IF NEW.parent_id IS NULL THEN RETURN NEW; END IF;
    IF NEW.parent_id = NEW.id THEN
        RAISE EXCEPTION '分类不能以自身为父级';
    END IF;
    p := NEW.parent_id;
    WHILE p IS NOT NULL AND depth < 8 LOOP
        IF p = NEW.id THEN
            RAISE EXCEPTION '分类层级成环';
        END IF;
        SELECT parent_id INTO p FROM categories WHERE id = p;
        depth := depth + 1;
    END LOOP;
    IF depth >= 8 THEN
        RAISE EXCEPTION '分类层级超过 8 层上限';
    END IF;
    RETURN NEW;
END $$;

DROP TRIGGER IF EXISTS trg_categories_parent ON categories;
CREATE TRIGGER trg_categories_parent
    BEFORE INSERT OR UPDATE OF parent_id ON categories
    FOR EACH ROW EXECUTE FUNCTION categories_guard_parent();
