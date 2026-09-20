use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use lofty::config::{ParseOptions, ParsingMode};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Duration,
    pub artwork: Option<Arc<[u8]>>,
    pub year: Option<u32>,
    /// Last modified time of the file, used for the "Recently added" ordering.
    pub added: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug, Default)]
pub struct ScanResult {
    pub tracks: Vec<Track>,
    pub errors: Vec<String>,
}

pub fn default_music_folder() -> Option<PathBuf> {
    dirs::audio_dir()
}

pub struct PreparedAudio {
    pub track: Track,
    remove_on_drop: bool,
}

impl PreparedAudio {
    pub fn keep(&mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for PreparedAudio {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_file(&self.track.path);
        }
    }
}

struct TemporaryAudio(PathBuf);

impl Drop for TemporaryAudio {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn audio_filename(title: &str, artist: &str) -> String {
    let clean = |value: &str| {
        let value: String = value
            .chars()
            .map(|c| {
                if c.is_control() || "\\/:*?\"<>|".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut bytes = 0;
        let value: String = value
            .trim_matches(['.', ' '])
            .chars()
            .take(60)
            .take_while(|c| {
                bytes += c.len_utf8();
                bytes <= 90
            })
            .collect();
        let value = value.trim_end_matches(['.', ' ']);
        let stem = value.split('.').next().unwrap_or("").to_ascii_uppercase();
        if value.is_empty() {
            "track".to_owned()
        } else if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|suffix| {
                    matches!(
                        suffix,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            })
        {
            format!("_{value}")
        } else {
            value.to_owned()
        }
    };
    format!("{} - {}", clean(artist), clean(title))
}

fn publish_audio(temp: &Path, folder: &Path, stem: &str, extension: &str) -> Result<PathBuf> {
    for number in 0..10_000 {
        let name = if number == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} ({number}).{extension}")
        };
        let destination = folder.join(name);
        match fs::hard_link(temp, &destination) {
            Ok(()) => return Ok(destination),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => bail!("Could not publish audio atomically. The destination must support hard links and allow writing."),
        }
    }
    bail!("Too many files share this audio filename.")
}

fn validate_prepared_audio(path: &Path, cancel: &std::sync::atomic::AtomicBool) -> Result<()> {
    let decoder = decode_file(path)
        .map_err(|_| anyhow!("Downloaded data could not be decoded as playable audio."))?;
    let rate = u64::from(decoder.sample_rate());
    let channels = u64::from(decoder.channels());
    if rate == 0 || rate > 384_000 || channels == 0 || channels > 8 {
        bail!("Downloaded audio has unsupported sample parameters.");
    }
    let started = std::time::Instant::now();
    let maximum = rate * channels * 60 * 60 * 3;
    let mut samples = 0_u64;
    for sample in decoder {
        samples += 1;
        if !sample.is_finite() || samples > maximum {
            bail!("Downloaded audio is invalid or exceeds the three-hour limit.");
        }
        if samples.is_multiple_of(65_536) {
            crate::api::check_cancel(cancel)?;
            if started.elapsed() > Duration::from_secs(60) {
                bail!("Audio validation timed out.");
            }
        }
    }
    crate::api::check_cancel(cancel)?;
    if samples == 0 {
        bail!("Downloaded audio contains no playable samples.");
    }
    Ok(())
}

pub fn prepare_discover_audio(
    source: &crate::api::Track,
    download: bool,
    cached: Option<&Path>,
    download_folder: Option<&Path>,
    cancel: &std::sync::atomic::AtomicBool,
    progress: &impl Fn(&str),
) -> Result<PreparedAudio> {
    use lofty::config::WriteOptions;
    use lofty::file::FileType;
    use lofty::tag::{Tag, TagExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    crate::api::check_cancel(cancel)?;
    let folder = if download {
        download_folder
            .map(Path::to_path_buf)
            .or_else(default_music_folder)
            .ok_or_else(|| anyhow!("Could not locate the download music folder."))?
    } else {
        dirs::cache_dir()
            .ok_or_else(|| anyhow!("Could not locate the audio cache folder."))?
            .join("noir-player")
            .join("discover")
    };
    fs::create_dir_all(&folder)
        .map_err(|_| anyhow!("Could not create the audio folder. Check permissions."))?;
    for ancestor in folder.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|_| anyhow!("Could not inspect the audio folder."))?;
        if is_link(&metadata) || !metadata.is_dir() {
            bail!("Audio destination must not contain links or non-directory components.");
        }
    }
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = folder.join(format!(
        ".noir-{}-{stamp}-{}.part",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| {
            anyhow!("Could not create a temporary audio file. Check free space and permissions.")
        })?;
    let temp = TemporaryAudio(path);
    let mut file = file;
    if let Some(cached) = cached {
        progress("Copying prepared audio...");
        use std::io::{Read, Write};
        let mut input = File::open(cached)
            .map_err(|_| anyhow!("Prepared audio is unavailable. Retry Play first."))?;
        let metadata = input
            .metadata()
            .map_err(|_| anyhow!("Could not inspect prepared audio."))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 100 * 1024 * 1024 {
            bail!("Prepared audio is empty or exceeds the 100 MiB limit.");
        }
        let mut received = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            crate::api::check_cancel(cancel)?;
            let count = input
                .read(&mut buffer)
                .map_err(|_| anyhow!("Could not read prepared audio."))?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if received > 100 * 1024 * 1024 {
                bail!("Prepared audio exceeds the 100 MiB limit.");
            }
            file.write_all(&buffer[..count])
                .map_err(|_| anyhow!("Could not copy prepared audio."))?;
        }
        if received != metadata.len() {
            bail!("Prepared audio changed while copying.");
        }
    } else {
        let url = crate::api::resolve_audio(source, cancel, progress)?;
        crate::api::download_audio(&url, &mut file, cancel, progress)?;
    }
    file.sync_all()
        .map_err(|_| anyhow!("Could not flush the temporary audio file."))?;
    drop(file);
    crate::api::check_cancel(cancel)?;
    progress("Validating audio and saving metadata...");
    validate_prepared_audio(&temp.0, cancel)?;
    let probe = Probe::open(&temp.0)
        .and_then(|probe| Ok(probe.guess_file_type()?))
        .map_err(|_| anyhow!("Could not identify the downloaded audio format."))?;
    let kind = probe
        .file_type()
        .ok_or_else(|| anyhow!("Unknown downloaded audio format."))?;
    let extension = match kind {
        FileType::Mpeg => "mp3",
        FileType::Flac => "flac",
        FileType::Wav => "wav",
        FileType::Vorbis => "ogg",
        FileType::Mp4 => "m4a",
        FileType::Aac => "aac",
        FileType::Aiff => "aiff",
        _ => bail!("Downloaded audio uses an unsupported format."),
    };
    let mut tag = Tag::new(kind.primary_tag_type());
    tag.set_title(source.name.clone());
    tag.set_artist(source.artist.clone());
    tag.save_to_path(&temp.0, WriteOptions::default())
        .map_err(|_| {
            anyhow!("Could not preserve title and artist in downloaded audio metadata.")
        })?;
    let duration = decoded_duration(&temp.0)
        .map_err(|_| anyhow!("Audio validation failed after writing metadata."))?;
    let mut track =
        read_track(&temp.0).map_err(|_| anyhow!("Could not read prepared audio metadata."))?;
    track.title = source.name.clone();
    track.artist = source.artist.clone();
    track.duration = duration;
    fs::OpenOptions::new()
        .write(true)
        .open(&temp.0)
        .and_then(|file| file.sync_all())
        .map_err(|_| anyhow!("Could not flush prepared audio."))?;
    crate::api::check_cancel(cancel)?;
    track.path = publish_audio(
        &temp.0,
        &folder,
        &audio_filename(&source.name, &source.artist),
        extension,
    )?;
    let prepared = PreparedAudio {
        track,
        remove_on_drop: true,
    };
    crate::api::check_cancel(cancel)?;
    Ok(prepared)
}

pub fn scan_folder(folder: &Path) -> ScanResult {
    let mut result = ScanResult::default();
    match fs::symlink_metadata(folder) {
        Ok(metadata) if is_link(&metadata) || !metadata.is_dir() => {
            result.errors.push(format!(
                "{}: expected a directory that is not a link",
                folder.display()
            ));
            return result;
        }
        Err(error) => {
            result.errors.push(format!("{}: {error}", folder.display()));
            return result;
        }
        Ok(_) => {}
    }

    let entries = WalkDir::new(folder)
        .follow_links(false)
        .follow_root_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            !entry.path_is_symlink()
                && entry
                    .metadata()
                    .map(|metadata| !is_link(&metadata))
                    .unwrap_or(true)
        });

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                result.errors.push(error.to_string());
                continue;
            }
        };
        if !entry.file_type().is_file() || !supported_extension(entry.path()) {
            continue;
        }
        match read_track(entry.path()) {
            Ok(track) => result.tracks.push(track),
            Err(error) => result.errors.push(format!("{error:#}")),
        }
    }

    result.tracks.sort_by_cached_key(|track| {
        (
            track.title.to_lowercase(),
            track.artist.to_lowercase(),
            track.album.to_lowercase(),
            track.path.clone(),
        )
    });
    result.errors.sort();
    result
}

pub fn scan_folders(folders: &[PathBuf]) -> ScanResult {
    let mut result = ScanResult::default();
    let mut seen = std::collections::HashSet::new();
    for folder in folders {
        let sub = scan_folder(folder);
        result.errors.extend(sub.errors);
        for track in sub.tracks {
            if seen.insert(track.path.clone()) {
                result.tracks.push(track);
            }
        }
    }
    result.tracks.sort_by_cached_key(|track| {
        (
            track.title.to_lowercase(),
            track.artist.to_lowercase(),
            track.album.to_lowercase(),
            track.path.clone(),
        )
    });
    result.errors.sort();
    result.errors.dedup();
    result
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp3" | "flac" | "wav" | "ogg" | "oga" | "m4a" | "mp4" | "aac" | "aif" | "aiff"
            )
        })
}

fn read_track(path: &Path) -> Result<Track> {
    let tagged = Probe::open(path)
        .map_err(|error| anyhow!("{}: {error}", path.display()))
        .and_then(|probe| {
            probe
                .guess_file_type()
                .map_err(|error| anyhow!("{}: {error}", path.display()))
        })
        .and_then(|probe| {
            probe
                .options(ParseOptions::new().parsing_mode(ParsingMode::Relaxed))
                .read()
                .map_err(|error| anyhow!("{}: {error}", path.display()))
        });
    let tagged = match tagged {
        Ok(tagged) => tagged,
        Err(metadata_error) => {
            let duration = decoded_duration(path).with_context(|| {
                format!(
                    "{}: failed to read audio metadata ({metadata_error}) and validate audio",
                    path.display()
                )
            })?;
            return Ok(Track {
                path: path.to_path_buf(),
                title: filename_title(path),
                artist: "Unknown Artist".to_owned(),
                album: "Unknown Album".to_owned(),
                duration,
                artwork: None,
                year: None,
                added: file_added(path),
            });
        }
    };
    let duration = if tagged.properties().duration().is_zero()
        || tagged.properties().sample_rate().unwrap_or(0) == 0
        || tagged.properties().channels().unwrap_or(0) == 0
    {
        decoded_duration(path)?
    } else {
        tagged.properties().duration()
    };
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let title = tag
        .and_then(|tag| tag.title())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.into_owned())
        .unwrap_or_else(|| filename_title(path));
    let artist = tag
        .and_then(|tag| tag.artist())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.into_owned())
        .unwrap_or_else(|| "Unknown Artist".to_owned());
    let album = tag
        .and_then(|tag| tag.album())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.into_owned())
        .unwrap_or_else(|| "Unknown Album".to_owned());
    let artwork = tag
        .and_then(|tag| {
            tag.pictures()
                .iter()
                .find(|picture| picture.pic_type() == PictureType::CoverFront)
                .or_else(|| tag.pictures().first())
        })
        .map(|picture| Arc::from(picture.data()));

    let year = tag.and_then(|tag| tag.year()).filter(|year| *year > 0);

    Ok(Track {
        path: path.to_path_buf(),
        title,
        artist,
        album,
        duration,
        artwork,
        year,
        added: file_added(path),
    })
}

/// Modified time of a library file, used to order "Recently added".
fn file_added(path: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// Reads lyrics embedded in a file's tags. Returns `None` when the file has no
/// lyrics tag, or cannot be read.
pub fn read_lyrics(path: &Path) -> Option<String> {
    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().parsing_mode(ParsingMode::Relaxed))
        .read()
        .ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    tag.get_string(&ItemKey::Lyrics)
        .map(str::trim)
        .filter(|lyrics| !lyrics.is_empty())
        .map(ToOwned::to_owned)
}

fn filename_title(path: &Path) -> String {
    path.file_stem()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn decoded_duration(path: &Path) -> Result<Duration> {
    let mut decoder = decode_file(path)?;
    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels();
    if sample_rate == 0 || channels == 0 || decoder.next().is_none() {
        bail!("{}: no playable audio samples", path.display());
    }
    if let Some(duration) = decoder
        .total_duration()
        .filter(|duration| !duration.is_zero())
    {
        return Ok(duration);
    }
    let samples = decoder.fold(1_u64, |count, _| count + 1);
    Ok(Duration::from_secs_f64(
        samples as f64 / f64::from(sample_rate) / f64::from(channels),
    ))
}

pub const EQ_FREQUENCIES: [f32; 5] = [60.0, 230.0, 910.0, 3600.0, 14000.0];

pub const EQ_PRESETS: &[(&str, [f32; 5])] = &[
    ("Flat", [0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Bass Boost", [6.0, 4.0, 1.0, 0.0, 0.0]),
    ("Treble Boost", [0.0, 0.0, 1.0, 4.0, 6.0]),
    ("Vocal", [-2.0, 1.0, 5.0, 3.0, 0.0]),
    ("Rock", [5.0, 3.0, -1.0, 3.0, 5.0]),
    ("Pop", [2.0, 4.0, 3.0, 1.0, -1.0]),
];

#[derive(Clone, Debug, PartialEq)]
pub struct EqualizerState {
    pub enabled: bool,
    pub gains: [f32; 5],
}

impl Default for EqualizerState {
    fn default() -> Self {
        Self {
            enabled: false,
            gains: [0.0; 5],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoeffs {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }

    pub fn peaking(f0: f32, gain_db: f32, sample_rate: f32, q: f32) -> Self {
        if gain_db.abs() < 0.05 || f0 <= 0.0 || sample_rate <= 0.0 || f0 >= sample_rate * 0.49 {
            return Self::identity();
        }
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * (f0 / sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BiquadChannel {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadChannel {
    #[inline]
    pub fn process(&mut self, input: f32, coeffs: &BiquadCoeffs) -> f32 {
        let output = coeffs.b0 * input + coeffs.b1 * self.x1 + coeffs.b2 * self.x2
            - coeffs.a1 * self.y1
            - coeffs.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

pub struct EqualizerSource<I> {
    input: I,
    channels: u16,
    sample_rate: u32,
    current_channel: usize,
    filters: Vec<Vec<BiquadChannel>>,
    coeffs: [BiquadCoeffs; 5],
    cached_gains: [f32; 5],
    state: Arc<std::sync::RwLock<EqualizerState>>,
}

impl<I> EqualizerSource<I>
where
    I: Source<Item = f32>,
{
    pub fn new(input: I, state: Arc<std::sync::RwLock<EqualizerState>>) -> Self {
        let channels = input.channels().max(1);
        let sample_rate = input.sample_rate().max(1);
        let mut filters = Vec::with_capacity(5);
        for _ in 0..5 {
            filters.push(vec![BiquadChannel::default(); channels as usize]);
        }
        let initial_gains = state.read().map(|s| s.gains).unwrap_or([0.0; 5]);
        let mut coeffs = [BiquadCoeffs::identity(); 5];
        for b in 0..5 {
            coeffs[b] =
                BiquadCoeffs::peaking(EQ_FREQUENCIES[b], initial_gains[b], sample_rate as f32, 1.0);
        }
        Self {
            input,
            channels,
            sample_rate,
            current_channel: 0,
            filters,
            coeffs,
            cached_gains: initial_gains,
            state,
        }
    }
}

impl<I> Source for EqualizerSource<I>
where
    I: Source<Item = f32>,
{
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.channels
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    #[inline]
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.current_channel = 0;
        for band in &mut self.filters {
            for ch in band {
                *ch = BiquadChannel::default();
            }
        }
        self.input.try_seek(pos)
    }
}

impl<I> Iterator for EqualizerSource<I>
where
    I: Source<Item = f32>,
{
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        let sample = self.input.next()?;

        if self.current_channel == 0 {
            if let Ok(state) = self.state.read() {
                if !state.enabled {
                    self.current_channel = (self.current_channel + 1) % (self.channels as usize);
                    return Some(sample);
                }
                if state.gains != self.cached_gains {
                    self.cached_gains = state.gains;
                    for (b, &freq) in EQ_FREQUENCIES.iter().enumerate() {
                        self.coeffs[b] = BiquadCoeffs::peaking(
                            freq,
                            self.cached_gains[b],
                            self.sample_rate as f32,
                            1.0,
                        );
                    }
                }
            }
        }

        let ch = self.current_channel;
        self.current_channel = (ch + 1) % (self.channels as usize);

        let mut out = sample;
        for b in 0..5 {
            if self.cached_gains[b].abs() >= 0.05 {
                out = self.filters[b][ch].process(out, &self.coeffs[b]);
            }
        }

        Some(out.clamp(-1.0, 1.0))
    }
}

pub struct MediaPlayer {
    sink: Sink,
    stream: OutputStream,
    duration: Option<Duration>,
    loaded: bool,
    pub eq_state: Arc<std::sync::RwLock<EqualizerState>>,
}

impl MediaPlayer {
    pub fn new() -> Result<Self> {
        let stream = OutputStreamBuilder::open_default_stream()
            .context("Failed to open an audio output device")?;
        let sink = Sink::connect_new(stream.mixer());
        sink.pause();
        let eq_state = Arc::new(std::sync::RwLock::new(EqualizerState::default()));
        Ok(Self {
            sink,
            stream,
            duration: None,
            loaded: false,
            eq_state,
        })
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let decoder = decode_file(path)?;
        let duration = decoder
            .total_duration()
            .filter(|duration| !duration.is_zero())
            .or_else(|| {
                lofty::read_from_path(path)
                    .ok()
                    .map(|tagged| tagged.properties().duration())
                    .filter(|duration| !duration.is_zero())
            });
        let sink = Sink::connect_new(self.stream.mixer());
        sink.pause();
        sink.set_volume(self.sink.volume());
        let eq_source = EqualizerSource::new(decoder, self.eq_state.clone());
        sink.append(eq_source);
        self.sink.stop();
        self.sink = sink;
        self.duration = duration;
        self.loaded = true;
        Ok(())
    }

    pub fn play(&self) {
        self.sink.play();
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    pub fn position(&self) -> Duration {
        if self.is_finished() {
            self.duration.unwrap_or_else(|| self.sink.get_pos())
        } else {
            self.sink.get_pos()
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn is_finished(&self) -> bool {
        self.loaded && self.sink.empty()
    }

    #[allow(dead_code)]
    pub fn seek(&self, position: Duration) -> Result<()> {
        if !self.loaded || self.sink.empty() {
            bail!("No active track to seek; load a track first");
        }
        let position = self
            .duration
            .map_or(position, |duration| position.min(duration));
        self.sink
            .try_seek(position)
            .map_err(|error| anyhow!("Failed to seek audio: {error}"))
    }

    pub fn skip_forward(&self, offset: Duration) -> Result<()> {
        if !self.loaded || self.sink.empty() {
            bail!("No active track to seek; load a track first");
        }
        let current = self.position();
        let duration = self.duration.unwrap_or(current + offset);
        let target = (current + offset).min(duration);
        self.seek(target)
    }

    pub fn skip_backward(&self, offset: Duration) -> Result<()> {
        if !self.loaded || self.sink.empty() {
            bail!("No active track to seek; load a track first");
        }
        let current = self.position();
        let target = current.saturating_sub(offset);
        self.seek(target)
    }

    pub fn set_equalizer_enabled(&self, enabled: bool) {
        if let Ok(mut state) = self.eq_state.write() {
            state.enabled = enabled;
        }
    }

    pub fn set_equalizer_gains(&self, gains: [f32; 5]) {
        if let Ok(mut state) = self.eq_state.write() {
            state.gains = gains;
        }
    }

    #[allow(dead_code)]
    pub fn set_equalizer_band(&self, band: usize, gain_db: f32) {
        if band < 5 {
            if let Ok(mut state) = self.eq_state.write() {
                state.gains[band] = gain_db.clamp(-12.0, 12.0);
            }
        }
    }

    #[allow(dead_code)]
    pub fn equalizer_state(&self) -> EqualizerState {
        self.eq_state.read().map(|s| s.clone()).unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn volume(&self) -> f32 {
        self.sink.volume()
    }

    pub fn set_volume(&self, volume: f32) {
        if volume.is_finite() {
            self.sink.set_volume(volume.clamp(0.0, 1.0));
        }
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) {
        let volume = self.sink.volume();
        self.sink.stop();
        self.sink = Sink::connect_new(self.stream.mixer());
        self.sink.set_volume(volume);
        self.sink.pause();
        self.duration = None;
        self.loaded = false;
    }
}

fn decode_file(path: &Path) -> Result<Decoder<BufReader<File>>> {
    let file =
        File::open(path).with_context(|| format!("{}: failed to open audio", path.display()))?;
    if !file.metadata()?.is_file() {
        bail!("{}: expected a regular audio file", path.display());
    }
    Decoder::try_from(file).with_context(|| format!("{}: failed to decode audio", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "noir-media-{}-{timestamp}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_wav(path: &Path) {
        let sample_rate = 8_000_u32;
        let data_size = sample_rate * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.resize(44 + data_size as usize, 0);
        fs::write(path, bytes).unwrap();
    }

    fn timestamp_tag(timestamp: &[u8]) -> Vec<u8> {
        let mut frames = Vec::new();
        for (id, value) in [
            (b"TIT2", b"\x03Tagged title".as_slice()),
            (b"TPE1", b"\x03Tagged artist".as_slice()),
            (b"TALB", b"\x03Tagged album".as_slice()),
            (b"TDRC", timestamp),
        ] {
            assert!(value.len() < 128);
            frames.extend_from_slice(id);
            frames.extend_from_slice(&[0, 0, 0, value.len() as u8, 0, 0]);
            frames.extend_from_slice(value);
        }
        assert!(frames.len() < 128);
        let mut bytes = b"ID3\x04\x00\x00\x00\x00\x00".to_vec();
        bytes.push(frames.len() as u8);
        bytes.extend_from_slice(&frames);
        bytes
    }

    fn write_tagged_mp3(path: &Path, timestamp: &[u8]) {
        let mut bytes = timestamp_tag(timestamp);
        for _ in 0..8 {
            let mut frame = [0_u8; 417];
            frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0xc0]);
            bytes.extend_from_slice(&frame);
        }
        fs::write(path, bytes).unwrap();
        let mut decoder = decode_file(path).unwrap();
        assert_eq!(decoder.sample_rate(), 44_100);
        assert_eq!(decoder.channels(), 1);
        assert_eq!(decoder.next(), Some(0.0));
    }

    #[test]
    fn download_filenames_are_portable_and_keep_unicode() {
        assert_eq!(audio_filename("Song", "Artist"), "Artist - Song");
        assert_eq!(audio_filename("CON", "LPT1"), "_LPT1 - _CON");
        assert_eq!(audio_filename("... ", " "), "track - track");
        assert_eq!(audio_filename("夜の歌", "Björk"), "Björk - 夜の歌");
        let name = audio_filename("../../bad: song?\u{0}\n", " A  B. ");
        assert!(!name
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c)));
        assert!(!name.ends_with(['.', ' ']));
        assert!(
            audio_filename(&"夜".repeat(200), &"a".repeat(200))
                .chars()
                .count()
                <= 123
        );
    }

    #[test]
    fn publishing_never_overwrites_and_temporary_files_are_removed() {
        let temp = TempDir::new();
        let partial = TemporaryAudio(temp.0.join("audio.part"));
        fs::write(&partial.0, b"prepared bytes").unwrap();
        let first = publish_audio(&partial.0, &temp.0, "Artist - Song", "mp3").unwrap();
        let second = publish_audio(&partial.0, &temp.0, "Artist - Song", "mp3").unwrap();
        assert_eq!(first.file_name().unwrap(), "Artist - Song.mp3");
        assert_eq!(second.file_name().unwrap(), "Artist - Song (1).mp3");
        let partial_path = partial.0.clone();
        drop(partial);
        assert!(!partial_path.exists());
        assert_eq!(fs::read(first).unwrap(), b"prepared bytes");
        assert_eq!(fs::read(second).unwrap(), b"prepared bytes");
    }

    #[test]
    fn malformed_timestamp_keeps_playable_mp3_and_other_metadata() {
        let temp = TempDir::new();
        let path = temp.0.join("timestamp.mp3");
        write_tagged_mp3(&path, b"\x03not-a-date");
        assert!(Probe::open(&path).unwrap().read().is_err());
        let result = scan_folder(&temp.0);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.tracks.len(), 1);
        let track = &result.tracks[0];
        assert_eq!(track.path, path);
        assert_eq!(track.title, "Tagged title");
        assert_eq!(track.artist, "Tagged artist");
        assert_eq!(track.album, "Tagged album");
        assert!(!track.duration.is_zero());
        assert!(track.artwork.is_none());
    }

    #[test]
    fn undecodable_timestamp_text_uses_audio_fallback() {
        let temp = TempDir::new();
        let path = temp.0.join("fallback.mp3");
        write_tagged_mp3(&path, b"\x03\xff");
        assert!(Probe::open(&path)
            .unwrap()
            .options(ParseOptions::new().parsing_mode(ParsingMode::Relaxed))
            .read()
            .is_err());
        let result = scan_folder(&temp.0);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.tracks.len(), 1);
        let track = &result.tracks[0];
        assert_eq!(track.path, path);
        assert_eq!(track.title, "fallback");
        assert_eq!(track.artist, "Unknown Artist");
        assert_eq!(track.album, "Unknown Album");
        assert!(!track.duration.is_zero());
        assert!(track.artwork.is_none());
    }

    #[test]
    fn malformed_timestamp_without_audio_is_rejected() {
        let temp = TempDir::new();
        for (name, timestamp) in [
            ("bad-date.mp3", b"\x03not-a-date".as_slice()),
            ("bad-text.mp3", b"\x03\xff".as_slice()),
        ] {
            fs::write(temp.0.join(name), timestamp_tag(timestamp)).unwrap();
        }
        let result = scan_folder(&temp.0);
        assert!(result.tracks.is_empty());
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn extensions_are_case_insensitive_and_limited_to_enabled_formats() {
        for extension in [
            "mp3", "FLAC", "Wav", "ogg", "oga", "m4a", "mp4", "aac", "aif", "aiff",
        ] {
            assert!(supported_extension(Path::new(&format!(
                "track.{extension}"
            ))));
        }
        for filename in [
            "track",
            "track.txt",
            "track.opus",
            "track.wma",
            "track.mp3.txt",
        ] {
            assert!(!supported_extension(Path::new(filename)));
        }
    }

    #[test]
    fn missing_folder_and_file_root_report_errors() {
        let temp = TempDir::new();
        let missing = scan_folder(&temp.0.join("missing"));
        assert!(missing.tracks.is_empty());
        assert_eq!(missing.errors.len(), 1);
        let path = temp.0.join("song.wav");
        write_wav(&path);
        let file_root = scan_folder(&path);
        assert!(file_root.tracks.is_empty());
        assert_eq!(file_root.errors.len(), 1);
    }

    #[test]
    fn empty_folder_has_no_errors() {
        let temp = TempDir::new();
        let result = scan_folder(&temp.0);
        assert!(result.tracks.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn invalid_audio_reports_errors_and_unsupported_files_are_ignored() {
        let temp = TempDir::new();
        fs::write(temp.0.join("broken.MP3"), b"not audio").unwrap();
        fs::write(temp.0.join("empty.flac"), []).unwrap();
        fs::write(temp.0.join("notes.txt"), b"not audio").unwrap();
        write_wav(&temp.0.join("valid.wav"));
        let result = scan_folder(&temp.0);
        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.errors.len(), 2);
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("broken.MP3")));
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("empty.flac")));
    }

    #[test]
    fn recursive_scan_uses_filename_fallback_and_deterministic_order() {
        let temp = TempDir::new();
        fs::create_dir(temp.0.join("nested")).unwrap();
        write_wav(&temp.0.join("Zulu.wav"));
        write_wav(&temp.0.join("nested").join("alpha.WAV"));
        write_wav(&temp.0.join("alpha.wav"));
        let result = scan_folder(&temp.0);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.tracks.len(), 3);
        assert_eq!(result.tracks[0].title, "alpha");
        assert_eq!(result.tracks[0].path, temp.0.join("alpha.wav"));
        assert_eq!(result.tracks[1].title, "alpha");
        assert_eq!(result.tracks[2].title, "Zulu");
        for track in &result.tracks {
            assert_eq!(track.artist, "Unknown Artist");
            assert_eq!(track.album, "Unknown Album");
            assert_eq!(track.duration, Duration::from_secs(1));
            assert!(track.artwork.is_none());
        }
        let again = scan_folder(&temp.0);
        assert_eq!(
            result
                .tracks
                .iter()
                .map(|track| &track.path)
                .collect::<Vec<_>>(),
            again
                .tracks
                .iter()
                .map(|track| &track.path)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn decoder_reads_real_samples_and_seeks_without_an_output_device() {
        let temp = TempDir::new();
        let path = temp.0.join("sound.wav");
        write_wav(&path);
        let mut decoder = decode_file(&path).unwrap();
        assert_eq!(decoder.total_duration(), Some(Duration::from_secs(1)));
        assert_eq!(decoder.next(), Some(0.0));
        decoder.try_seek(Duration::from_millis(500)).unwrap();
        assert_eq!(decoder.next(), Some(0.0));
    }

    #[test]
    fn decoder_rejects_missing_and_invalid_files() {
        let temp = TempDir::new();
        assert!(decode_file(&temp.0.join("missing.wav")).is_err());
        let path = temp.0.join("broken.mp3");
        fs::write(&path, b"not audio").unwrap();
        assert!(decode_file(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn scan_does_not_follow_file_directory_or_root_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let outside = TempDir::new();
        write_wav(&outside.0.join("outside.wav"));
        symlink(&outside.0, temp.0.join("directory-link")).unwrap();
        symlink(outside.0.join("outside.wav"), temp.0.join("file-link.wav")).unwrap();
        symlink(&temp.0, temp.0.join("loop")).unwrap();
        let result = scan_folder(&temp.0);
        assert!(result.tracks.is_empty());
        assert!(result.errors.is_empty());
        let root = scan_folder(&temp.0.join("directory-link"));
        assert!(root.tracks.is_empty());
        assert_eq!(root.errors.len(), 1);
    }

    #[test]
    fn equalizer_coefficients_and_neutral_state() {
        let state = EqualizerState::default();
        assert!(!state.enabled);
        assert_eq!(state.gains, [0.0; 5]);

        let neutral = BiquadCoeffs::peaking(1000.0, 44100.0, 0.0, 1.0);
        let mut ch = BiquadChannel::default();
        assert_eq!(ch.process(0.5, &neutral), 0.5);

        let boost = BiquadCoeffs::peaking(1000.0, 44100.0, 6.0, 1.0);
        let out = ch.process(0.5, &boost);
        assert!(out.is_finite());
        assert_ne!(out, 0.0);
    }

    #[test]
    fn equalizer_source_processes_audio_and_seeks() {
        let temp = TempDir::new();
        let path = temp.0.join("eq_test.wav");
        write_wav(&path);
        let decoder = decode_file(&path).unwrap();

        let state = Arc::new(std::sync::RwLock::new(EqualizerState {
            enabled: true,
            gains: [6.0, 3.0, 0.0, -3.0, -6.0],
        }));

        let mut eq_source = EqualizerSource::new(decoder, state.clone());
        assert_eq!(eq_source.channels(), 1);
        assert_eq!(eq_source.sample_rate(), 8000);
        assert_eq!(eq_source.total_duration(), Some(Duration::from_secs(1)));

        let mut sample_count = 0;
        for _ in 0..100 {
            if let Some(s) = eq_source.next() {
                assert!(s.is_finite());
                sample_count += 1;
            }
        }
        assert_eq!(sample_count, 100);

        assert!(eq_source.try_seek(Duration::from_millis(200)).is_ok());
        let sample_after_seek = eq_source.next();
        assert!(sample_after_seek.is_some());
    }

    #[test]
    fn equalizer_presets_are_valid() {
        assert_eq!(EQ_FREQUENCIES.len(), 5);
        assert_eq!(EQ_FREQUENCIES, [60.0, 230.0, 910.0, 3600.0, 14000.0]);
        for (name, gains) in EQ_PRESETS {
            assert!(!name.is_empty());
            assert_eq!(gains.len(), 5);
            for &g in gains {
                assert!(
                    (-12.0..=12.0).contains(&g),
                    "Preset {name} gain out of bounds: {g}"
                );
            }
        }
    }

    #[test]
    fn multi_folder_scan_aggregates_and_deduplicates() {
        let temp1 = TempDir::new();
        let temp2 = TempDir::new();
        let path1 = temp1.0.join("track1.wav");
        let path2 = temp2.0.join("track2.wav");
        write_wav(&path1);
        write_wav(&path2);

        let res = scan_folders(&[temp1.0.clone(), temp2.0.clone(), temp1.0.clone()]);
        assert_eq!(res.tracks.len(), 2);
        assert!(res.errors.is_empty());
    }
}
