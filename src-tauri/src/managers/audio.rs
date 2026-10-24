use crate::audio_toolkit::{vad::EarshotVad, SileroVad, SpeechScorer};
use crate::host::AppHandle;
use crate::settings::VadBackend;

const SILERO_VAD_THRESHOLD: f32 = 0.3;
const EARSHOT_VAD_THRESHOLD: f32 = 0.5;

/// A fresh speech scorer for `backend`, used to cut long audio into chunks.
pub fn create_detector(
    backend: VadBackend,
    app_handle: &AppHandle,
) -> Result<Box<dyn SpeechScorer>, anyhow::Error> {
    Ok(match backend {
        VadBackend::Silero => {
            let vad_path = app_handle.resource_path("resources/models/silero-vad-v6.2-F32.gguf");
            Box::new(
                SileroVad::new(vad_path, SILERO_VAD_THRESHOLD)
                    .map_err(|e| anyhow::anyhow!("Failed to create SileroVad: {e}"))?,
            )
        }
        VadBackend::Earshot => Box::new(
            EarshotVad::new(EARSHOT_VAD_THRESHOLD)
                .map_err(|e| anyhow::anyhow!("Failed to create EarshotVad: {e}"))?,
        ),
    })
}
