// SPDX-License-Identifier: MIT OR Apache-2.0
//! GStreamer `playbin3` wrapper for v0.1.0 single-file playback.
//!
//! The engine owns a `gst::Pipeline` with one `playbin3` element.
//! v0.1.0 supports `muzon play <file>`: load a single URI, play to
//! end-of-stream, return. v0.2.0+ adds gapless (about-to-finish),
//! ReplayGain, crossfade, bit-perfect mode, and the appsink PCM
//! tap for the spectrum UI.
//!
//! See TZ.md §2.1.1, §3.2, §3.7. Pipeline-shape decision:
//! [docs/decisions/0003-audio-backend.md](../docs/decisions/0003-audio-backend.md).

use std::path::Path;
use std::sync::Once;

use gstreamer::prelude::*;
use gstreamer::{ElementFactory, Pipeline, State};
use tracing::{info, warn};

static INIT_GST: Once = Once::new();

/// Ensure `gst::init` has been called exactly once. Subsequent
/// calls are no-ops.
pub fn init() -> Result<(), gstreamer::glib::Error> {
    let mut result: Result<(), gstreamer::glib::Error> = Ok(());
    INIT_GST.call_once(|| {
        result = gstreamer::init();
    });
    result
}

/// Errors produced by the engine.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("GStreamer initialisation failed: {0}")]
    Init(#[from] gstreamer::glib::Error),

    #[error("failed to build GStreamer pipeline: {0}")]
    Build(String),

    #[error("file does not exist: {0}")]
    MissingFile(String),

    #[error("playback failed: {0}")]
    Playback(String),
}

/// Audio engine that owns one `playbin3` pipeline. Cheap to clone
/// (the underlying `Pipeline` is reference-counted inside gstreamer-rs).
#[derive(Clone)]
pub struct AudioEngine {
    pipeline: Pipeline,
}

impl AudioEngine {
    /// Build a new pipeline. The pipeline is in `Null` state until
    /// [`play`] is called.
    pub fn new() -> Result<Self, AudioError> {
        init()?;
        let pipeline = gstreamer::Pipeline::with_name("muzon-audio");
        let playbin = ElementFactory::make("playbin3")
            .name("playbin3")
            .build()
            .map_err(|e| AudioError::Build(format!("playbin3: {e}")))?;
        pipeline.add(&playbin).map_err(|e| AudioError::Build(format!("add: {e}")))?;
        info!("muzon-audio: pipeline built; playbin3 attached");
        Ok(Self { pipeline })
    }

    /// Play a single file to completion. Blocks the calling task
    /// until the pipeline hits `EndOfStream` or `Error`. The
    /// pipeline is returned to `Null` on exit.
    pub fn play(&self, path: &Path) -> Result<(), AudioError> {
        if !path.exists() {
            return Err(AudioError::MissingFile(path.to_string_lossy().into_owned()));
        }
        let uri = url_from_path(path);
        let playbin = self.pipeline.by_name("playbin3").ok_or_else(|| {
            AudioError::Build("playbin3 element missing from pipeline".to_string())
        })?;
        playbin.set_property("uri", &uri);

        info!("muzon-audio: playing {uri}");

        self.pipeline
            .set_state(State::Playing)
            .map_err(|e| AudioError::Playback(format!("set_state Playing: {e}")))?;

        let bus = self.pipeline.bus().ok_or_else(|| {
            AudioError::Playback("pipeline has no bus".to_string())
        })?;

        for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Eos(_) => {
                    info!("muzon-audio: end of stream");
                    break;
                }
                MessageView::Error(err) => {
                    let src = err
                        .src()
                        .map(|s| s.name().to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    let detail = err.error().to_string();
                    let _ = self.pipeline.set_state(State::Null);
                    return Err(AudioError::Playback(format!("{src}: {detail}")));
                }
                MessageView::StateChanged(state) => {
                    if let Some(src) = msg.src() {
                        if src == self.pipeline.upcast_ref::<gstreamer::Object>() {
                            let new = state.current();
                            let old = state.old();
                            if new == State::Playing && old == State::Paused {
                                info!("muzon-audio: resumed");
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.pipeline
            .set_state(State::Null)
            .map_err(|e| warn!("muzon-audio: failed to set pipeline to Null: {e}"))
            .ok();

        Ok(())
    }

    /// Stop the pipeline. Returns the pipeline to `Null` and
    /// releases the audio device.
    pub fn stop(&self) -> Result<(), AudioError> {
        self.pipeline
            .set_state(State::Null)
            .map_err(|e| AudioError::Playback(format!("stop: {e}")))?;
        Ok(())
    }

    /// Reference to the underlying `gst::Pipeline`. Use with care.
    pub fn pipeline(&self) -> &Pipeline {
        &self.pipeline
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new().expect("AudioEngine::new with default GStreamer install")
    }
}

fn url_from_path(path: &Path) -> String {
    // GStreamer URIs are file:// URIs. On Linux an absolute path
    // is encoded as `file:///abs/path`. We do not bother with
    // URL-encoding the path because gstreamer-rs's URI parser is
    // tolerant of unencoded paths in practice; v0.2.0 will switch
    // to `gst_uri_uri_from_path` (or `glib::filename_to_uri`) when
    // it lands.
    format!("file://{}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_construction() {
        let engine = AudioEngine::new().expect("engine");
        let state = engine.pipeline().current_state();
        // The pipeline starts in Null; never in Playing.
        assert_ne!(state, State::Playing);
    }

    #[test]
    fn missing_file_errors() {
        let engine = AudioEngine::new().expect("engine");
        let result = engine.play(Path::new("/nonexistent/path.flac"));
        assert!(matches!(result, Err(AudioError::MissingFile(_))));
    }

    #[test]
    fn url_from_path_prepends_scheme() {
        let url = url_from_path(Path::new("/tmp/a.flac"));
        assert_eq!(url, "file:///tmp/a.flac");
    }

    #[test]
    fn init_is_idempotent() {
        // The static Once means two init() calls do not panic and
        // do not re-init GStreamer.
        init().expect("first");
        init().expect("second");
    }
}
