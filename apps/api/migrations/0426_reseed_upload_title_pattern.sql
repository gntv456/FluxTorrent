-- 0426: 补种 `upload_title_pattern`（0419 想补、但**没补上**的那一把）。
--
-- 为什么要有第二个迁移补同一把键：sqlx 只按**版本号**记账，`_sqlx_migrations`
-- 里已有的号下次启动直接跳过、不看文件内容。0419 的台账时刻是
-- `installed_on = 2026-10-25 02:58`，而它进仓库是 `2026-10-26 07:10`
-- （505f853a「补 add 漏交的功能源文件——8 迁移」）⇒ 跑的时候是工作树里的一份
-- 草稿，之后往同一文件里追加的内容对**已应用过的站点永远不会再执行**。
-- 现网实测正是这个形状：0419 一次补四把键，另外三把
-- （upload_auto_approve_class / torrent_edit_class / torrent_delete_class）
-- 都在 site_settings + settings_meta 里，只有 upload_title_pattern 两表皆无。
--
-- 排除了「被谁删了」：全仓除迁移外没有 DELETE FROM site_settings 的代码路径；
-- `settings_meta_name_fkey` 是 ON DELETE CASCADE（真删值行会连带删元数据行，
-- 两表同缺确实符合它的形状），但同为空串的 accountpassword / captcha_secret /
-- tg_bot_token / tmdb_api_key 至今健在 ⇒ 不存在「清理空值设定」的自动动作。
-- 也就是说 0419 的结论「判定为建库初期的库外操作所致」对本键不成立——
-- 键从来没被插进来过。
--
-- 后果（与 0419 头注同一条，只是仍然没修好）：META_SELECT 是
-- site_settings JOIN settings_meta 的内连接 ⇒ 两表缺行 = 设置页根本不渲染
-- 「发种命名规范（正则）」这张卡，站长无从填写；`upload_precheck.rs:233`
-- 读不到就回落空串（不校验），于是「命名规范」这个发布闸门在存量站上
-- 结构性失效，而且界面上看不出任何异常。
--
-- 幂等与顺序照 0412/0419：先值行后元数据（name 有外键指向 site_settings），
-- 两侧都 ON CONFLICT DO NOTHING；默认值与代码回落值一致（空＝不校验），
-- 不改变任何现有行为，只是把这张卡还给设置页。card_order 84 在 grp=torrent
-- 卡片里实测空位（82/83/85 在用）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('upload_title_pattern', '',
 '发种名称需匹配的 Java 正则（命名规范），留空即不校验。'
 '例：^[^.]+\.[0-9]{4}\.(1080p|2160p|720p) 只放行「片名.年份.分辨率」格式。',
 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key,
                           card_order, options) VALUES
('upload_title_pattern', 'text', '发种命名规范（正则）',
 'Title pattern (regex)',
 '留空不校验；填正则表达式，种子名不匹配即拒。保存这一刻就会编译，写坏当场拒——'
 '否则是「会员每次发种都被挡住、站长界面一切正常」。本站引擎是 Rust regex：'
 '不支持后查 (?<=…) 与反向引用 \1，反斜杠按字面写一条（示例里的 \d 就是一个反斜杠）。',
 'torrent', 84, '{"rule": "regex"}'::jsonb)
ON CONFLICT (name) DO NOTHING;

-- 同族文案修正（照 0345 的先例：它修的就是这条说明里的双写反斜杠）：
-- 0286/0419 都写着「Java 正则」，而 upload_precheck.rs 用的是 `regex` crate。
-- 按 Java 写法填的站长会写出 (?<=…) / \1，保存时无感、发种时全站被
-- 「站点命名正则非法，请联系站长」拦住。新库走上面的 INSERT，存量行这里 UPDATE。
UPDATE settings_meta
   SET hint = '留空不校验；填正则表达式，种子名不匹配即拒。本站引擎是 Rust regex：'
              || '不支持后查 (?<=…) 与反向引用 \1，反斜杠按字面写一条（示例里的 \d 就是一个反斜杠）。',
       options = '{"rule": "regex"}'::jsonb
 WHERE name = 'upload_title_pattern'
   AND (hint LIKE '%Java%' OR options IS DISTINCT FROM '{"rule": "regex"}'::jsonb);

UPDATE site_settings
   SET descr = replace(descr, 'Java 正则', '正则表达式（Rust regex 语法，'
                                          || '不支持后查与反向引用）')
 WHERE name = 'upload_title_pattern' AND descr LIKE '%Java 正则%';

-- 同批：`resurrection_hours`（死种复活任务所需做种时长）。三处读取方
-- （ops_http/resurrections.rs 列表与领取、social_http/team.rs 组队入口）都按
-- `COALESCE(..., 240)` 回落，但两表从来没有登记过这把键 ⇒ 界面上根本没有
-- 这一格，「可调」只存在于注释里，全站恒 240 小时。
-- 10-27 之前的现网形状是「有人手工插过一行、值被闸门探针改成 1」，闸门收尾
-- 把这行残留删掉后才露出「键本身没登记」这一层——所以本轮连带补登记。
-- 默认值与代码回落值一致（240），不改变任何现有行为。
-- 领取时会把 required_hours 快照进 resurrections 行，改这里只影响**之后**
-- 新领的任务，这一点写进 hint，免得站长以为改了会追认在途任务。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('resurrection_hours', '240',
 '复活一个死种需要累计做满的小时数（领取时快照，改动只影响之后新领的任务）',
 'ops')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max,
                           group_key, card_order) VALUES
('resurrection_hours', 'number', '死种复活任务做种时长',
 'Resurrection seeding hours',
 '会员领取「复活死种」任务后需累计做满的小时数；领取瞬间快照，改这里不追认'
 '在途任务。三处读取方（死种列表 / 领取 / 组队入口）缺行时一律回落 240。',
 '小时', 1, 2000, 'ops', 64)
ON CONFLICT (name) DO NOTHING;
