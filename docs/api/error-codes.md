# API 错误码注册表（生成物，勿手改）

> 由 `node scripts/error_codes_gen.mjs --update` 生成（W-13）：
> apps/api/src/errors.rs（变体→码→触发条件注释 + HTTP 态）、
> apps/api/src/i18n.rs（三语文案）。CI 跑 `--check`：源码改动而未刷新 → 红灯。
> 码集对拍（与 index.ts 手写 ErrorCode）见 check_type_drift.mjs 第 4 维。

- 变体 23 个 → 错误码 22 个；i18n 文案 24 条（缺 0 见文末待补清单）

| 码 | 变体 | 触发条件 | zh-CN | zh-TW | en | HTTP |
|----:|------|----------|-------|-------|----|------|
| 1000 | Internal | 未分类的服务端故障（含 db_to_domain 兜底的数据库错误） | 内部错误 | 內部錯誤 | Internal error | 500 INTERNAL_SERVER_ERROR |
| 1002 | FieldErrors | 字段校验不通过或外键引用了不存在的 id（db_to_domain 23503）<br>站点设定批量保存预校验多字段不通过（§4.1） | 参数校验失败 | 參數校驗失敗 | Validation failed | 400 BAD_REQUEST |
| 1002 | Validation | 字段校验不通过或外键引用了不存在的 id（db_to_domain 23503）<br>站点设定批量保存预校验多字段不通过（§4.1） | 参数校验失败 | 參數校驗失敗 | Validation failed | 400 BAD_REQUEST |
| 1004 | NotFound | 按 id 查资源（种子/评论/工单/订单…）无行命中 | 资源不存在 | 資源不存在 | Resource not found | 404 NOT_FOUND |
| 1015 | RateLimited | 命中限流桶（openapi 登录/下载 passkey，60s 窗口） | 操作太频繁啦，休息一下 | 操作太頻繁啦，休息一下 | Too many requests, take a break | 429 TOO_MANY_REQUESTS |
| 2001 | Unauthorized | JWT 缺失/过期/签名不对，或匿名访问需登录端点 | 未认证 | 未認證 | Not authenticated | 401 UNAUTHORIZED |
| 2003 | Forbidden | 已认证但等级/权限不足（先过模块开关再查权限，§5.5） | 无权限 | 無權限 | Permission denied | 403 FORBIDDEN |
| 2004 | InvalidCredentials | 登录用户名或密码不匹配 | 凭证无效 | 憑證無效 | Invalid credentials | 400 BAD_REQUEST |
| 2005 | InviteInvalid | 注册邀请码不存在或已过有效期 | 邀请码无效或已过期 | 邀請碼無效或已過期 | Invite code is invalid or expired | 400 BAD_REQUEST |
| 2006 | InviteUsed | 注册邀请码已被消费（一码一用） | 邀请码已被使用 | 邀請碼已被使用 | Invite code already used | 400 BAD_REQUEST |
| 2007 | UsernameTaken | 注册/改名的用户名与他人冲突 | 用户名已被占用 | 用戶名已被佔用 | Username already taken | 400 BAD_REQUEST |
| 2010 | TwoFactorRequired | 密码已对但未提交 2FA 验证码（登录第二步） | 账号已开启两步验证，请填写动态验证码 | 帳號已開啟兩步驗證，請填寫動態驗證碼 | This account has 2FA enabled; enter the 6-digit code | 400 BAD_REQUEST |
| 2011 | TwoFactorInvalid | 已提交 2FA 验证码但校验不匹配 | 两步验证码不正确，请核对验证器当前 6 位数字 | 兩步驗證碼不正確，請核對驗證器當前 6 位數字 | Incorrect 2FA code; check the current 6 digits | 400 BAD_REQUEST |
| 2012 | PowRequired | 登录失败达到触发线，缺令牌或令牌失效应重新取题 | 登录尝试较多，需先完成工作量证明 | 登入嘗試較多，需先完成工作量證明 | Too many attempts; solve the proof-of-work first | 400 BAD_REQUEST |
| 3003 | TorrentInvalid | 种子文件解析失败或不合规（缺 info_hash 等） | 种子文件无效 | 種子檔案無效 | Invalid torrent file | 400 BAD_REQUEST |
| 3004 | TorrentDuplicate | info_hash 与库中已有种子重复 | 种子重复 | 種子重複 | Duplicate torrent | 400 BAD_REQUEST |
| 4001 | InsufficientSpark | 经济操作（购买/抽卡/打赏…）火花余额不足 | 余额不足 | 餘額不足 | Insufficient balance | 409 CONFLICT |
| 4002 | LedgerConflict | spark_ledger 乐观并发版本冲突，客户端应重试 | 流水冲突，请重试 | 流水衝突，請重試 | Ledger conflict, please retry | 409 CONFLICT |
| 4101 | ModuleDisabled | 站点关闭了该功能模块（先模块后权限，§5.5） | 本站未开放此功能 | 本站未開放此功能 | This feature is not enabled on this site | 404 NOT_FOUND |
| 4102 | PoolGuard | 抽卡池含保底综合返还率 >100% 被运行时拒绝（G31-B） | 奖池保底后的返还率超过 100%，已拒绝 | 獎池保底後的返還率超過 100%，已拒絕 | Pool return rate exceeds 100% after the guarantee; rejected | 409 CONFLICT |
| 4103 | MailUnconfigured | 站点未配邮件服务却走了必须发信的链路（P-102） | 邮件服务当前不可用，请稍后重试或联系管理组 | 郵件服務目前無法使用，請稍後重試或聯繫管理組 | Mail service is temporarily unavailable; retry later or contact staff | 503 SERVICE_UNAVAILABLE |
| 5002 | AlreadyThanked | 对同一资源重复提交感谢 | 已感谢过 | 已感謝過 | Already thanked | 400 BAD_REQUEST |
| 5003 | VersionConflict | 设定保存携带的 expected updated_at 与库中不一致 | 配置已被其他人修改，请刷新后重试 | 配置已被其他人修改，請重新整理後重試 | This configuration was changed by someone else; please reload and retry | 409 CONFLICT |
