//! GeoIP 离线解析（GeoLite2 mmdb）：登录记录的国家/城市列（参考站 login-logs 口径）。
//!
//! 数据文件不入仓库（.gitignore），由镜像构建时经 `apps/api/geoip` 进入上下文；
//! 文件缺失时查询一律返回 None，功能静默降级，不影响其余字段。

use std::sync::OnceLock;

use maxminddb::geoip2;

static CITY_READER: OnceLock<Option<maxminddb::Reader<Vec<u8>>>> = OnceLock::new();
static COUNTRY_READER: OnceLock<Option<maxminddb::Reader<Vec<u8>>>> = OnceLock::new();

fn resolve(variant: &str) -> Option<String> {
    if let Ok(p) = std::env::var(format!("GEOIP_{}_DB", variant.to_uppercase())) {
        if !p.is_empty() {
            return Some(p);
        }
    }
    // 容器 WORKDIR=/app；本地 cargo 运行/测试的 cwd 是 apps/api，三种布局都兜住
    for p in [
        format!("./geoip/GeoLite2-{variant}.mmdb"),
        format!("./apps/api/geoip/GeoLite2-{variant}.mmdb"),
        format!("geoip/GeoLite2-{variant}.mmdb"),
    ] {
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    None
}

fn city_reader() -> Option<&'static maxminddb::Reader<Vec<u8>>> {
    CITY_READER
        .get_or_init(|| resolve("city").and_then(|p| maxminddb::Reader::open_readfile(p).ok()))
        .as_ref()
}

fn country_reader() -> Option<&'static maxminddb::Reader<Vec<u8>>> {
    COUNTRY_READER
        .get_or_init(|| resolve("country").and_then(|p| maxminddb::Reader::open_readfile(p).ok()))
        .as_ref()
}

/// 中文名优先（zh-CN → en），库内缺该语言时退英文
fn name_of(names: geoip2::Names<'_>) -> Option<String> {
    names
        .simplified_chinese
        .or(names.english)
        .map(str::to_string)
}

/// 返回 (国家 ISO 码, 国家名, 城市名)。内网 IP / 未命中 / 库缺失 → (None, None, None)。
pub fn lookup(ip: &str) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(addr) = ip.parse::<std::net::IpAddr>() else {
        return (None, None, None);
    };
    if let Some(reader) = city_reader() {
        // 0.27 API：lookup 返回 LookupResult，再 decode 成 geoip2 结构（借用模型，字段非 Option）
        if let Ok(hit) = reader.lookup(addr) {
            if let Ok(Some(city)) = hit.decode::<geoip2::City<'_>>() {
                let country_iso = city.country.iso_code.map(str::to_string);
                let country_name = name_of(city.country.names);
                let city_name = name_of(city.city.names);
                if country_iso.is_some() || country_name.is_some() || city_name.is_some() {
                    return (country_iso, country_name, city_name);
                }
            }
        }
    }
    // 城市库未命中（部分网段只有国家级记录）→ 国家库兜底
    if let Some(reader) = country_reader() {
        if let Ok(hit) = reader.lookup(addr) {
            if let Ok(Some(c)) = hit.decode::<geoip2::Country<'_>>() {
                let country_iso = c.country.iso_code.map(str::to_string);
                let country_name = name_of(c.country.names);
                if country_iso.is_some() || country_name.is_some() {
                    return (country_iso, country_name, None);
                }
            }
        }
    }
    (None, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geo_lookup_public_ip() {
        // mmdb 是部署资产不入库（.gitignore；CI 无库）——无库时 lookup 全程走
        // (None,None,None) 分支，该测试只在库在位时才有意义：条件跳过而非误报失败。
        if city_reader().is_none() && country_reader().is_none() {
            eprintln!("skip: GeoLite2 mmdb 不在位（部署资产，CI 环境正常缺省）");
            return;
        }
        // 8.8.8.8（Google DNS）在 GeoLite2 中应命中美国
        let (iso, name, _city) = lookup("8.8.8.8");
        assert_eq!(iso.as_deref(), Some("US"));
        assert!(name.is_some());
    }

    #[test]
    fn geo_lookup_private_ip_empty() {
        let (iso, name, city) = lookup("127.0.0.1");
        assert!(iso.is_none() && name.is_none() && city.is_none());
    }

    #[test]
    fn geo_lookup_invalid_ip_empty() {
        assert_eq!(lookup("not-an-ip"), (None, None, None));
    }
}
