//! sql_guard 单元测试：每个拒绝规则至少一个用例，接受路径覆盖
//! 内置插件原句与常用查询形态。

use super::validate;
use crate::plugin_ctx::{DbCapability, DbMode};

fn caps() -> Vec<DbCapability> {
    vec![DbCapability {
        table: "torrents".into(),
        columns: ["sticky", "id", "official_tag", "name"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        mode: DbMode::Write,
        allow_unbounded: false,
    }]
}

fn v(sql: &str) -> crate::errors::DomainResult<()> {
    validate(sql, &caps())
}

#[test]
fn accepts_pin_update() {
    // auto_pin_official 原句：显式列 + 带 WHERE
    assert!(v(
        "UPDATE torrents SET sticky = TRUE WHERE id = $1 AND official_tag"
    )
    .is_ok());
}

#[test]
fn accepts_select_declared() {
    assert!(v("SELECT id, official_tag FROM torrents WHERE id = $1").is_ok());
    assert!(v("SELECT count(*) FROM torrents WHERE official_tag").is_ok());
}

#[test]
fn accepts_insert_declared() {
    assert!(v("INSERT INTO torrents (id, sticky) VALUES ($1, TRUE)").is_ok());
}

#[test]
fn rejects_other_table() {
    let e = v("SELECT id FROM users").unwrap_err();
    assert!(e.to_string().contains("越表"), "{e}");
}

#[test]
fn rejects_other_column() {
    assert!(v("SELECT pass_hash FROM users").is_err()); // 越表先拦
    let e = v("SELECT descr FROM torrents").unwrap_err();
    assert!(e.to_string().contains("未知列"), "{e}");
}

#[test]
fn rejects_qualified_over_column() {
    let e = v("SELECT torrents.descr FROM torrents").unwrap_err();
    assert!(e.to_string().contains("越列"), "{e}");
}

#[test]
fn rejects_insert_over_column() {
    // INSERT 列名走手动补收：未声明列必须拒
    let e = v("INSERT INTO torrents (id, descr) VALUES ($1, $2)").unwrap_err();
    assert!(e.to_string().contains("未知列"), "{e}");
}

#[test]
fn rejects_update_set_over_column() {
    // UPDATE SET 目标走手动补收：未声明列必须拒
    let e = v("UPDATE torrents SET descr = 'x' WHERE id = $1").unwrap_err();
    assert!(e.to_string().contains("未知列"), "{e}");
}

#[test]
fn rejects_ddl() {
    for s in [
        "CREATE TABLE x (a int)",
        "DROP TABLE torrents",
        "ALTER TABLE torrents ADD COLUMN x int",
        "TRUNCATE torrents",
        "CREATE INDEX idx ON torrents (id)",
    ] {
        assert!(v(s).is_err(), "应拒绝：{s}");
    }
}

#[test]
fn rejects_privilege() {
    for s in [
        "SET ROLE postgres",
        "RESET ROLE",
        "GRANT ALL ON torrents TO PUBLIC",
        "SELECT set_config('role', 'postgres', false)",
        "SELECT current_setting('role')",
    ] {
        assert!(v(s).is_err(), "应拒绝：{s}");
    }
}

#[test]
fn rejects_multi_statement() {
    assert!(v("SELECT 1; DROP TABLE users").is_err());
}

#[test]
fn rejects_unbounded_write() {
    assert!(v("UPDATE torrents SET sticky = TRUE").is_err());
    assert!(v("DELETE FROM torrents").is_err());
}

#[test]
fn rejects_star() {
    assert!(v("SELECT * FROM torrents").is_err());
    assert!(v("SELECT torrents.* FROM torrents").is_err());
}

#[test]
fn rejects_unknown_func() {
    assert!(v("SELECT sin(id) FROM torrents").is_err());
}

#[test]
fn rejects_table_alias() {
    // 表别名经 table_factor 钩子直接拒（错误信息含"表别名"）
    let e = v("SELECT t.id FROM torrents t").unwrap_err();
    assert!(e.to_string().contains("表别名"), "{e}");
}

#[test]
fn rejects_derived_table() {
    let e = v("SELECT s.id FROM (SELECT id FROM torrents) s").unwrap_err();
    assert!(e.to_string().contains("派生表"), "{e}");
}

#[test]
fn rejects_cte() {
    // WITH 被语句类型闸门拦
    assert!(v("WITH x AS (SELECT id FROM torrents) SELECT id FROM x").is_err());
}

#[test]
fn rejects_insert_table_alias() {
    let e = v("INSERT INTO torrents AS x (id) VALUES ($1)").unwrap_err();
    assert!(e.to_string().contains("INSERT 表别名"), "{e}");
}

#[test]
fn rejects_for_update_lock() {
    assert!(v("SELECT id FROM torrents WHERE id = $1 FOR UPDATE").is_err());
}

#[test]
fn rejects_parse_error_fail_closed() {
    assert!(v("SELCT (").is_err());
    assert!(v("").is_err());
}

#[test]
fn rejects_unknown_cast_type() {
    // `::mytype` 的自定义类型名不在类型白名单
    let e = v("SELECT $1::mytype FROM torrents WHERE id = $2").unwrap_err();
    assert!(e.to_string().contains("未知对象名"), "{e}");
}

#[test]
fn accepts_cast_builtin_type() {
    // 标准类型是具名 DataType 变体，不经对象名兜底
    assert!(v("SELECT $1::inet FROM torrents WHERE id = $2").is_ok());
}

#[test]
fn write_on_read_cap_rejected() {
    // 只读能力的表执行 UPDATE → 拒
    let read_caps = vec![DbCapability {
        table: "torrents".into(),
        columns: vec!["sticky".into(), "id".into(), "official_tag".into()],
        mode: DbMode::Read,
        allow_unbounded: false,
    }];
    let e = validate(
        "UPDATE torrents SET sticky = TRUE WHERE id = $1",
        &read_caps,
    )
    .unwrap_err();
    assert!(e.to_string().contains("Write 能力"), "{e}");
}
