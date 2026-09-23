use std::borrow::Cow;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use image::{ImageDecoder, ImageEncoder};
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
    /// The cover re-encoded down to `HERO_EDGE`, kept only so the now-playing
    /// hero can be built on demand. A track never holds the original blob: a
    /// 3000px embedded cover would cost megabytes per album for pixels no part
    /// of the UI ever draws.
    pub artwork: Option<Arc<[u8]>>,
    /// The cover every list, card and thumbnail draws, decoded once at scan
    /// time and bounded to `COVER_EDGE`. Handing the renderer a decoded image
    /// keeps the pixels ours: an encoded `Image` is decoded into a global cache
    /// that is never evicted, so every cover drawn would be retained at its
    /// full embedded size for the life of the process.
    pub artwork_image: Option<Arc<gpui_kit::RenderImage>>,
    pub year: Option<u32>,
    /// Last modified time of the file, used for the "Recently added" ordering.
    pub added: Option<std::time::SystemTime>,
    /// Lowercased `title\0artist\0album`, built once so search allocates nothing.
    pub search_key: String,
}

impl Track {
    pub fn build_search_key(title: &str, artist: &str, album: &str) -> String {
        let mut key = String::with_capacity(title.len() + artist.len() + album.len() + 2);
        key.push_str(&title.to_lowercase());
        key.push('\0');
        key.push_str(&artist.to_lowercase());
        key.push('\0');
        key.push_str(&album.to_lowercase());
        key
    }

    pub fn rebuild_search_key(&mut self) {
        self.search_key = Self::build_search_key(&self.title, &self.artist, &self.album);
    }
}

/// Longest edge of the cover cached for every track. The largest cover the UI
/// draws outside the now-playing hero is a 158px card, and the extra headroom
/// keeps that sharp on a display scaled past 100%. Costs 147KB of resident
/// memory per distinct cover, where a 1300px embedded cover costs 6.5MB decoded.
const COVER_EDGE: u32 = 192;
/// Longest edge of the bytes the 276px now-playing hero is built from. Kept
/// encoded rather than decoded because only one hero exists at a time.
const HERO_EDGE: u32 = 320;
/// Quality for the re-encoded hero. The source is already lossy and this is
/// only ever drawn at 276px.
const HERO_QUALITY: u8 = 86;

/// Decodes `bytes` once and returns the cover every list draws plus the bytes
/// the hero is built from. Artwork that will not decode yields `None`, which
/// leaves the track looking like it carries no cover at all.
fn build_cover(bytes: &Arc<[u8]>) -> Option<(Arc<gpui_kit::RenderImage>, Arc<[u8]>)> {
    let decoded = decode_artwork(bytes)?;
    let cover = render_image(fit_within(&decoded, COVER_EDGE));

    let scaled = fit_within(&decoded, HERO_EDGE);
    let shrunk = matches!(scaled, Cow::Owned(_));
    let hero = match encode_hero(&scaled) {
        // A cover that already fit can re-encode larger than it arrived, so keep
        // whichever of the two is less to hold. One that had to be scaled down
        // has no choice: the bytes it came in are the ones worth dropping.
        Some(encoded) if shrunk || encoded.len() < bytes.len() => encoded,
        _ => bytes.clone(),
    };
    Some((Arc::new(cover), hero))
}

/// Builds the now-playing hero from the bytes `build_cover` kept.
pub fn hero_image(bytes: &[u8]) -> Option<gpui_kit::RenderImage> {
    decode_artwork(bytes).map(|decoded| render_image(Cow::Owned(decoded)))
}

fn decode_artwork(bytes: &[u8]) -> Option<image::RgbaImage> {
    let mut decoder = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_decoder()
        .ok()?;
    // Cover art lifted from a photo carries an orientation tag; without this it
    // is drawn on its side.
    let orientation = decoder.orientation().ok()?;
    let mut decoded = image::DynamicImage::from_decoder(decoder).ok()?;
    decoded.apply_orientation(orientation);
    Some(decoded.into_rgba8())
}

/// Scales `source` down so its longest edge is at most `max_edge`, borrowing
/// when it is already small enough.
fn fit_within(source: &image::RgbaImage, max_edge: u32) -> Cow<'_, image::RgbaImage> {
    let (width, height) = source.dimensions();
    let longest = width.max(height);
    if longest <= max_edge || longest == 0 {
        return Cow::Borrowed(source);
    }
    let scale = f64::from(max_edge) / f64::from(longest);
    let scaled = |edge: u32| ((f64::from(edge) * scale).round() as u32).max(1);
    Cow::Owned(image::imageops::resize(
        source,
        scaled(width),
        scaled(height),
        image::imageops::FilterType::Triangle,
    ))
}

fn render_image(rgba: Cow<'_, image::RgbaImage>) -> gpui_kit::RenderImage {
    let mut buffer = rgba.into_owned();
    // The renderer reads BGRA.
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    gpui_kit::RenderImage::new(vec![image::Frame::new(buffer)])
}

fn encode_hero(rgba: &image::RgbaImage) -> Option<Arc<[u8]>> {
    let mut out = Vec::new();
    // JPEG has no alpha, so cover art that uses it stays PNG rather than being
    // flattened onto black.
    if rgba.pixels().any(|pixel| pixel.0[3] != u8::MAX) {
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )
            .ok()?;
        return Some(Arc::from(out));
    }
    let rgb = image::DynamicImage::ImageRgba8(rgba.clone()).into_rgb8();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, HERO_QUALITY)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .ok()?;
    Some(Arc::from(out))
}

/// One worker per core, never more than there is work for.
fn worker_count(items: usize) -> usize {
    if items < 2 {
        return items;
    }
    std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(items)
}

/// Runs `work` over every item across all cores and returns the results in
/// input order. Scanning is the one place the app does sustained CPU work —
/// tag parsing and cover decoding are per-file and independent — and it used to
/// run on a single thread.
fn map_parallel<T, R, F>(items: &[T], work: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let workers = worker_count(items.len());
    if workers <= 1 {
        return items.iter().map(work).collect();
    }

    let cursor = AtomicUsize::new(0);
    let parts: Vec<Vec<(usize, R)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let (cursor, work) = (&cursor, &work);
                scope.spawn(move || {
                    let mut done = Vec::new();
                    loop {
                        // A shared cursor rather than fixed chunks: files vary
                        // enormously in size, so chunking would leave one worker
                        // grinding while the rest sat idle.
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(index) else { break };
                        done.push((index, work(item)));
                    }
                    done
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("scan worker panicked"))
            .collect()
    });

    let mut slots: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    for (index, value) in parts.into_iter().flatten() {
        slots[index] = Some(value);
    }
    slots
        .into_iter()
        .map(|slot| slot.expect("every index is claimed by exactly one worker"))
        .collect()
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
    track.rebuild_search_key();
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

fn scan_folder_inner(folder: &Path, sort: bool) -> ScanResult {
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

    // Walking is inherently serial; collect the whole list first so the reads
    // can be spread across every core.
    let mut found: Vec<(PathBuf, Option<std::time::SystemTime>)> = Vec::new();
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
        // WalkDir already statted this entry; reuse it instead of a second stat.
        let added = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        found.push((entry.into_path(), added));
    }

    result.tracks.reserve(found.len());
    for read in map_parallel(&found, |(path, added)| read_track_with_added(path, *added)) {
        match read {
            Ok(track) => result.tracks.push(track),
            Err(error) => result.errors.push(format!("{error:#}")),
        }
    }

    if sort {
        // The standalone entry point has no later pass to attach covers.
        attach_covers(&mut result.tracks);
        sort_tracks(&mut result.tracks);
    }
    result.errors.sort();
    result
}

/// Sorted single-folder scan. Kept as the stable single-folder entry point
/// (and used by tests); multi-folder scans go through `scan_folders` so the
/// final sort happens once.
#[allow(dead_code)]
pub fn scan_folder(folder: &Path) -> ScanResult {
    scan_folder_inner(folder, true)
}
fn sort_tracks(tracks: &mut [Track]) {
    tracks.sort_by_cached_key(|track| {
        (
            track.title.to_lowercase(),
            track.artist.to_lowercase(),
            track.album.to_lowercase(),
            track.path.clone(),
        )
    });
}

pub fn scan_folders(folders: &[PathBuf]) -> ScanResult {
    let mut result = ScanResult::default();
    let mut total = 0usize;
    let mut subs: Vec<ScanResult> = Vec::with_capacity(folders.len());
    for folder in folders {
        // Skip the per-folder sort; one final sort covers the merge.
        let sub = scan_folder_inner(folder, false);
        total += sub.tracks.len();
        subs.push(sub);
    }
    result.tracks.reserve(total);
    for sub in subs {
        result.errors.extend(sub.errors);
        result.tracks.extend(sub.tracks);
    }
    // Dedupe without cloning a PathBuf per track: adjacent after a path sort.
    result.tracks.sort_by(|a, b| a.path.cmp(&b.path));
    result.tracks.dedup_by(|a, b| a.path == b.path);
    attach_covers(&mut result.tracks);
    sort_tracks(&mut result.tracks);
    result.errors.sort();
    result.errors.dedup();
    result
}

/// Groups tracks by the exact bytes of their cover. Tracks from one album carry
/// byte-identical covers, so this is what keeps an album from decoding, scaling
/// and holding the same JPEG once per track.
fn artwork_groups(tracks: &[Track]) -> Vec<(Arc<[u8]>, Vec<usize>)> {
    use std::collections::hash_map::DefaultHasher;
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};

    let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, track) in tracks.iter().enumerate() {
        let Some(bytes) = &track.artwork else {
            continue;
        };
        let mut hasher = DefaultHasher::new();
        bytes.len().hash(&mut hasher);
        hasher.write(bytes);
        buckets.entry(hasher.finish()).or_default().push(index);
    }

    let mut groups: Vec<(Arc<[u8]>, Vec<usize>)> = Vec::new();
    for indices in buckets.into_values() {
        // Confirm byte equality inside a bucket before sharing one cover.
        let first = groups.len();
        for index in indices {
            let Some(bytes) = &tracks[index].artwork else {
                continue;
            };
            match groups[first..]
                .iter_mut()
                .find(|(known, _)| Arc::ptr_eq(known, bytes) || known.as_ref() == bytes.as_ref())
            {
                Some((_, members)) => members.push(index),
                None => groups.push((bytes.clone(), vec![index])),
            }
        }
    }
    groups
}

/// Builds one cover per distinct artwork, across all cores, and shares each
/// result with every track that carries it.
fn attach_covers(tracks: &mut [Track]) {
    let groups = artwork_groups(tracks);
    if groups.is_empty() {
        return;
    }
    let built = map_parallel(&groups, |(bytes, _)| build_cover(bytes));
    for ((_, members), cover) in groups.iter().zip(built) {
        for &index in members {
            match &cover {
                Some((image, hero)) => {
                    tracks[index].artwork = Some(hero.clone());
                    tracks[index].artwork_image = Some(image.clone());
                }
                // Undecodable artwork is dropped rather than carried around as
                // bytes nothing can draw.
                None => tracks[index].artwork = None,
            }
        }
    }
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

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "oga", "m4a", "mp4", "aac", "aif", "aiff",
];

fn supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

/// Single-track read, used for freshly downloaded audio. The bulk scan defers
/// cover building to `attach_covers` so albums decode once.
fn read_track(path: &Path) -> Result<Track> {
    let mut track = read_track_with_added(path, None)?;
    attach_covers(std::slice::from_mut(&mut track));
    Ok(track)
}

fn read_track_with_added(path: &Path, added_hint: Option<std::time::SystemTime>) -> Result<Track> {
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
            let title = filename_title(path);
            let artist = "Unknown Artist".to_owned();
            let album = "Unknown Album".to_owned();
            let search_key = Track::build_search_key(&title, &artist, &album);
            return Ok(Track {
                path: path.to_path_buf(),
                title,
                artist,
                album,
                duration,
                artwork: None,
                artwork_image: None,
                year: None,
                added: added_hint.or_else(|| file_added(path)),
                search_key,
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
    let artwork: Option<Arc<[u8]>> = tag
        .and_then(|tag| {
            tag.pictures()
                .iter()
                .find(|picture| picture.pic_type() == PictureType::CoverFront)
                .or_else(|| tag.pictures().first())
        })
        .map(|picture| Arc::from(picture.data()));
    let year = tag.and_then(|tag| tag.year()).filter(|year| *year > 0);

    let search_key = Track::build_search_key(&title, &artist, &album);
    Ok(Track {
        path: path.to_path_buf(),
        title,
        artist,
        album,
        duration,
        artwork,
        artwork_image: None,
        year,
        added: added_hint.or_else(|| file_added(path)),
        search_key,
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

/// Lock-free EQ parameters shared between the UI thread and the realtime
/// audio thread. The old `RwLock<EqualizerState>` read on every sample (up to
/// 44,100 lock acquisitions per second) risked priority inversion and audible
/// dropouts; these atomics publish gains with a generation counter instead.
pub struct EqualizerShared {
    enabled: std::sync::atomic::AtomicBool,
    gains_bits: [std::sync::atomic::AtomicU32; 5],
    generation: std::sync::atomic::AtomicU64,
}

impl std::fmt::Debug for EqualizerShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (enabled, gains, generation) = self.load();
        f.debug_struct("EqualizerShared")
            .field("enabled", &enabled)
            .field("gains", &gains)
            .field("generation", &generation)
            .finish()
    }
}

impl EqualizerShared {
    pub fn new(enabled: bool, gains: [f32; 5]) -> Self {
        Self {
            enabled: std::sync::atomic::AtomicBool::new(enabled),
            gains_bits: gains.map(|gain| std::sync::atomic::AtomicU32::new(gain.to_bits())),
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Publish new parameters. Called from the UI thread only.
    pub fn store(&self, enabled: bool, gains: [f32; 5]) {
        use std::sync::atomic::Ordering;
        for (slot, &gain) in self.gains_bits.iter().zip(gains.iter()) {
            slot.store(gain.to_bits(), Ordering::Relaxed);
        }
        self.enabled.store(enabled, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn load(&self) -> (bool, [f32; 5], u64) {
        use std::sync::atomic::Ordering;
        let gains = self
            .gains_bits
            .each_ref()
            .map(|slot| f32::from_bits(slot.load(Ordering::Relaxed)));
        let enabled = self.enabled.load(Ordering::Relaxed);
        let generation = self.generation.load(Ordering::Acquire);
        (enabled, gains, generation)
    }
}

/// Maximum channels per EQ frame. Five bands times eight channels fits in a
/// few cache lines and avoids the old double `Vec` pointer chase.
pub const MAX_EQ_CHANNELS: usize = 8;

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

/// Peak level of the audio leaving the player, written by the audio thread
/// and read by the interface. Cloning shares the same reading.
#[derive(Clone, Debug, Default)]
pub struct LevelMeter(Arc<std::sync::atomic::AtomicU32>);

impl LevelMeter {
    /// The current level, roughly 0 for silence and 1 for a full scale peak.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.0.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn store(&self, value: f32) {
        self.0
            .store(value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

/// Samples per meter update: about 11 ms at 48 kHz stereo, fast enough to
/// follow a beat without writing on every sample.
const METER_WINDOW: u32 = 1024;

pub struct EqualizerSource<I> {
    input: I,
    channels: u16,
    sample_rate: u32,
    current_channel: usize,
    /// Flat `[band][channel]` storage: contiguous, no per-band heap pointer.
    filters: [[BiquadChannel; MAX_EQ_CHANNELS]; 5],
    coeffs: [BiquadCoeffs; 5],
    cached_enabled: bool,
    cached_gains: [f32; 5],
    cached_generation: u64,
    /// Bit `b` set while band `b` is enabled and audible (|gain| >= 0.05).
    active_mask: u8,
    shared: Arc<EqualizerShared>,
    meter: LevelMeter,
    peak: f32,
    metered: u32,
}

fn active_band_mask(enabled: bool, gains: &[f32; 5]) -> u8 {
    if !enabled {
        return 0;
    }
    let mut mask = 0u8;
    for (b, &gain) in gains.iter().enumerate() {
        if gain.abs() >= 0.05 {
            mask |= 1 << b;
        }
    }
    mask
}

impl<I> EqualizerSource<I>
where
    I: Source<Item = f32>,
{
    pub fn new(input: I, shared: Arc<EqualizerShared>, meter: LevelMeter) -> Self {
        let channels = input.channels().max(1).min(MAX_EQ_CHANNELS as u16);
        let sample_rate = input.sample_rate().max(1);
        let (enabled, gains, generation) = shared.load();
        let mut coeffs = [BiquadCoeffs::identity(); 5];
        for b in 0..5 {
            coeffs[b] = BiquadCoeffs::peaking(EQ_FREQUENCIES[b], gains[b], sample_rate as f32, 1.0);
        }
        Self {
            input,
            channels,
            sample_rate,
            current_channel: 0,
            filters: [[BiquadChannel::default(); MAX_EQ_CHANNELS]; 5],
            coeffs,
            cached_enabled: enabled,
            cached_gains: gains,
            cached_generation: generation,
            active_mask: active_band_mask(enabled, &gains),
            meter,
            peak: 0.0,
            metered: 0,
            shared,
        }
    }

    /// Backwards-compatible constructor for a fixed snapshot (used by tests
    /// that don't need live updates).
    #[allow(dead_code)]
    pub fn new_static(input: I, state: &EqualizerState, meter: LevelMeter) -> Self {
        Self::new(
            input,
            Arc::new(EqualizerShared::new(state.enabled, state.gains)),
            meter,
        )
    }

    #[inline]
    fn poll_params(&mut self) {
        let generation = self.shared.generation();
        if generation == self.cached_generation {
            return;
        }
        let (enabled, gains, generation) = self.shared.load();
        self.cached_generation = generation;
        self.cached_enabled = enabled;
        self.cached_gains = gains;
        for (b, &freq) in EQ_FREQUENCIES.iter().enumerate() {
            self.coeffs[b] = BiquadCoeffs::peaking(freq, gains[b], self.sample_rate as f32, 1.0);
        }
        self.active_mask = active_band_mask(enabled, &gains);
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
            for ch in band.iter_mut() {
                *ch = BiquadChannel::default();
            }
        }
        self.input.try_seek(pos)
    }
}

impl<I> EqualizerSource<I> {
    /// Tracks the peak over a short window. The level jumps to a new peak at
    /// once and falls back gradually, so the meter follows transients without
    /// flickering between windows.
    #[inline]
    fn meter_sample(&mut self, sample: f32) {
        self.peak = self.peak.max(sample.abs());
        self.metered += 1;
        if self.metered >= METER_WINDOW {
            let previous = self.meter.level();
            let level = if self.peak > previous {
                self.peak
            } else {
                previous * 0.80 + self.peak * 0.20
            };
            self.meter.store(level.clamp(0.0, 1.0));
            self.peak = 0.0;
            self.metered = 0;
        }
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
        self.meter_sample(sample);

        // One generation load per audio frame; coefficients rebuild only when
        // the UI actually published new gains.
        if self.current_channel == 0 {
            self.poll_params();
            if self.active_mask == 0 {
                self.current_channel = (self.current_channel + 1) % (self.channels as usize);
                return Some(sample);
            }
        }

        let ch = self.current_channel;
        self.current_channel = (ch + 1) % (self.channels as usize);

        let mut out = sample;
        let mut mask = self.active_mask;
        while mask != 0 {
            let b = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            out = self.filters[b][ch].process(out, &self.coeffs[b]);
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
    eq_shared: Arc<EqualizerShared>,
    meter: LevelMeter,
}

impl MediaPlayer {
    pub fn new() -> Result<Self> {
        let stream = OutputStreamBuilder::open_default_stream()
            .context("Failed to open an audio output device")?;
        let sink = Sink::connect_new(stream.mixer());
        sink.pause();
        let eq_state = Arc::new(std::sync::RwLock::new(EqualizerState::default()));
        let eq_shared = Arc::new(EqualizerShared::new(false, [0.0; 5]));
        Ok(Self {
            sink,
            stream,
            duration: None,
            loaded: false,
            eq_state,
            eq_shared,
            meter: LevelMeter::default(),
        })
    }

    fn publish_equalizer(&self) {
        let (enabled, gains) = self
            .eq_state
            .read()
            .map(|state| (state.enabled, state.gains))
            .unwrap_or((false, [0.0; 5]));
        self.eq_shared.store(enabled, gains);
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
        let eq_source = EqualizerSource::new(decoder, self.eq_shared.clone(), self.meter.clone());
        sink.append(eq_source);
        self.sink.stop();
        self.sink = sink;
        self.duration = duration;
        self.loaded = true;
        Ok(())
    }

    /// Shared peak meter for the audio being played.
    pub fn meter(&self) -> LevelMeter {
        self.meter.clone()
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
        let gains = self
            .eq_state
            .read()
            .map(|state| state.gains)
            .unwrap_or([0.0; 5]);
        self.eq_shared.store(enabled, gains);
    }

    pub fn set_equalizer_gains(&self, gains: [f32; 5]) {
        if let Ok(mut state) = self.eq_state.write() {
            state.gains = gains;
        }
        let enabled = self
            .eq_state
            .read()
            .map(|state| state.enabled)
            .unwrap_or(false);
        self.eq_shared.store(enabled, gains);
    }

    #[allow(dead_code)]
    pub fn set_equalizer_band(&self, band: usize, gain_db: f32) {
        if band < 5 {
            if let Ok(mut state) = self.eq_state.write() {
                state.gains[band] = gain_db.clamp(-12.0, 12.0);
            }
            self.publish_equalizer();
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

        let state = EqualizerState {
            enabled: true,
            gains: [6.0, 3.0, 0.0, -3.0, -6.0],
        };

        let mut eq_source = EqualizerSource::new_static(decoder, &state, LevelMeter::default());
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

    #[test]
    fn parallel_work_keeps_input_order_and_runs_every_item_once() {
        let items: Vec<usize> = (0..1_000).collect();
        let visits: Vec<AtomicU64> = items.iter().map(|_| AtomicU64::new(0)).collect();

        let squares = map_parallel(&items, |item| {
            visits[*item].fetch_add(1, Ordering::Relaxed);
            item * item
        });

        assert_eq!(squares.len(), items.len());
        for (item, square) in items.iter().zip(&squares) {
            assert_eq!(*square, item * item, "results came back out of order");
        }
        for (item, visits) in visits.iter().enumerate() {
            assert_eq!(
                visits.load(Ordering::Relaxed),
                1,
                "item {item} was not run exactly once"
            );
        }
    }

    /// A cover is drawn at 158px at the largest, so what is kept has to be
    /// bounded regardless of what was embedded — the renderer holds every image
    /// it draws for the life of the process.
    #[test]
    fn covers_are_scaled_down_and_shared_across_an_album() {
        let embedded = encoded_artwork(900, 700);
        let original = embedded.len();
        let mut tracks: Vec<Track> = (0..4)
            .map(|index| track_with_artwork(index, Some(embedded.clone())))
            .collect();

        attach_covers(&mut tracks);

        for track in &tracks {
            let cover = track.artwork_image.as_ref().expect("cover was built");
            let size = cover.size(0);
            assert_eq!(size.width.0, COVER_EDGE as i32);
            let expected = (700.0 * f64::from(COVER_EDGE) / 900.0).round() as i32;
            assert_eq!(size.height.0, expected, "aspect ratio was not preserved");
            let kept = track.artwork.as_ref().expect("hero bytes were kept");
            assert!(
                kept.len() < original / 2,
                "hero bytes were not re-encoded smaller: {} of {original}",
                kept.len()
            );
            let hero = hero_image(kept).expect("hero decodes");
            assert!(hero.size(0).width.0 <= HERO_EDGE as i32);
        }

        let first = tracks[0].artwork_image.as_ref().unwrap();
        for track in &tracks[1..] {
            assert!(
                Arc::ptr_eq(first, track.artwork_image.as_ref().unwrap()),
                "an album decoded its cover more than once"
            );
        }
    }

    #[test]
    fn artwork_that_cannot_be_decoded_is_dropped() {
        let mut tracks = vec![track_with_artwork(0, Some(Arc::from(vec![1u8, 2, 3, 4])))];

        attach_covers(&mut tracks);

        assert!(tracks[0].artwork.is_none());
        assert!(tracks[0].artwork_image.is_none());
    }

    /// Noise rather than a gradient: real cover art is photographic, and a
    /// smooth synthetic image compresses so well that re-encoding it would
    /// legitimately grow it.
    fn encoded_artwork(width: u32, height: u32) -> Arc<[u8]> {
        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let source = image::RgbaImage::from_fn(width, height, |_, _| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let bytes = seed.to_le_bytes();
            image::Rgba([bytes[0], bytes[1], bytes[2], u8::MAX])
        });
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                source.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        Arc::from(bytes)
    }

    fn track_with_artwork(index: usize, artwork: Option<Arc<[u8]>>) -> Track {
        Track {
            path: PathBuf::from(format!("track{index}.mp3")),
            title: format!("Track {index}"),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            duration: Duration::from_secs(1),
            artwork,
            artwork_image: None,
            year: None,
            added: None,
            search_key: Track::build_search_key("", "", ""),
        }
    }
}
