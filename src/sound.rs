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
            use rodio::{Decoder, OutputStream, Sink, Source};
            // The stream must live on this thread for as long as we play.
            let Ok((_stream, handle)) = OutputStream::try_default() else {
                AVAILABLE.store(false, Ordering::Relaxed);
                return;
            };
            let mut music_sink: Option<Sink> = None;
            let start_music = |on: bool, sink: &mut Option<Sink>| {
                if let Some(s) = sink.take() {
                    s.stop();
                }
                if on
                    && let Some(bytes) = &music_bytes
                    && let Ok(source) = Decoder::new(Cursor::new(bytes.clone()))
                    && let Ok(s) = Sink::try_new(&handle)
                {
                    s.set_volume(0.25);
                    s.append(source.buffered().repeat_infinite());
                    *sink = Some(s);
                }
            };
            start_music(music && !muted, &mut music_sink);
            for command in rx {
                match command {
                    Command::Play(sound) => {
                        if MUTED.load(Ordering::Relaxed) {
                            continue;
                        }
                        if let Some((_, bytes)) = clips.iter().find(|(s, _)| *s == sound)
                            && let Ok(source) = Decoder::new(Cursor::new(bytes.clone()))
                        {
                            let _ = handle.play_raw(source.amplify(sound.volume()).convert_samples());
                        }
                    }
                    Command::Music(on) => start_music(on, &mut music_sink),
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
