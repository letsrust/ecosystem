use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use http::{header::LOCATION, HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[derive(Debug, Deserialize)]
struct ShortenReq {
    url: String,
}

#[derive(Debug, Serialize)]
struct ShortenRes {
    url: String,
}

#[derive(Debug, Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Debug, FromRow)]
struct UrlRecord {
    #[sqlx(default)]
    id: String,
    #[sqlx(default)]
    url: String,
}

#[derive(Debug, thiserror::Error)]
enum ShortenError {
    #[error("{0}")]
    BindException(String),
    #[error("Connection/Accept failure: {0}")]
    ClientConnectionFailure(String),
    #[error("{0}")]
    PrimaryKeyConflict(String),
    #[error("DB Operation: {0}")]
    DbOperationException(String),

    #[error("Redirect URL not found")]
    NotFound,
}

const LISTEN_ADDR: &str = "127.0.0.1:9876";
const MAX_RETRY_TIMES: usize = 10;

#[tokio::main]
async fn main() -> Result<(), ShortenError> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let db_url = "postgres://user:user@localhost:5432/shortener";
    let state = AppState::try_new(db_url).await?;

    let listener = TcpListener::bind(LISTEN_ADDR).await.map_err(|e| {
        let err_msg = format!("{}: {}", e, LISTEN_ADDR);
        ShortenError::BindException(err_msg)
    })?;
    info!("Listening server on: {}", LISTEN_ADDR);

    let router = Router::new()
        .route("/", post(shorten))
        .route("/:id", get(redirect))
        .with_state(state);

    axum::serve(listener, router.into_make_service())
        .await
        .map_err(|e| ShortenError::ClientConnectionFailure(e.to_string()))?;

    Ok(())
}

async fn shorten(
    State(state): State<AppState>,
    Json(data): Json<ShortenReq>,
) -> Result<impl IntoResponse, StatusCode> {
    // let id = state.shorten(&data.url).await.map_err(|e| {
    //     warn!("Failed to shorten URL: {:?}", e);
    //     StatusCode::UNPROCESSABLE_ENTITY
    // })?;

    let mut id: String = String::new();
    let mut retry_cnt = 0;
    loop {
        if retry_cnt >= MAX_RETRY_TIMES {
            warn!("Exceed max retry times");
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }

        let shorten_res = state.shorten(&data.url).await;
        let id_str = match shorten_res {
            Ok(id) => id,
            Err(ShortenError::PrimaryKeyConflict(_)) => {
                info!("Primary key conflict, continue to generate new id");
                retry_cnt += 1;
                continue;
            }
            Err(e) => {
                warn!("Failed to shorten URL: {:?}", e);
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        };

        id.push_str(id_str.as_str());
        break;
    }

    let body = Json(ShortenRes {
        url: format!("http://{}/{}", LISTEN_ADDR, id),
    });

    Ok((StatusCode::CREATED, body))
}

async fn redirect(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ShortenError> {
    let url = state
        .get_url(&id)
        .await
        .map_err(|_| ShortenError::NotFound)?;

    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, url.parse().unwrap());

    Ok((StatusCode::PERMANENT_REDIRECT, headers))
}

impl AppState {
    async fn try_new(url: &str) -> Result<Self, ShortenError> {
        let pool = PgPool::connect(url).await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS urls (
                id CHAR(6) PRIMARY KEY,
                url TEXT NOT NULL UNIQUE
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { db: pool })
    }

    async fn shorten(&self, url: &str) -> Result<String, ShortenError> {
        let id = nanoid::nanoid!(6);
        // let id = "gz8GFx";

        let ret: UrlRecord = sqlx::query_as(
            r#"
            INSERT INTO urls (id, url) VALUES ($1, $2) on conflict(url) do update set url=excluded.url returning id
            "#,
        )
        .bind(&id)
        .bind(url)
        .fetch_one(&self.db)
        .await?;

        Ok(ret.id)
    }

    async fn get_url(&self, id: &str) -> Result<String> {
        let ret: UrlRecord = sqlx::query_as("select url from urls where id = $1")
            .bind(id)
            .fetch_one(&self.db)
            .await?;

        Ok(ret.url)
    }
}

// impl Into<ShortenError> for sqlx::Error {
//     fn into(self) -> ShortenError {
//         match self {
//             sqlx::Error::Database(err) if err.constraint() == Some("urls_pkey") => {
//                 ShortenError::PrimaryKeyConflict(String::from("urls_pkey constraint violation"))
//             }
//             _ => ShortenError::DbOperationException(self.to_string()),
//         }
//     }
// }

impl From<sqlx::Error> for ShortenError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Database(err) if err.constraint() == Some("urls_pkey") => {
                ShortenError::PrimaryKeyConflict(String::from("urls_pkey constraint violation"))
            }
            _ => ShortenError::DbOperationException(e.to_string()),
        }
    }
}

impl IntoResponse for ShortenError {
    fn into_response(self) -> Response {
        #[derive(serde::Serialize)]
        struct ErrorResp<'a> {
            message: &'a str,
            code: &'a str,
        }

        let status = match self {
            ShortenError::BindException(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ShortenError::ClientConnectionFailure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ShortenError::PrimaryKeyConflict(_) => StatusCode::CONFLICT,
            ShortenError::DbOperationException(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ShortenError::NotFound => StatusCode::NOT_FOUND,
        };

        let code = match self {
            ShortenError::BindException(_) => "BIND_ERROR",
            ShortenError::ClientConnectionFailure(_) => "CLIENT_CONNECTION_ERROR",
            ShortenError::PrimaryKeyConflict(_) => "PRIMARY_KEY_CONFLICT",
            ShortenError::DbOperationException(_) => "DB_OPERATION_ERROR",
            ShortenError::NotFound => "NOT_FOUND",
        };

        (
            status,
            Json(ErrorResp {
                message: self.to_string().as_str(),
                code,
            }),
        )
            .into_response()

        // http::Response::builder()
        //     .status(status)
        //     .body(self.to_string().into())
        //     .unwrap()
    }
}
