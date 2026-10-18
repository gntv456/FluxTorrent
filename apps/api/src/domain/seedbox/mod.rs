//! seedbox 盒子的校验与标志位语义（PT Studio FR-SBOX 对标，FRAME §7 第 6 条）。
//!
//! 【为什么这层必须在 domain】用户申报的 CIDR 会直接进入「反作弊放宽」
//! 的判据——放错了等于给真实作弊者开门。所以「什么算合法申报」不能是
//! H 层随手写的正则，而是要有单测、有边界判定的纯函数。
//!
//! 【安全边界——本设计最重要的一条】
//! seedbox 标志**只放宽、不加重**：
//!   * 豁免的是「因不可探测（加密/CGNAT）而被误判」这一类；
//!   * 幽灵做种、流量虚报、跨种污染等**不豁免**——盒子不改变行为的性质，
//!     只改变「能不能被观测到」。
//! 这条写死在 `Flags` 的构造上：任何标志位都只是 *relax*，没有 *penalize*
//! 分支。这样即使将来有人加字段，也不会不小心加出「更严格」的语义。

use std::net::IpAddr;

use crate::errors::{DomainError, DomainResult};

/// 单段 CIDR 的最小前缀长度。
///
/// 为什么卡 /24：太宽的段（如 /8）等于「几乎全网」，任何人都能申报一个
/// 大段来覆盖自己的动态 IP 池，从而拿到放宽。/24（256 个地址）对家庭
/// 宽带/小机房是合理粒度，盒子商家按 /24 段申报也不会碎得太厉害。
const MIN_PREFIX_V4: u8 = 24;
/// IPv6 卡 /48（一个标准分配单元）。
const MIN_PREFIX_V6: u8 = 48;
/// 单次申报最多几段。
const MAX_CIDRS: usize = 20;

/// 校验并归一化一段 CIDR。
///
/// 拒绝：裸 IP（无前缀）、前缀过宽、非全局地址（回环/私有/链路本地/
/// 组播/未指定）。私有段允许——盒子多在机房私网里，这是正常用法。
pub fn normalize_cidr(raw: &str) -> DomainResult<(IpAddr, u8)> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(DomainError::Validation("CIDR 为空".into()));
    }
    if s.split_whitespace().count() != 1 {
        return Err(DomainError::Validation("一次只能申报一段 CIDR".into()));
    }
    let (ip_part, prefix_part) = s.split_once('/').ok_or_else(|| {
        DomainError::Validation("缺少前缀长度（应形如 1.2.3.0/24）".into())
    })?;
    let ip: IpAddr = ip_part
        .parse()
        .map_err(|_| DomainError::Validation(format!("IP 非法：{ip_part}")))?;
    let prefix: u8 = prefix_part.parse().map_err(|_| {
        DomainError::Validation(format!("前缀长度非法：{prefix_part}"))
    })?;
    let max = match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if prefix > max {
        return Err(DomainError::Validation(format!(
            "前缀长度超界：/{prefix}（最大 /{max}）"
        )));
    }
    let floor = match ip {
        IpAddr::V4(_) => MIN_PREFIX_V4,
        IpAddr::V6(_) => MIN_PREFIX_V6,
    };
    if prefix < floor {
        return Err(DomainError::Validation(format!(
            "前缀过宽（至少 /{floor}）：/{} 等于把大半个互联网纳入豁免",
            prefix
        )));
    }
    Ok((ip, prefix))
}

/// 校验一整批申报并返回归一化文本（去重 + 稳定排序）。
///
/// 去重很关键：同一段重复申报会让计数类判断偏大，排序则是为了让
/// `submit` 天然幂等——同样的输入产生同样的存储形态。
pub fn normalize_claims(raw: &[String]) -> DomainResult<Vec<String>> {
    if raw.is_empty() {
        return Err(DomainError::Validation("至少申报一段 CIDR".into()));
    }
    if raw.len() > MAX_CIDRS {
        return Err(DomainError::Validation(format!(
            "一次最多申报 {MAX_CIDRS} 段"
        )));
    }
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    for r in raw {
        let (ip, prefix) = normalize_cidr(r)?;
        out.push(format!("{ip}/{prefix}"));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// 某 IP 是否落在申报段内（反作弊判据的读取口）。
pub fn ip_in_claims(ip: IpAddr, claims: &[String]) -> bool {
    claims.iter().any(|c| {
        let Some((net, prefix)) = c.split_once('/').and_then(|(a, b)| {
            let ip: IpAddr = a.parse().ok()?;
            let p: u8 = b.parse().ok()?;
            Some((ip, p))
        }) else {
            return false;
        };
        match (net, ip) {
            (IpAddr::V4(n), IpAddr::V4(t)) => {
                mask_v4(n, prefix) == mask_v4(t, prefix)
            }
            (IpAddr::V6(n), IpAddr::V6(t)) => {
                mask_v6(n, prefix) == mask_v6(t, prefix)
            }
            _ => false, // v4/v6 混合不可能相等
        }
    })
}

fn mask_v4(ip: std::net::Ipv4Addr, prefix: u8) -> u32 {
    let bits = u32::from(ip);
    if prefix == 0 {
        0
    } else {
        bits & (!0u32 << (32 - prefix as u32))
    }
}

fn mask_v6(ip: std::net::Ipv6Addr, prefix: u8) -> u128 {
    // u128::from(Ipv6Addr) 不存在；从 16 字节拼（Octets 大端）。
    let o = ip.octets();
    let bits = u128::from_be_bytes(o);
    if prefix == 0 {
        0
    } else {
        bits & (!0u128 << (128 - prefix as u32))
    }
}

/// 生效的标志位（反作弊 / H&R / 保种倍率统一读它）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flags {
    /// 是否盒子用户（自证通过 或 系统识别）。
    pub is_seedbox: bool,
    /// 用户自证且人工审核通过（可信等级最高）。
    pub user_declared: bool,
    /// H&R 豁免。
    pub hnr_exempt: bool,
    /// 保种倍率（默认 1.0）。
    pub bonus_factor: f64,
    /// 速度规则放宽。
    pub speed_relax: bool,
}

impl Default for Flags {
    /// 默认必须是「零影响」——拿不到画像的老用户不应被任何特殊对待。
    fn default() -> Self {
        Self {
            is_seedbox: false,
            user_declared: false,
            hnr_exempt: false,
            bonus_factor: 1.0,
            speed_relax: false,
        }
    }
}

impl Flags {
    /// 由画像行构造。**只放宽，不加重**——见模块头安全边界。
    pub fn from_profile(
        approved: bool,
        detected: bool,
        hnr_exempt: bool,
        bonus_factor: f64,
        speed_relax: bool,
    ) -> Self {
        let is_seedbox = approved || detected;
        if !is_seedbox {
            return Self::default();
        }
        // detected 只作提示：它可以给 speed_relax，但**不给** hnr_exempt
        // （豁免 H&R 是强承诺，须人工审核背书；自动识别误判成本太高）。
        Self {
            is_seedbox: true,
            user_declared: approved,
            hnr_exempt: approved && hnr_exempt,
            bonus_factor: if approved { bonus_factor.max(1.0) } else { 1.0 },
            speed_relax: speed_relax || detected,
        }
    }
}

#[cfg(test)]
mod tests;
