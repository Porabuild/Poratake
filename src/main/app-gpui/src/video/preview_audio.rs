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
use crate::video::audio_tracks::{self, VolumeSource};
use crate::video::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::video::{keyboard_audio, sidecars};
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

pub struct RebuildInputs {
    project: PathBuf,
    stems: Vec<audio_tracks::Stem>,
    keyboard: Option<KeyboardRequest>,
    cache: HashMap<PathBuf, Arc<Vec<i16>>>,
    signature: StemSignature,
}

struct KeyboardRequest {
    segments: Vec<Segment>,
    sound_type: String,
    duration: f64,
}

impl RebuildInputs {
    pub fn capture(
        project: &Path,
        state: &VideoEditorState,
        cache: &HashMap<PathBuf, Arc<Vec<i16>>>,
    ) -> Self {
        let sources = audio_tracks::Sources::resolve(project);
        let duration = timeline_duration(state);
        let keyboard = (VolumeSource::Keyboard.resolve(state) > 0.0).then(|| KeyboardRequest {
            segments: state.segments.clone(),
            sound_type: state.audio_style.keyboard_sound_type.clone(),
            duration,
        });
        Self {
            project: project.to_path_buf(),
            stems: audio_tracks::stems(&sources, state, state.source_duration.unwrap_or(0.0)),
            keyboard,
            cache: cache.clone(),
            signature: StemSignature::capture(state),
        }
    }
}

fn timeline_duration(state: &VideoEditorState) -> f64 {
    crate::video::composition::segments::total_duration(
        &state.segments,
        state.source_duration.unwrap_or(0.0),
    )
}

pub struct BuiltStem {
    pub volume: VolumeSource,
    pub program: bool,
    pub samples: Arc<Vec<i16>>,
}

/// Decoded stems ready to install, plus the source cache the next rebuild reuses.
pub struct BuiltStems {
    pub stems: Vec<BuiltStem>,
    pub cache: HashMap<PathBuf, Arc<Vec<i16>>>,
    pub signature: StemSignature,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StemSignature {
    segments: Vec<Segment>,
    music_tracks: Vec<MusicTrack>,
    source_duration: Option<f64>,
    system_audio_enabled: bool,
    mic_audio_enabled: bool,
    keyboard_sound_enabled: bool,
    keyboard_sound_type: String,
}

impl StemSignature {
    pub fn capture(state: &VideoEditorState) -> Self {
        Self {
            segments: state.segments.clone(),
            music_tracks: state.music_tracks.clone(),
            source_duration: state.source_duration,
            system_audio_enabled: state.audio_style.system_audio_enabled,
            mic_audio_enabled: state.audio_style.mic_audio_enabled,
            keyboard_sound_enabled: state.audio_style.keyboard_sound_enabled,
            keyboard_sound_type: state.audio_style.keyboard_sound_type.clone(),
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
    let signature = inputs.signature;
    let mut cache = inputs.cache;
    let mut stems = Vec::new();
    for stem in inputs.stems {
        let Some(samples) = decode_cached(&stem.path, &mut cache) else {
            continue;
        };
        stems.push(BuiltStem {
            volume: stem.volume,
            program: stem.program,
            samples: Arc::new(audio::place(&samples, &stem.placement)),
        });
    }

    if let Some(keyboard) = inputs.keyboard {
        let clicks = sidecars::load_keyboard(&inputs.project).and_then(|data| {
            keyboard_audio::render(
                &data,
                &keyboard.segments,
                &keyboard.sound_type,
                keyboard.duration,
            )
        });
        if let Some(samples) = clicks {
            stems.push(BuiltStem {
                volume: VolumeSource::Keyboard,
                program: false,
                samples: Arc::new(samples),
            });
        }
    }

    BuiltStems {
        stems,
        cache,
        signature,
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

struct Playing {
    volume: VolumeSource,
    program: bool,
    stem: Stem,
}

struct Output {
    stems: Vec<Playing>,
    anchor: Option<(Instant, f64)>,
    playing: bool,
    device: MixerDeviceSink,
}

impl Output {
    fn program(&self) -> impl Iterator<Item = &Playing> {
        self.stems.iter().filter(|stem| stem.program)
    }
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

    /// `element.volume = enabled ? volume : 0` on the same state changes.
    pub fn apply_volumes(&self, state: &VideoEditorState) {
        let Some(output) = &self.output else {
            return;
        };
        for stem in &output.stems {
            stem.stem
                .player
                .set_volume(stem.volume.resolve(state).clamp(0.0, 4.0) as f32);
        }
    }

    /// Installs freshly rendered stems, replacing the players. Keeps the
    /// previous transport state so a rebuild mid-playback resumes in place.
    /// A stem-less project records the build and stays silent without holding
    /// an audio device.
    pub fn install(&mut self, built: BuiltStems, state: &VideoEditorState, offset: f64) {
        if built.stems.is_empty() {
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
            stems: Vec::new(),
            anchor: None,
            playing,
            device,
        };
        for stem in built.stems {
            let player = connect(&output.device, stem.samples);
            output.stems.push(Playing {
                volume: stem.volume,
                program: stem.program,
                stem: player,
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
        for stem in output.stems.iter().filter(|stem| stem.program) {
            stem.stem.play_from(offset);
        }
    }

    pub fn stop_scrub(&mut self) {
        let Some(output) = self.output.as_mut() else {
            return;
        };
        if output.playing {
            return;
        }
        for stem in output.stems.iter().filter(|stem| stem.program) {
            stem.stem.player.pause();
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
        for stem in &output.stems {
            stem.stem.player.pause();
        }
    }

    fn seek_all(&mut self, offset: f64) {
        let Some(output) = self.output.as_mut() else {
            return;
        };
        output.anchor = Some((Instant::now(), offset.max(0.0)));
        for stem in &output.stems {
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
        for stem in &output.stems {
            stem.stem.player.play();
        }
    }

    fn has_samples(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| !output.stems.is_empty())
    }

    fn has_program(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| output.program().next().is_some())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stem_volume_follows_the_model_it_came_from() {
        let mut state = VideoEditorState::default();
        state.audio_style.system_audio_volume = 0.75;
        assert_eq!(VolumeSource::SystemAudio.resolve(&state), 0.75);
        state.audio_style.system_audio_enabled = false;
        assert_eq!(VolumeSource::SystemAudio.resolve(&state), 0.0);
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

    fn project_with_every_source() -> (tempfile::TempDir, PathBuf, VideoEditorState) {
        let directory = tempfile::tempdir().expect("temp directory");
        let project = directory.path().join("Take 1.poratake");
        std::fs::create_dir(&project).expect("project folder");
        std::fs::create_dir(project.join("music")).expect("music folder");
        for file in ["system.m4a", "mic.m4a", "recording.mov", "keys.json"] {
            std::fs::write(project.join(file), []).expect("source file");
        }
        std::fs::write(project.join("music/song.mp3"), []).expect("music file");

        let state = VideoEditorState {
            segments: vec![Segment::spanning(4.0)],
            source_duration: Some(4.0),
            music_tracks: vec![MusicTrack {
                id: "music-song".into(),
                file_name: "song.mp3".into(),
                end_time: 4.0,
                original_duration: 4.0,
                ..MusicTrack::default()
            }],
            ..VideoEditorState::default()
        };
        (directory, project, state)
    }

    #[test]
    fn the_preview_renders_the_same_stem_set_the_export_mixes() {
        let (_directory, project, mut state) = project_with_every_source();
        state.audio_style.keyboard_sound_enabled = true;

        let inputs = RebuildInputs::capture(&project, &state, &HashMap::new());
        let sources = audio_tracks::Sources::resolve(&project);
        let expected = audio_tracks::stems(&sources, &state, 4.0);

        assert_eq!(inputs.stems, expected);
        assert_eq!(inputs.stems.len(), 3);
        assert!(inputs
            .stems
            .iter()
            .any(|stem| stem.volume == VolumeSource::SystemAudio));
        assert!(inputs
            .stems
            .iter()
            .any(|stem| stem.volume == VolumeSource::MicAudio));
        assert!(inputs
            .stems
            .iter()
            .any(|stem| stem.volume == VolumeSource::Track("music-song".into())));
        assert!(inputs.keyboard.is_some());
    }

    #[test]
    fn a_muted_music_track_leaves_the_preview_as_it_leaves_the_export() {
        let (_directory, project, mut state) = project_with_every_source();
        state.music_tracks[0].enabled = false;
        let inputs = RebuildInputs::capture(&project, &state, &HashMap::new());
        assert!(inputs
            .stems
            .iter()
            .all(|stem| stem.volume != VolumeSource::Track("music-song".into())));
    }

    #[test]
    fn a_recording_without_a_system_stem_still_has_program_audio() {
        let (_directory, project, state) = project_with_every_source();
        std::fs::remove_file(project.join("system.m4a")).expect("remove system stem");
        std::fs::remove_file(project.join("mic.m4a")).expect("remove mic stem");

        let inputs = RebuildInputs::capture(&project, &state, &HashMap::new());
        let program: Vec<_> = inputs.stems.iter().filter(|stem| stem.program).collect();
        assert_eq!(program.len(), 1);
        assert!(program[0].path.ends_with("recording.mov"));
    }

    #[test]
    fn the_keyboard_click_track_follows_its_setting() {
        let (_directory, project, mut state) = project_with_every_source();
        assert!(RebuildInputs::capture(&project, &state, &HashMap::new())
            .keyboard
            .is_none());
        state.audio_style.keyboard_sound_enabled = true;
        assert!(RebuildInputs::capture(&project, &state, &HashMap::new())
            .keyboard
            .is_some());
        state.audio_style.keyboard_sound_volume = 0.0;
        assert!(RebuildInputs::capture(&project, &state, &HashMap::new())
            .keyboard
            .is_none());
    }

    #[test]
    fn stem_signature_ignores_volumes() {
        let first = VideoEditorState {
            segments: vec![Segment::spanning(4.0)],
            ..VideoEditorState::default()
        };
        let mut second = first.clone();
        second.audio_style.system_audio_volume = 0.25;
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
