-- 0165：pgcrypto 扩展补装。
-- repo/auth.rs 注册链路用 gen_random_bytes(20) 生成 passkey，但 0001 只装了
-- citext——干净库上注册直接 500（函数不存在）。幂等：已装环境无副作用。
CREATE EXTENSION IF NOT EXISTS pgcrypto;
