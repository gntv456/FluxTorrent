//! 访问日志（P2 防凭据泄漏）。从 main.rs 按域拆出（300 行门禁）。
//!
//! actix 默认格式里的 `%r` 含完整 query——compat 下载走 `?passkey=`、
//! 凭证下载走 `?token=`、开放 API 走 `?apikey=`，那等价把长期/短期凭据
//! 写进 access log。故这里把 `%r` 换成「method + **不含 query** 的 path」
//! （`%{method}xi %U`），其余字段与 default 对齐。
//!
//! 二轮审计（中）：`/rss/{passkey}` 的 passkey 在**路径**里，`%U` 挡不住
//! 路径形态 ⇒ 整个 RSS 前缀不落 access log（RSS 自身有 30/min/passkey
//! 限流，不记日志不影响排障粒度）。
//!
//! P-66（批三十，两轮实测）：这一条原本写 `exclude("/api/v1/rss")`。actix 的
//! exclude 是 HashSet 精确匹配（logger.rs:322 `exclude.contains(req.path())`），
//! 不存在通配语义 ⇒ 带路径参数的 RSS 一条都没被排除，实测 access log 与
//! request_id 的「请求开始」行都落了整串 passkey（长期凭据：拿到即可下种、
//! 订阅 RSS、替该账号 announce；日志的读者=运维/日志聚合/任何能
//! `docker logs` 的人都持有全体订阅者凭据）。
//! 现在 exclude_regex 挡路径形态，request_id 侧由 `log_safe_path` 按形态脱敏
//! 兜第二条通道——两条都得堵，只堵一条等于没堵。
//!
//! P-74（本批实测）：上面那句「换成 method + path」原本是写 `%m %U`，而
//! actix 4.15 的 specifier 表里**没有 `%m`**（logger.rs:525-536 只认
//! `% a t r s b U T D` 与 `%{x}i/o/e`、`%{x}xi/xo`，其余走
//! `_ => FormatText::Str(...)`）。于是它既不报错也不警告，把 `%m` 原样印成
//! 字面量 `m`——线上每行都是 `172.20.0.1 "m /api/v1/auth/login" 500 …`，
//! **HTTP 方法整列静默消失**（谁 POST 发种、谁 PUT 改设置，日志里分不出来）。
//! 教训：日志格式的判据要钉在**渲染出来的那一行**上，不能只验「有没有泄漏」
//! ——同一行里「泄漏挡住了」和「字段丢了」是两个独立方向，只验前者会把
//! 后者当成功通过。现在 method 用 `%{method}xi` 自定义段补回，
//! 闸门 AU S1 同时断言「不含 passkey」与「含方法」。

use actix_web::middleware::Logger;

pub fn build() -> Logger {
    Logger::new(
        "%a \"%{method}xi %U\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T",
    )
    // actix 没有 method 格式符，只能用自定义请求段补上（P-74）。
    .custom_request_replace("method", |req| req.method().to_string())
    .exclude("/api/v1/health")
    // actix 的 exclude 是 **HashSet 精确匹配**（actix-web-4.15.0
    // src/middleware/logger.rs:322 `exclude.contains(req.path())`），
    // 文档里没有通配这回事：`/api/v1/rss` 与 `/api/v1/rss*` 都一条没挡
    // （后者本批先写上去，实测 logger 行仍带整串 passkey）。
    // 带路径参数的出口只能用 exclude_regex。
    .exclude_regex("^/api/v1/rss")
}
