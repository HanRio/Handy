//! Headless HTTP front end: `POST /transcribe` takes an audio file and returns
//! Korean text with segment timestamps.

use crate::audio_toolkit::decode_to_16k_mono;
use crate::managers::transcription::TimedSegment;
use crate::{ModelManager, TranscriptionManager};
use anyhow::Result;
use axum::extract::{DefaultBodyLimit, Multipart, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

pub const DEFAULT_MODEL_ID: &str = "handy-computer/whisper-large-v3-turbo-gguf/whisper-large-v3-turbo-Q8_0.gguf";
const MAX_UPLOAD_BYTES: usize = 2 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct ServerState {
    pub tm: Arc<TranscriptionManager>,
    pub models: Arc<ModelManager>,
    pub token: Option<Arc<str>>,
    /// One transcription at a time: the engine holds a single model/worker.
    pub gate: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Serialize)]
struct TranscribeResponse {
    text: String,
    language: &'static str,
    duration_sec: f64,
    segments: Vec<SegmentOut>,
}

#[derive(Serialize)]
struct SegmentOut {
    start: f64,
    end: f64,
    text: String,
}

impl From<TimedSegment> for SegmentOut {
    fn from(s: TimedSegment) -> Self {
        Self {
            start: s.start_ms as f64 / 1000.0,
            end: s.end_ms as f64 / 1000.0,
            text: s.text,
        }
    }
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/transcribe", post(transcribe))
        .route("/health", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state)
}

/// Optional bearer auth (`HANDY_API_TOKEN`); /health stays open.
async fn require_token(State(state): State<ServerState>, req: Request, next: Next) -> Response {
    if let Some(token) = &state.token {
        let ok = req.uri().path() == "/health"
            || req
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .is_some_and(|given| given == token.as_ref());
        if !ok {
            return ApiError(StatusCode::UNAUTHORIZED, "invalid or missing token".into())
                .into_response();
        }
    }
    next.run(req).await
}

async fn transcribe(
    State(state): State<ServerState>,
    mut multipart: Multipart,
) -> Result<Json<TranscribeResponse>, ApiError> {
    let bad = |msg: String| ApiError(StatusCode::BAD_REQUEST, msg);
    let mut upload = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| bad(e.to_string()))? {
        if field.name() == Some("file") {
            let ext = field
                .file_name()
                .and_then(|n| n.rsplit_once('.'))
                .map(|(_, e)| e.to_lowercase());
            let bytes = field.bytes().await.map_err(|e| bad(e.to_string()))?;
            upload = Some((bytes.to_vec(), ext));
            break;
        }
    }
    let (bytes, ext) = upload.ok_or_else(|| bad("multipart field `file` is required".into()))?;

    let audio = tokio::task::spawn_blocking(move || decode_to_16k_mono(bytes, ext.as_deref()))
        .await
        .map_err(internal)?
        .map_err(|e| ApiError(StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))?;
    let duration_sec = audio.len() as f64 / 16_000.0;

    let _turn = state.gate.lock().await;
    let tm = state.tm.clone();
    let result = tokio::task::spawn_blocking(move || {
        if !tm.is_model_loaded() {
            tm.load_model(DEFAULT_MODEL_ID)?;
        }
        tm.transcribe_timed(audio)
    })
    .await
    .map_err(internal)?
    .map_err(internal)?;

    Ok(Json(TranscribeResponse {
        text: result.text,
        language: "ko",
        duration_sec,
        segments: result.segments.into_iter().map(Into::into).collect(),
    }))
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    log::error!("transcription failed: {e}");
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, "transcription failed".into())
}
