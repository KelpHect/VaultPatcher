//! UI sound effects, taken from the user's own game install.
//!
//! Borderlands 2's launcher ships its UI sounds as plain MP3s in
//! `Binaries\Win32\Audio` (ButtonClick, MouseOver, Applied, Whoosh, Music).
//! Vault Patcher plays those straight from the install, so the app bundles
//! no audio; without a detected install it is simply silent.

use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// Buttons, toggles, chips.
    Click,
    /// Hovering primary actions and navigation.
    Hover,
    /// A change was saved / an install finished.
    Applied,
    /// Switching page, game or mode.
    Whoosh,
}

impl Sound {
    fn file(self) -> &'static str {
        match self {
            Sound::Click => "ButtonClick.mp3",
            Sound::Hover => "MouseOver.mp3",
            Sound::Applied => "Applied.mp3",
            Sound::Whoosh => "Whoosh.mp3",
        }
    }

    fn volume(self) -> f32 {
        match self {
            Sound::Hover => 0.35,
            Sound::Click => 0.7,
            Sound::Applied | Sound::Whoosh => 0.6,
        }
    }
}

enum Command {
    Play(Sound),
    Music(bool),
}

static SENDER: OnceLock<Sender<Command>> = OnceLock::new();
static MUTED: AtomicBool = AtomicBool::new(false);
static AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Where the launcher sounds live, relative to a Willow game's install root.
pub const AUDIO_DIR: &str = "Binaries\\Win32\\Audio";

/// Loads the clips from `audio_dir` and starts the audio thread. Safe to
/// call once; later calls are ignored.
pub fn init(audio_dir: Option<&Path>, muted: bool, music: bool) {
    MUTED.store(muted, Ordering::Relaxed);
    let Some(dir) = audio_dir else { return };
    let load = |name: &str| std::fs::read(dir.join(name)).ok().map(Arc::<[u8]>::from);
    let clips: Vec<(Sound, Arc<[u8]>)> = [Sound::Click, Sound::Hover, Sound::Applied, Sound::Whoosh]
        .into_iter()
        .filter_map(|s| load(s.file()).map(|b| (s, b)))
        .collect();
    if clips.is_empty() {
        return;
    }
    let music_bytes = load("Music.mp3");
    let (tx, rx) = channel::<Command>();
    if SENDER.set(tx).is_err() {
        return;
    }
    AVAILABLE.store(true, Ordering::Relaxed);
    std::thread::Builder::new()
        .name("vault-audio".into())
        .spawn(move || {
            use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
            use std::sync::mpsc::RecvTimeoutError;
            use std::time::Duration;
            // An open output stream keeps the audio engine mixing (CPU and a
            // device thread) even in silence, so it's opened on demand and
            // closed again after a few quiet seconds unless music is playing.
            let mut stream: Option<(OutputStream, OutputStreamHandle)> = None;
            let open = |stream: &mut Option<(OutputStream, OutputStreamHandle)>| -> Option<OutputStreamHandle> {
                if stream.is_none() {
                    *stream = OutputStream::try_default().ok();
                }
                stream.as_ref().map(|(_, h)| h.clone())
            };
            let mut music_sink: Option<Sink> = None;
            let start_music = |on: bool, sink: &mut Option<Sink>, stream: &mut Option<(OutputStream, OutputStreamHandle)>| {
                if let Some(s) = sink.take() {
                    s.stop();
                }
                if on
                    && let Some(bytes) = &music_bytes
                    && let Ok(source) = Decoder::new(Cursor::new(bytes.clone()))
                    && let Some(handle) = open(stream)
                    && let Ok(s) = Sink::try_new(&handle)
                {
                    s.set_volume(0.25);
                    s.append(source.buffered().repeat_infinite());
                    *sink = Some(s);
                }
            };
            start_music(music && !muted, &mut music_sink, &mut stream);
            loop {
                let idle = if stream.is_some() && music_sink.is_none() { Duration::from_secs(5) } else { Duration::from_secs(3600) };
                match rx.recv_timeout(idle) {
                    Ok(Command::Play(sound)) => {
                        if MUTED.load(Ordering::Relaxed) {
                            continue;
                        }
                        if let Some((_, bytes)) = clips.iter().find(|(s, _)| *s == sound)
                            && let Ok(source) = Decoder::new(Cursor::new(bytes.clone()))
                            && let Some(handle) = open(&mut stream)
                        {
                            let _ = handle.play_raw(source.amplify(sound.volume()).convert_samples());
                        }
                    }
                    Ok(Command::Music(on)) => start_music(on, &mut music_sink, &mut stream),
                    Err(RecvTimeoutError::Timeout) => {
                        if music_sink.is_none() {
                            stream = None;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .ok();
}

pub fn play(sound: Sound) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.send(Command::Play(sound));
    }
}

pub fn set_muted(muted: bool) {
    MUTED.store(muted, Ordering::Relaxed);
}

pub fn set_music(on: bool) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.send(Command::Music(on));
    }
}

/// True when the game's sounds were found and an audio device opened.
pub fn available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}
