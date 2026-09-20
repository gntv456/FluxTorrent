//! 用户公开主页类型（M01）：RecentComment / RecentUpload / PublicProfile。
//! 从 auth_http.rs 按域拆出（profile.rs 使用）。

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct RecentComment {
    pub(super) torrent_id: i64,
    pub(super) body: String,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct RecentUpload {
    pub(super) id: i64,
    pub(super) name: String,
    pub(super) small_descr: Option<String>,
    pub(super) size: i64,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct PublicProfile {
    pub(super) id: i64,
    pub(super) username: String,
    pub(super) title: Option<String>,
    pub(super) avatar_url: Option<String>,
    pub(super) class_id: i32,
    pub(super) class_name: Option<String>,
    pub(super) uploaded: i64,
    pub(super) downloaded: i64,
    pub(super) donor: bool,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
    pub(super) last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    pub(super) avatar_frame_css: Option<String>,
    #[sqlx(default)]
    pub(super) avatar_frame_image: Option<String>,
    pub(super) seeding: i64,
    pub(super) leeching: i64,
    pub(super) uploads: i64,
    #[serde(rename = "comments")]
    pub(super) comment_count: i64,
    pub(super) medals: i64,
}
