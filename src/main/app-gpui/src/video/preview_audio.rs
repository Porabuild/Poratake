//! Preview audio for the video editor — the GPUI equivalent of the
//! `<video>`/`<audio>` elements in `native-video-player.tsx` and the per-track
//! elements in `use-music-playback.ts`. Stems are rendered with the same
//! `audio::decode` / `apply_segments` / `place_music_track` pipeline the
//! export uses, so the preview hears exactly what the export writes; one
//! rodio player per stem keeps volumes live without re-rendering the mix.
//! The playhead timer stays the master clock and the transport re-anchors
//! whenever it jumps or drifts past Electron's 0.3s seek threshold. Without
//! an audio device (headless CI, failing backend) everything degrades to a
//! silent no-op.

use std::collections::HashMap;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rodio::source::{SeekError, Source};
use rodio::{ChannelCount, MixerDeviceSink, Player, Sample, SampleRate};

use crate::video::audio;
use crate::video::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::video::project;
use crate::windows::video_editor::model::{MusicTrack, Segment, VideoEditorState};

const RESYNC_THRESHOLD_SECONDS: f64 = 0.3;

fn channels() -> ChannelCount {
    NonZero::new(AUDIO_CHANNELS as u16).expect("stereo channels")
}

fn sample_rate() -> SampleRate {
    NonZero::new(AUDIO_SAMPLE_RATE).expect("audio sample rate")
}

/// Interleaved stereo 16-bit samples as a seekable rodio source, converting
/// to float on the fly so long timelines do not pay the `f32` copy.
struct PcmSource {
    samples: Arc<Vec<i16>>,
    position: usize,
}

impl PcmSource {
    fn at(samples: Arc<Vec<i16>>, offset: Duration) -> Self {
        let mut source = Self {
            samples,
            position: 0,
        };
        let _ = source.try_seek(offset);
        source
    }
}

impl Iterator for PcmSource {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        let sample = *self.samples.get(self.position)?;
        self.position += 1;
        Some(sample as f32 / 32768.0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.samples.len().saturating_sub(self.position);
        (remaining, Some(remaining))
    }
}

impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        if self.position >= self.samples.len() {
            return Some(0);
        }
        Some(self.samples.len())
    }

    fn channels(&self) -> ChannelCount {
        channels()
    }

    fn sample_rate(&self) -> SampleRate {
        sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        let frames = self.samples.len() / AUDIO_CHANNELS as usize;
        Some(Duration::from_secs_f64(
            frames as f64 / AUDIO_SAMPLE_RATE as f64,
        ))
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let channels = AUDIO_CHANNELS as usize;
        let frame = (pos.as_secs_f64() * AUDIO_SAMPLE_RATE as f64).round() as usize;
        let frames = self.samples.len() / channels;
        self.position = frame.min(frames) * channels;
        Ok(())
    }
}

/// Everything a rebuild needs, cloned off the GPUI thread.
pub struct RebuildInputs {
    system_path: PathBuf,
    mic_path: PathBuf,
    music_folder: Option<PathBuf>,
    segments: Vec<Segment>,
    music_tracks: Vec<MusicTrack>,
    cache: HashMap<PathBuf, Arc<Vec<i16>>>,
}

impl RebuildInputs {
    pub fn capture(
        project: &Path,
        state: &VideoEditorState,
        cache: &HashMap<PathBuf, Arc<Vec<i16>>>,
    ) -> Self {
        Self {
            system_path: project::system_audio_path(project),
            mic_path: project::mic_audio_path(project),
            music_folder: project::music_folder(project),
            segments: state.segments.clone(),
            music_tracks: state.music_tracks.clone(),
            cache: cache.clone(),
        }
    }
}

/// Decoded stems ready to install, plus the source cache the next rebuild reuses.
pub struct BuiltStems {
    pub system: Option<Arc<Vec<i16>>>,
    pub mic: Option<Arc<Vec<i16>>>,
    pub music: Vec<(String, Arc<Vec<i16>>)>,
    pub cache: HashMap<PathBuf, Arc<Vec<i16>>>,
    pub signature: StemSignature,
}

/// The structural inputs a stem set was rendered from. Volumes and mute flags
/// are deliberately excluded — they apply live on the players.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StemSignature {
    segments: Vec<Segment>,
    music_tracks: Vec<MusicTrack>,
}

impl StemSignature {
    pub fn capture(state: &VideoEditorState) -> Self {
        Self {
            segments: state.segments.clone(),
            music_tracks: state.music_tracks.clone(),
        }
    }
}

fn decode_cached(
    path: &Path,
    cache: &mut HashMap<PathBuf, Arc<Vec<i16>>>,
) -> Option<Arc<Vec<i16>>> {
    if let Some(samples) = cache.get(path) {
        return Some(samples.clone());
    }
    let samples = Arc::new(audio::decode(path)?);
    cache.insert(path.to_path_buf(), samples.clone());
    Some(samples)
}

/// Renders every audible stem on a background thread. Mirrors
/// `export::build_audio` stem for stem, minus the final mixdown.
pub fn build_stems(inputs: RebuildInputs) -> BuiltStems {
    let mut cache = inputs.cache;
    let system = decode_cached(&inputs.system_path, &mut cache)
        .map(|samples| Arc::new(audio::apply_segments(&samples, &inputs.segments)));
    let mic = decode_cached(&inputs.mic_path, &mut cache)
        .map(|samples| Arc::new(audio::apply_segments(&samples, &inputs.segments)));
    let mut music = Vec::new();
    if let Some(folder) = inputs.music_folder {
        for track in &inputs.music_tracks {
            if track.file_name.is_empty() {
                continue;
            }
            let path = folder.join(&track.file_name);
            if let Some(samples) = decode_cached(&path, &mut cache) {
                music.push((
                    track.id.clone(),
                    Arc::new(audio::place_music_track(&samples, track)),
                ));
            }
        }
    }
    BuiltStems {
        system,
        mic,
        music,
        cache,
        signature: StemSignature {
            segments: inputs.segments,
            music_tracks: inputs.music_tracks,
        },
    }
}

struct Stem {
    player: Player,
}

impl Stem {
    fn play_from(&self, offset: f64) {
        let _ = self
            .player
            .try_seek(Duration::from_secs_f64(offset.max(0.0)));
        self.player.play();
    }
}

struct MusicStem {
    id: String,
    stem: Stem,
}

struct Output {
    system: Option<Stem>,
    mic: Option<Stem>,
    music: Vec<MusicStem>,
    anchor: Option<(Instant, f64)>,
    playing: bool,
    device: MixerDeviceSink,
}

#[derive(Default)]
pub struct PreviewAudio {
    output: Option<Output>,
    cache: HashMap<PathBuf, Arc<Vec<i16>>>,
    signature: StemSignature,
    built_once: bool,
    device_error_logged: bool,
}

impl PreviewAudio {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cache(&self) -> &HashMap<PathBuf, Arc<Vec<i16>>> {
        &self.cache
    }

    pub fn needs_rebuild(&self, state: &VideoEditorState) -> bool {
        !self.built_once || self.signature != StemSignature::capture(state)
    }

    /// Applies volumes and mute flags to the live players. Electron sets
    /// `element.volume = enabled ? volume : 0` on the same state changes.
    pub fn apply_volumes(&self, state: &VideoEditorState) {
        let Some(output) = &self.output else {
            return;
        };
        if let Some(stem) = &output.system {
            stem.player.set_volume(program_volume(
                state.audio_style.system_audio_enabled,
                state.audio_style.system_audio_volume,
            ));
        }
        if let Some(stem) = &output.mic {
            stem.player.set_volume(program_volume(
                state.audio_style.mic_audio_enabled,
                state.audio_style.mic_audio_volume,
            ));
        }
        for stem in &output.music {
            let track = state.music_tracks.iter().find(|track| track.id == stem.id);
            let volume = track
                .map(|track| program_volume(track.enabled, track.volume))
                .unwrap_or(0.0);
            stem.stem.player.set_volume(volume);
        }
    }

    /// Installs freshly rendered stems, replacing the players. Keeps the
    /// previous transport state so a rebuild mid-playback resumes in place.
    /// A stem-less project records the build and stays silent without holding
    /// an audio device.
    pub fn install(&mut self, built: BuiltStems, state: &VideoEditorState, offset: f64) {
        if built.system.is_none() && built.mic.is_none() && built.music.is_empty() {
            self.cache = built.cache;
            self.signature = built.signature;
            self.built_once = true;
            self.output = None;
            return;
        }
        let device = match self.take_or_open_device() {
            Some(device) => device,
            None => {
                self.cache = built.cache;
                return;
            }
        };
        self.cache = built.cache;
        self.signature = built.signature;
        self.built_once = true;
        let playing = self.output.as_ref().is_some_and(|output| output.playing);
        let mut output = Output {
            system: None,
            mic: None,
            music: Vec::new(),
            anchor: None,
            playing,
            device,
        };
        if let Some(samples) = built.system {
            output.system = Some(connect(&output.device, samples));
        }
        if let Some(samples) = built.mic {
            output.mic = Some(connect(&output.device, samples));
        }
        for (id, samples) in built.music {
            output.music.push(MusicStem {
                id,
                stem: connect(&output.device, samples),
            });
        }
        self.output = Some(output);
        self.apply_volumes(state);
        if playing {
            self.seek_all(offset);
            self.set_playing(true);
        } else {
            self.pause_all();
        }
    }

    /// Follows the playhead while playing: starts, pauses through the silent
    /// first-frame section, and re-anchors on jumps or drift.
    pub fn transport(&mut self, playing: bool, offset: f64) {
        if !playing || offset < 0.0 || !self.has_samples() {
            self.pause_all();
            return;
        }
        let Some(output) = self.output.as_mut() else {
            return;
        };
        match output.anchor {
            None => {
                self.seek_all(offset);
                self.set_playing(true);
            }
            Some((at, anchor)) => {
                let drift = (offset - anchor) - at.elapsed().as_secs_f64();
                if drift.abs() > RESYNC_THRESHOLD_SECONDS {
                    self.seek_all(offset);
                    self.set_playing(true);
                }
            }
        }
    }

    /// Plays the program stems at the scrub position while paused. Music
    /// stays silent, matching `useMusicPlayback`, which only syncs while
    /// playing.
    pub fn scrub_to(&mut self, offset: f64) {
        if !self.has_program() {
            return;
        }
        let Some(output) = self.output.as_mut() else {
            return;
        };
        output.anchor = None;
        output.playing = false;
        for stem in [&output.system, &output.mic].into_iter().flatten() {
            stem.play_from(offset);
        }
    }

    pub fn stop_scrub(&mut self) {
        let Some(output) = self.output.as_mut() else {
            return;
        };
        if output.playing {
            return;
        }
        if let Some(stem) = &output.system {
            stem.player.pause();
        }
        if let Some(stem) = &output.mic {
            stem.player.pause();
        }
    }

    pub fn pause_all(&mut self) {
        let Some(output) = self.output.as_mut() else {
            return;
        };
        output.anchor = None;
        output.playing = false;
        self.pause_players();
    }

    fn pause_players(&self) {
        let Some(output) = &self.output else {
            return;
        };
        for stem in [&output.system, &output.mic].into_iter().flatten() {
            stem.player.pause();
        }
        for stem in &output.music {
            stem.stem.player.pause();
        }
    }

    fn seek_all(&mut self, offset: f64) {
        let Some(output) = self.output.as_mut() else {
            return;
        };
        output.anchor = Some((Instant::now(), offset.max(0.0)));
        for stem in [&output.system, &output.mic].into_iter().flatten() {
            let _ = stem
                .player
                .try_seek(Duration::from_secs_f64(offset.max(0.0)));
        }
        for stem in &output.music {
            let _ = stem
                .stem
                .player
                .try_seek(Duration::from_secs_f64(offset.max(0.0)));
        }
    }

    fn set_playing(&mut self, playing: bool) {
        let Some(output) = self.output.as_mut() else {
            return;
        };
        output.playing = playing;
        if !playing {
            self.pause_players();
            return;
        }
        for stem in [&output.system, &output.mic].into_iter().flatten() {
            stem.player.play();
        }
        for stem in &output.music {
            stem.stem.player.play();
        }
    }

    fn has_samples(&self) -> bool {
        self.has_program() || self.has_music()
    }

    fn has_program(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| output.system.is_some() || output.mic.is_some())
    }

    fn has_music(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| !output.music.is_empty())
    }

    fn take_or_open_device(&mut self) -> Option<MixerDeviceSink> {
        if let Some(output) = self.output.take() {
            return Some(output.device);
        }
        match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(device) => Some(device),
            Err(error) => {
                if !self.device_error_logged {
                    self.device_error_logged = true;
                    eprintln!("[preview-audio] no audio device: {error}");
                }
                None
            }
        }
    }
}

fn connect(device: &MixerDeviceSink, samples: Arc<Vec<i16>>) -> Stem {
    let player = Player::connect_new(device.mixer());
    player.append(PcmSource::at(samples, Duration::ZERO));
    player.pause();
    Stem { player }
}

fn program_volume(enabled: bool, volume: f64) -> f32 {
    if !enabled {
        return 0.0;
    }
    volume.clamp(0.0, 4.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_volume_mutes_when_disabled() {
        assert_eq!(program_volume(false, 1.0), 0.0);
        assert_eq!(program_volume(true, 0.75), 0.75);
        assert_eq!(program_volume(true, 9.0), 4.0);
        assert_eq!(program_volume(true, -1.0), 0.0);
    }

    #[test]
    fn pcm_source_converts_and_seeks_on_frame_boundaries() {
        let samples = Arc::new(vec![0i16, 32767, -32768, 16384]);
        let mut source = PcmSource::at(samples, Duration::ZERO);
        assert_eq!(source.channels(), channels());
        assert_eq!(source.sample_rate(), sample_rate());
        assert_eq!(source.next(), Some(0.0));
        assert!((source.next().expect("sample") - 0.9999695).abs() < 1e-6);
        source
            .try_seek(Duration::from_secs_f64(1.0 / AUDIO_SAMPLE_RATE as f64))
            .expect("seek");
        assert_eq!(source.next(), Some(-1.0));
        assert_eq!(source.next(), Some(0.5));
        assert_eq!(source.next(), None);
    }

    #[test]
    fn stem_signature_ignores_volumes() {
        let first = VideoEditorState {
            segments: vec![Segment::spanning(4.0)],
            ..VideoEditorState::default()
        };
        let mut second = first.clone();
        second.audio_style.system_audio_volume = 0.25;
        second.audio_style.mic_audio_enabled = false;
        assert_eq!(
            StemSignature::capture(&first),
            StemSignature::capture(&second)
        );
        second.segments[0].original_end = 3.0;
        assert_ne!(
            StemSignature::capture(&first),
            StemSignature::capture(&second)
        );
    }
}
