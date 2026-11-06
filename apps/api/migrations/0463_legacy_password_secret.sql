-- 0463 旧站迁移兼容：NexusPHP 密码盐列
-- 好学站（www.hxpt.org）整体迁入：passhash 是双层 sha256(secret+sha256(pw))
-- 口径（include/takelogin.php:61），盐 users.secret 必须随迁才能验证。
-- 登录命中迁移口径后由 maybe_upgrade_bcrypt 透明升级 argon2id 并清盐。
ALTER TABLE users ADD COLUMN IF NOT EXISTS legacy_secret TEXT;
