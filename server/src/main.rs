use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::{error, info};

type Db = Arc<Mutex<Connection>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "amt_social.db".to_string());
    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let cors_origin = std::env::var("CORS_ORIGIN").unwrap_or_else(|_| "*".to_string());

    let conn = Connection::open(&db_path).expect("Failed to open database");
    init_db(&conn);

    let db: Db = Arc::new(Mutex::new(conn));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/social/register", post(register_user))
        .route("/api/social/users/:uuid", get(get_user))
        .route("/api/social/posts", post(create_post))
        .route("/api/social/feed", get(get_feed))
        .route("/api/social/posts/:id", get(get_post))
        .route("/api/social/posts/:id/like", post(like_post))
        .route("/api/social/hashtags", get(trending_hashtags))
        .route("/api/social/search", get(search_posts))
        .layer(CorsLayer::permissive())
        .with_state(db);

    info!("🚀 AMT Social Server listening on {bind}");
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}

fn init_db(conn: &Connection) {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS users (
            uuid TEXT PRIMARY KEY,
            minecraft_username TEXT NOT NULL,
            badge TEXT NOT NULL DEFAULT '',
            equipped_cape TEXT,
            joined_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS posts (
            id TEXT PRIMARY KEY,
            author_uuid TEXT NOT NULL,
            content TEXT NOT NULL,
            post_type TEXT NOT NULL DEFAULT 'text',
            attachment_data TEXT,
            created_at TEXT NOT NULL,
            likes INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (author_uuid) REFERENCES users(uuid)
        );
        CREATE TABLE IF NOT EXISTS likes (
            post_id TEXT NOT NULL,
            user_uuid TEXT NOT NULL,
            PRIMARY KEY (post_id, user_uuid),
            FOREIGN KEY (post_id) REFERENCES posts(id),
            FOREIGN KEY (user_uuid) REFERENCES users(uuid)
        );
        CREATE TABLE IF NOT EXISTS hashtags (
            tag TEXT NOT NULL,
            post_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (post_id) REFERENCES posts(id)
        );
        CREATE INDEX IF NOT EXISTS idx_hashtags_tag ON hashtags(tag);
        CREATE INDEX IF NOT EXISTS idx_hashtags_post ON hashtags(post_id);
        CREATE INDEX IF NOT EXISTS idx_posts_created ON posts(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_posts_author ON posts(author_uuid);
        ",
    )
    .expect("Failed to initialize database");
}

async fn health() -> &'static str {
    "ok"
}

// ── Models ──

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UserRecord {
    uuid: String,
    minecraft_username: String,
    badge: String,
    equipped_cape: Option<String>,
    joined_at: String,
    last_seen_at: String,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    uuid: String,
    minecraft_username: String,
    badge: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PostRecord {
    id: String,
    author_uuid: String,
    author_username: String,
    author_badge: String,
    content: String,
    post_type: String,
    attachment_data: Option<serde_json::Value>,
    created_at: String,
    likes: i32,
    liked_by_me: bool,
    hashtags: Vec<String>,
    mentions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreatePostRequest {
    author_uuid: String,
    content: String,
    post_type: Option<String>,
    attachment_data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct FeedQuery {
    tag: Option<String>,
    user: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Serialize)]
struct FeedResponse {
    posts: Vec<PostRecord>,
    total: i64,
}

// ── Handlers ──

async fn register_user(
    State(db): State<Db>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let db = db.lock().await;
    let now = Utc::now().to_rfc3339();

    let result = db.execute(
        "INSERT INTO users (uuid, minecraft_username, badge, joined_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(uuid) DO UPDATE SET
            minecraft_username = excluded.minecraft_username,
            badge = COALESCE(NULLIF(excluded.badge, ''), users.badge),
            last_seen_at = excluded.last_seen_at",
        rusqlite::params![
            req.uuid,
            req.minecraft_username,
            req.badge.unwrap_or_default(),
            now,
            now,
        ],
    );

    match result {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => {
            error!("Failed to register user: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed").into_response()
        }
    }
}

async fn get_user(
    State(db): State<Db>,
    Path(uuid): Path<String>,
) -> impl IntoResponse {
    let db = db.lock().await;
    let mut stmt = db
        .prepare("SELECT uuid, minecraft_username, badge, equipped_cape, joined_at, last_seen_at FROM users WHERE uuid = ?1")
        .unwrap();

    let user = stmt
        .query_row(rusqlite::params![uuid], |row| {
            Ok(UserRecord {
                uuid: row.get(0)?,
                minecraft_username: row.get(1)?,
                badge: row.get(2)?,
                equipped_cape: row.get(3)?,
                joined_at: row.get(4)?,
                last_seen_at: row.get(5)?,
            })
        })
        .ok();

    match user {
        Some(u) => Json(serde_json::json!(u)).into_response(),
        None => (StatusCode::NOT_FOUND, "User not found").into_response(),
    }
}

async fn create_post(
    State(db): State<Db>,
    Json(req): Json<CreatePostRequest>,
) -> impl IntoResponse {
    let db = db.lock().await;

    // Verify user exists
    let user_exists: bool = db
        .query_row(
            "SELECT COUNT(*) FROM users WHERE uuid = ?1",
            rusqlite::params![req.author_uuid],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);

    if !user_exists {
        return (
            StatusCode::BAD_REQUEST,
            "User not registered. Call /api/social/register first.",
        )
            .into_response();
    }

    let post_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let post_type = req.post_type.unwrap_or_else(|| "text".to_string());
    let attachment = req
        .attachment_data
        .as_ref()
        .map(|d| serde_json::to_string(d).unwrap_or_default());

    // Extract hashtags and mentions
    let hashtags: Vec<String> = extract_hashtags(&req.content);
    let mentions: Vec<String> = extract_mentions(&req.content);

    let result = db.execute(
        "INSERT INTO posts (id, author_uuid, content, post_type, attachment_data, created_at, likes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
        rusqlite::params![post_id, req.author_uuid, req.content, post_type, attachment, now],
    );

    if let Err(e) = result {
        error!("Failed to create post: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create post").into_response();
    }

    // Insert hashtags
    for tag in &hashtags {
        let _ = db.execute(
            "INSERT INTO hashtags (tag, post_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![tag.to_lowercase(), post_id, now],
        );
    }

    // Fetch author info for response
    let (author_username, author_badge) = db
        .query_row(
            "SELECT minecraft_username, badge FROM users WHERE uuid = ?1",
            rusqlite::params![req.author_uuid],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap_or_default();

    let post = PostRecord {
        id: post_id,
        author_uuid: req.author_uuid,
        author_username,
        author_badge,
        content: req.content,
        post_type,
        attachment_data: req.attachment_data,
        created_at: now,
        likes: 0,
        liked_by_me: false,
        hashtags,
        mentions,
    };

    (StatusCode::CREATED, Json(post)).into_response()
}

async fn get_feed(
    State(db): State<Db>,
    Query(query): Query<FeedQuery>,
) -> impl IntoResponse {
    let db = db.lock().await;
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    // Build query based on filters
    let mut where_clauses = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(tag) = &query.tag {
        where_clauses.push("p.id IN (SELECT post_id FROM hashtags WHERE tag = ?)".to_string());
        params.push(Box::new(tag.to_lowercase()));
    }

    if let Some(user) = &query.user {
        where_clauses.push("p.author_uuid = ?".to_string());
        params.push(Box::new(user.clone()));
    }

    if let Some(search) = &query.search {
        where_clauses.push("p.content LIKE ?".to_string());
        params.push(Box::new(format!("%{search}%")));
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Count total
    let count_query = format!("SELECT COUNT(*) FROM posts p {where_sql}");
    let total: i64 = db
        .query_row(&count_query, rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |row| row.get(0))
        .unwrap_or(0);

    // Get posts sorted by recommendation score: likes*10 + recency in hours
    let feed_query = format!(
        "SELECT p.id, p.author_uuid, p.content, p.post_type, p.attachment_data, p.created_at, p.likes,
                u.minecraft_username, u.badge
         FROM posts p
         JOIN users u ON p.author_uuid = u.uuid
         {where_sql}
         ORDER BY (p.likes * 10 + CAST((julianday('now') - julianday(p.created_at)) * -24 AS INTEGER)) DESC
         LIMIT ? OFFSET ?"
    );

    let mut stmt = match db.prepare(&feed_query) {
        Ok(s) => s,
        Err(e) => {
            error!("Query error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Query failed").into_response();
        }
    };

    let mut param_refs: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    param_refs.push(&limit);
    param_refs.push(&offset);

    let posts = stmt
        .query_map(rusqlite::params_from_iter(param_refs), |row| {
            let attachment_str: Option<String> = row.get(4)?;
            let attachment_data = attachment_str
                .and_then(|s| serde_json::from_str(&s).ok());
            let content: String = row.get(2)?;

            Ok(PostRecord {
                id: row.get(0)?,
                author_uuid: row.get(1)?,
                content: content.clone(),
                post_type: row.get(3)?,
                attachment_data,
                created_at: row.get(5)?,
                likes: row.get(6)?,
                author_username: row.get(7)?,
                author_badge: row.get(8)?,
                liked_by_me: false,
                hashtags: extract_hashtags(&content),
                mentions: extract_mentions(&content),
            })
        })
        .unwrap()
        .filter_map(|p| p.ok())
        .collect::<Vec<_>>();

    Json(FeedResponse { posts, total }).into_response()
}

async fn get_post(
    State(db): State<Db>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let db = db.lock().await;

    let post = db.query_row(
        "SELECT p.id, p.author_uuid, p.content, p.post_type, p.attachment_data, p.created_at, p.likes,
                u.minecraft_username, u.badge
         FROM posts p
         JOIN users u ON p.author_uuid = u.uuid
         WHERE p.id = ?1",
        rusqlite::params![id],
        |row| {
            let attachment_str: Option<String> = row.get(4)?;
            let attachment_data = attachment_str.and_then(|s| serde_json::from_str(&s).ok());
            let content: String = row.get(2)?;

            Ok(PostRecord {
                id: row.get(0)?,
                author_uuid: row.get(1)?,
                content: content.clone(),
                post_type: row.get(3)?,
                attachment_data,
                created_at: row.get(5)?,
                likes: row.get(6)?,
                author_username: row.get(7)?,
                author_badge: row.get(8)?,
                liked_by_me: false,
                hashtags: extract_hashtags(&content),
                mentions: extract_mentions(&content),
            })
        },
    );

    match post {
        Ok(p) => Json(p).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Post not found").into_response(),
    }
}

async fn like_post(
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let user_uuid = body
        .get("user_uuid")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if user_uuid.is_empty() {
        return (StatusCode::BAD_REQUEST, "user_uuid required").into_response();
    }

    let db = db.lock().await;

    // Check if already liked
    let already_liked: bool = db
        .query_row(
            "SELECT COUNT(*) FROM likes WHERE post_id = ?1 AND user_uuid = ?2",
            rusqlite::params![id, user_uuid],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if already_liked {
        // Unlike
        let _ = db.execute(
            "DELETE FROM likes WHERE post_id = ?1 AND user_uuid = ?2",
            rusqlite::params![id, user_uuid],
        );
        let _ = db.execute(
            "UPDATE posts SET likes = MAX(0, likes - 1) WHERE id = ?1",
            rusqlite::params![id],
        );
        Json(serde_json::json!({"liked": false}))
    } else {
        // Like
        let _ = db.execute(
            "INSERT INTO likes (post_id, user_uuid) VALUES (?1, ?2)",
            rusqlite::params![id, user_uuid],
        );
        let _ = db.execute(
            "UPDATE posts SET likes = likes + 1 WHERE id = ?1",
            rusqlite::params![id],
        );
        Json(serde_json::json!({"liked": true}))
    }
    .into_response()
}

async fn trending_hashtags(
    State(db): State<Db>,
) -> impl IntoResponse {
    let db = db.lock().await;
    let mut stmt = db
        .prepare(
            "SELECT h.tag, COUNT(*) as count
             FROM hashtags h
             WHERE h.created_at > datetime('now', '-7 days')
             GROUP BY h.tag
             ORDER BY count DESC
             LIMIT 20",
        )
        .unwrap();

    let tags: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "tag": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?
            }))
        })
        .unwrap()
        .filter_map(|t| t.ok())
        .collect();

    Json(tags).into_response()
}

async fn search_posts(
    State(db): State<Db>,
    Query(query): Query<FeedQuery>,
) -> impl IntoResponse {
    get_feed(State(db), Query(query)).await
}

// ── Content Parsers ──

fn extract_hashtags(content: &str) -> Vec<String> {
    content
        .split_whitespace()
        .filter(|w| w.starts_with('#') && w.len() > 1)
        .map(|w| w.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string())
        .map(|w| w[1..].to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

fn extract_mentions(content: &str) -> Vec<String> {
    content
        .split_whitespace()
        .filter(|w| w.starts_with('@') && w.len() > 1)
        .map(|w| w.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string())
        .map(|w| w[1..].to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}
