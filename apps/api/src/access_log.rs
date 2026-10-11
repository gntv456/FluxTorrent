//! 访问日志（P2 防凭据泄漏）。从 main.rs 按域拆出（300 行门禁）。
//!
//! actix 默认格式里的 `%r` 含完整 query——compat 下载走 `?passkey=`、
//! 凭证下载走 `?token=`、开放 API 走 `?apikey=`，那等价把长期/短期凭据
//! 写进 access log。故这里把 `%r` 换成 `%m %U`（method + **不含 query** 的
//! path），其余字段与 default 对齐。
//!
//! 二轮审计（中）：`/rss/{passkey}` 的 passkey 在**路径**里，`%U` 挡不住
//! 路径形态 ⇒ 整个 RSS 前缀不落 access log（RSS 自身有 30/min/passkey
//! 限流，不记日志不影响排障粒度）。
//!
//! P-66（批三十实测）：这一条原本写成 `exclude("/api/v1/rss")`，而 actix 的
//! exclude 是**精确匹配**（要带 `*` 才当前缀），于是 `/rss/{passkey}` 一条都
//! 没被排除——实测 access log 与 request_id 的「请求开始」行都落了整串
//! passkey（passkey 是长期凭据：拿到即可下种、订阅 RSS、替该账号 announce）。
//! 通配补上；第二条通道由 `request_id::log_safe_path` 按形态脱敏，
//! 两条都得堵，只堵一条等于没堵。

use actix_web::middleware::Logger;

pub fn build() -> Logger {
    Logger::new("%a \"%m %U\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T")
        .exclude("/api/v1/health")
        .exclude("/api/v1/rss*")
}
