//! Join-alert sound playback (default embedded MP3 or a user-chosen file).

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

const DEFAULT_SOUND: &[u8] = include_bytes!("../assets/CS2-match-found.mp3");

pub struct AlertPlayer {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    volume: f32,
}

impl AlertPlayer {
    pub fn new(volume: f32) -> Self {
        let volume = volume.clamp(0.0, 1.0);
        match OutputStream::try_default() {
            Ok((stream, handle)) => Self {
                _stream: Some(stream),
                handle: Some(handle),
                sink: None,
                volume,
            },
            Err(_) => Self {
                _stream: None,
                handle: None,
                sink: None,
                volume,
            },
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
    }

    /// Start (or restart) the alert sound. Stops any currently playing alert first.
    pub fn play(&mut self, custom_path: Option<&Path>) {
        self.stop();
        let Some(handle) = &self.handle else {
            return;
        };
        let Ok(sink) = Sink::try_new(handle) else {
            return;
        };
        sink.set_volume(self.volume);

        let played = custom_path
            .and_then(|p| {
                let file = File::open(p).ok()?;
                let source = Decoder::new(BufReader::new(file)).ok()?;
                sink.append(source);
                Some(())
            })
            .is_some();

        if !played {
            if let Ok(source) = Decoder::new(Cursor::new(DEFAULT_SOUND)) {
                sink.append(source);
            } else {
                return;
            }
        }

        self.sink = Some(sink);
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }
}

/// Resolve a configured custom sound path (empty / missing → None).
pub fn custom_sound_path(configured: &Option<String>) -> Option<PathBuf> {
    configured
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_file())
}
