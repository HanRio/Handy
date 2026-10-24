use anyhow::{Context, Result};
use handy_app_lib::host::AppHandle;
use handy_app_lib::server::{router, ServerState, DEFAULT_MODEL_ID};
use handy_app_lib::{ModelManager, TranscriptionManager};
use std::path::PathBuf;
use std::sync::Arc;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn main() -> Result<()> {
    // Transcription worker: runs transcribe.cpp in an isolated child process.
    if handy_app_lib::engine_supervisor::is_worker_invocation() {
        std::process::exit(handy_app_lib::engine_supervisor::run_worker());
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(serve())
}

async fn serve() -> Result<()> {
    env_logger_init();

    let data_dir = env_path("HANDY_DATA_DIR").unwrap_or_else(|| {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        home.join(".handy-server")
    });
    std::fs::create_dir_all(&data_dir).context("cannot create data dir")?;
    // Silero VAD lives under <resource_dir>/resources/models.
    let resource_dir =
        env_path("HANDY_RESOURCE_DIR").unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    let app = AppHandle::new(data_dir, resource_dir);
    let models = Arc::new(ModelManager::new(&app)?);
    if !models
        .get_model_info(DEFAULT_MODEL_ID)
        .is_some_and(|m| m.is_downloaded)
    {
        log::info!("downloading {DEFAULT_MODEL_ID} (first run)");
        models.download_model(DEFAULT_MODEL_ID).await?;
    }
    let tm = Arc::new(TranscriptionManager::new(&app, models.clone())?);
    tm.load_model(DEFAULT_MODEL_ID)?;

    let state = ServerState {
        tm,
        models,
        token: std::env::var("HANDY_API_TOKEN").ok().filter(|t| !t.is_empty()).map(Into::into),
        gate: Arc::default(),
    };
    let addr = std::env::var("HANDY_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.context("bind failed")?;
    log::info!("listening on http://{addr}");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await?;
    Ok(())
}

fn env_logger_init() {
    let spec = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    let filter = env_filter::Builder::new().parse(&spec).build();
    struct Logger(env_filter::Filter);
    impl log::Log for Logger {
        fn enabled(&self, m: &log::Metadata) -> bool {
            self.0.enabled(m)
        }
        fn log(&self, r: &log::Record) {
            if self.0.matches(r) {
                eprintln!("[{}] {}", r.level(), r.args());
            }
        }
        fn flush(&self) {}
    }
    log::set_max_level(filter.filter());
    let _ = log::set_boxed_logger(Box::new(Logger(filter)));
}
