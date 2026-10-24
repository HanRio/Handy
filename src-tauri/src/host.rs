//! Minimal stand-in for `tauri::AppHandle`: carries settings and data paths so
//! the model and transcription managers run without a GUI runtime.

use crate::settings::{get_default_settings, AppSettings};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

struct Inner {
    settings: RwLock<AppSettings>,
    data_dir: PathBuf,
    resource_dir: PathBuf,
}

#[derive(Clone)]
pub struct AppHandle(Arc<Inner>);

impl AppHandle {
    pub fn new(data_dir: PathBuf, resource_dir: PathBuf) -> Self {
        let mut settings = get_default_settings();
        settings.selected_model = "handy-computer/whisper-large-v3-turbo-gguf/whisper-large-v3-turbo-Q8_0.gguf".to_string();
        settings.selected_language = "ko".to_string();
        Self(Arc::new(Inner {
            settings: RwLock::new(settings),
            data_dir,
            resource_dir,
        }))
    }

    pub fn settings(&self) -> AppSettings {
        self.0.settings.read().expect("settings lock").clone()
    }

    pub fn set_settings(&self, settings: AppSettings) {
        *self.0.settings.write().expect("settings lock") = settings;
    }

    pub fn data_dir(&self) -> PathBuf {
        self.0.data_dir.clone()
    }

    pub fn resource_path(&self, relative: &str) -> PathBuf {
        self.0.resource_dir.join(relative)
    }
}

/// Events used to drive a UI; headless, they are only logged.
pub trait Emitter {
    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> Result<(), String>;
}

impl Emitter for AppHandle {
    fn emit<S: Serialize + Clone>(&self, event: &str, _payload: S) -> Result<(), String> {
        log::debug!("event: {event}");
        Ok(())
    }
}
