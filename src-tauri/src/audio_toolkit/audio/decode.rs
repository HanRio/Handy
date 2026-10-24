use anyhow::{anyhow, Context, Result};
use audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Resampler};
use std::io::Cursor;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

const TARGET_RATE: u32 = 16_000;

/// Decode an uploaded audio file (m4a/mp3/wav/flac/ogg/...) to 16 kHz mono f32.
pub fn decode_to_16k_mono(bytes: Vec<u8>, extension: Option<&str>) -> Result<Vec<f32>> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = extension {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .context("unsupported audio format")?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow!("no audio track"))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| anyhow!("missing audio codec parameters"))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .context("unsupported audio codec")?;

    let mut mono: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    let mut interleaved: Vec<f32> = Vec::new();
    while let Some(packet) = format.next_packet().context("failed to read audio")? {
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // Skip a corrupt packet rather than failing a whole meeting recording.
            Err(Error::DecodeError(_)) | Err(Error::IoError(_)) => continue,
            Err(e) => return Err(e).context("audio decode failed"),
        };
        let channels = decoded.spec().channels().count().max(1);
        rate = decoded.spec().rate();
        interleaved.resize(decoded.samples_interleaved(), 0.0);
        decoded.copy_to_slice_interleaved(&mut interleaved);
        mono.extend(
            interleaved
                .chunks_exact(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
    }
    if mono.is_empty() || rate == 0 {
        return Err(anyhow!("no audio samples decoded"));
    }
    resample(mono, rate)
}

fn resample(mono: Vec<f32>, rate: u32) -> Result<Vec<f32>> {
    if rate == TARGET_RATE {
        return Ok(mono);
    }
    let frames = mono.len();
    let input = InterleavedOwned::new_from(mono, 1, frames).map_err(|e| anyhow!("{e:?}"))?;
    let mut resampler = Fft::<f32>::new(rate as usize, TARGET_RATE as usize, 1024, 1, FixedSync::Both)
        .map_err(|e| anyhow!("resampler init failed: {e}"))?;
    let out = resampler
        .process_all(&input, frames, None)
        .map_err(|e| anyhow!("resampling failed: {e}"))?;
    Ok(out.take_data())
}
