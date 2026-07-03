mod player;

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use midi2otomad_core::audio::pitch_detect::{calibration_for_hz, detect_fundamental_hz};
use midi2otomad_core::audio::{
    build_waveform_peaks, mix_project, MixOptions, MixResult, PcmAudio, RenderQuality, MAX_LAYERS,
};
use midi2otomad_core::id::make_id;
use midi2otomad_core::media::{decode_audio, encode_wav};
use midi2otomad_core::midi::{midi_to_project_with_mode, ImportMode};
use midi2otomad_core::schema::{
    create_empty_project, parse_project, Project, Sample, DEFAULT_PROJECT_NAME,
};

use player::{Player, PlayerStatus};

const PEAK_BUCKETS: usize = 600;
const AUDIO_EXTENSIONS: [&str; 8] = ["wav", "mp3", "ogg", "flac", "m4a", "aac", "aif", "aiff"];
const PROJECT_EXTENSION: &str = "m2oproj";

pub struct AppState {
    bank: Mutex<HashMap<String, PcmAudio>>,
    player: Option<Player>,
}

impl AppState {
    fn new() -> Self {
        let player = match Player::new() {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("オーディオ出力を初期化できませんでした（無音で続行）: {e}");
                None
            }
        };
        Self {
            bank: Mutex::new(HashMap::new()),
            player,
        }
    }

    fn render(&self, project: &Project, options: MixOptions) -> Result<MixResult, String> {
        let bank = self
            .bank
            .lock()
            .map_err(|_| "バンクのロックに失敗".to_string())?;
        Ok(mix_project(project, &*bank, &options))
    }

    fn engine_rate(&self) -> Option<f64> {
        self.player.as_ref().map(|p| p.engine_rate() as f64)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleDto {
    id: String,
    name: String,
    file_name: String,
    source_path: String,
    duration_sec: f64,
    peaks: Vec<f32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    project: Project,
    track_count: usize,
    note_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestResult {
    import: Option<ImportResult>,
    samples: Vec<SampleDto>,
    loaded: Option<ProjectLoad>,
    /// 読み込めなかったファイルのエラー（残りのファイルは処理を続ける）。
    failed: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoad {
    project: Project,
    samples: Vec<SampleDto>,
    missing: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixSummary {
    duration_sec: f64,
    peak: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    path: String,
    bytes: u64,
    duration_sec: f64,
}

#[derive(Serialize)]
pub struct MediaProbe {
    backend: String,
    version: String,
}

fn file_stem_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn is_midi(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mid") | Some("midi")
    )
}

fn is_project_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
        == Some(PROJECT_EXTENSION)
}

/// Windows で拡張子に関係なく無効になる予約デバイス名。
const RESERVED_FILE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// OS のファイル名として使えない文字・名前を除去する
/// （保存/書き出しダイアログの初期名用）。
fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        return "音MAD".to_string();
    }
    let stem = trimmed.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED_FILE_NAMES.contains(&stem.as_str()) {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn decode_and_store(state: &AppState, path: &std::path::Path) -> Result<SampleDto, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let pcm = decode_audio(&bytes)
        .map_err(|e| format!("{} をデコードできませんでした: {e}", file_stem_name(path)))?;
    let id = make_id("sample");
    let peaks = build_waveform_peaks(&pcm, PEAK_BUCKETS);
    let duration_sec = pcm.duration_sec();
    let file_name = file_stem_name(path);
    let name = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| file_name.clone());
    state
        .bank
        .lock()
        .map_err(|_| "バンクのロックに失敗".to_string())?
        .insert(id.clone(), pcm);
    Ok(SampleDto {
        id,
        name,
        file_name,
        source_path: path.display().to_string(),
        duration_sec,
        peaks,
    })
}

fn sample_dto(sample: &Sample, pcm: &PcmAudio) -> SampleDto {
    SampleDto {
        id: sample.id.clone(),
        name: sample.name.clone(),
        file_name: sample.file_name.clone(),
        source_path: sample.source_path.clone(),
        duration_sec: pcm.duration_sec(),
        peaks: build_waveform_peaks(pcm, PEAK_BUCKETS),
    }
}

/// プロジェクトファイル (.m2oproj) を読み込み、参照されている音声素材を
/// バンク（既にデコード済みならそれを再利用）または `sourcePath` から復元する。
fn load_project_from_path(state: &AppState, path: &std::path::Path) -> Result<ProjectLoad, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("プロジェクトを読み込めませんでした: {e}"))?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("プロジェクトの JSON 解析に失敗しました: {e}"))?;
    let mut project = parse_project(raw)?;

    let mut samples = Vec::new();
    let mut missing = Vec::new();
    {
        let mut bank = state
            .bank
            .lock()
            .map_err(|_| "バンクのロックに失敗".to_string())?;
        for sample in &mut project.samples {
            // まず元ファイルから読み直す（外部で編集された音声を反映するため）。
            let fresh = if sample.source_path.is_empty() {
                None
            } else {
                std::fs::read(std::path::Path::new(&sample.source_path))
                    .ok()
                    .and_then(|b| decode_audio(&b).ok())
            };
            match fresh {
                Some(pcm) => {
                    sample.duration_sec = pcm.duration_sec();
                    samples.push(sample_dto(sample, &pcm));
                    bank.insert(sample.id.clone(), pcm);
                }
                // 元ファイルが読めなくても、セッション内のデコード結果が
                // 残っていればそれで復元する（保存後にファイルを移動した
                // 直後などのフォールバック）。
                None => {
                    if let Some(pcm) = bank.get(&sample.id) {
                        sample.duration_sec = pcm.duration_sec();
                        samples.push(sample_dto(sample, pcm));
                        missing.push(format!("{}（メモリ上のデータで継続）", sample.name));
                    } else {
                        missing.push(sample.name.clone());
                    }
                }
            }
        }
    }
    Ok(ProjectLoad {
        project,
        samples,
        missing,
    })
}

fn load_into_player(state: &AppState, mix: &MixResult) {
    if let Some(player) = &state.player {
        player.set_mix(&mix.left, &mix.right, mix.sample_rate);
    }
}

#[tauri::command]
fn default_project() -> Project {
    create_empty_project(DEFAULT_PROJECT_NAME)
}

#[tauri::command]
fn probe_media() -> MediaProbe {
    MediaProbe {
        backend: "rust (symphonia + libmp3lame)".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
fn open_midi(
    app: AppHandle,
    previous: Option<Project>,
    mode: Option<String>,
) -> Result<Option<ImportResult>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("MIDI", &["mid", "midi"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let import_mode = ImportMode::from_str_or_auto(mode.as_deref().unwrap_or("auto"));
    let result = midi_to_project_with_mode(
        &bytes,
        &file_stem_name(&path),
        previous.as_ref(),
        import_mode,
    )?;
    Ok(Some(ImportResult {
        project: result.project,
        track_count: result.track_count,
        note_count: result.note_count,
    }))
}

#[tauri::command]
fn open_audio(app: AppHandle, state: State<AppState>) -> Result<Vec<SampleDto>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Audio", &AUDIO_EXTENSIONS)
        .blocking_pick_files();
    let Some(files) = picked else {
        return Ok(Vec::new());
    };
    let mut samples = Vec::new();
    for file in files {
        let path = file.into_path().map_err(|e| e.to_string())?;
        samples.push(decode_and_store(&state, &path)?);
    }
    Ok(samples)
}

#[tauri::command]
fn ingest_paths(
    state: State<AppState>,
    paths: Vec<String>,
    previous: Option<Project>,
    mode: Option<String>,
) -> Result<IngestResult, String> {
    // プロジェクトファイルが含まれていれば、それがすべてを置き換える。
    if let Some(project_path) = paths
        .iter()
        .map(std::path::PathBuf::from)
        .find(|p| is_project_file(p))
    {
        let loaded = load_project_from_path(&state, &project_path)?;
        return Ok(IngestResult {
            import: None,
            samples: Vec::new(),
            loaded: Some(loaded),
            failed: Vec::new(),
        });
    }

    let import_mode = ImportMode::from_str_or_auto(mode.as_deref().unwrap_or("auto"));
    let mut import = None;
    let mut samples = Vec::new();
    let mut failed = Vec::new();
    // 1 つのファイルが壊れていても残りは読み込む。
    for path_str in paths {
        let path = std::path::PathBuf::from(&path_str);
        if is_midi(&path) {
            if import.is_some() {
                continue;
            }
            let imported = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    midi_to_project_with_mode(
                        &bytes,
                        &file_stem_name(&path),
                        previous.as_ref(),
                        import_mode,
                    )
                });
            match imported {
                Ok(result) => {
                    import = Some(ImportResult {
                        project: result.project,
                        track_count: result.track_count,
                        note_count: result.note_count,
                    });
                }
                Err(e) => failed.push(e),
            }
        } else {
            match decode_and_store(&state, &path) {
                Ok(dto) => samples.push(dto),
                Err(e) => failed.push(e),
            }
        }
    }
    Ok(IngestResult {
        import,
        samples,
        loaded: None,
        failed,
    })
}

#[tauri::command]
fn save_project(app: AppHandle, project: Project) -> Result<Option<String>, String> {
    let suggested = format!("{}.{PROJECT_EXTENSION}", sanitize_file_name(&project.name));
    let picked = app
        .dialog()
        .file()
        .set_file_name(&suggested)
        .add_filter("midi2otomad プロジェクト", &[PROJECT_EXTENSION])
        .blocking_save_file();
    let Some(target) = picked else {
        return Ok(None);
    };
    let path = target.into_path().map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("保存に失敗しました: {e}"))?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
fn load_project(app: AppHandle, state: State<AppState>) -> Result<Option<ProjectLoad>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("midi2otomad プロジェクト", &[PROJECT_EXTENSION])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    load_project_from_path(&state, &path).map(Some)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PitchEstimate {
    base_pitch: i32,
    tune_cents: f64,
    hz: f64,
}

fn downmix_mono(pcm: &PcmAudio) -> Vec<f32> {
    match pcm.channels.as_slice() {
        [] => Vec::new(),
        [only] => only.clone(),
        channels => {
            let count = channels.len() as f32;
            (0..pcm.frames)
                .map(|i| {
                    channels
                        .iter()
                        .map(|c| c.get(i).copied().unwrap_or(0.0))
                        .sum::<f32>()
                        / count
                })
                .collect()
        }
    }
}

fn trim_region(sample: &Sample, frames: usize, rate: f64) -> (usize, usize) {
    if !sample.trim.enabled || frames < 2 {
        return (0, frames);
    }
    let start = ((sample.trim.start_sec * rate).floor().max(0.0) as usize).min(frames - 1);
    let end = if sample.trim.end_sec > sample.trim.start_sec {
        (sample.trim.end_sec * rate).floor() as usize
    } else {
        frames
    };
    (start, end.clamp(start + 1, frames))
}

#[tauri::command]
fn detect_pitch(state: State<AppState>, sample: Sample) -> Result<Option<PitchEstimate>, String> {
    let bank = state
        .bank
        .lock()
        .map_err(|_| "バンクのロックに失敗".to_string())?;
    let pcm = bank.get(&sample.id).ok_or("素材がデコードされていません")?;
    let mono = downmix_mono(pcm);
    let (start, end) = trim_region(&sample, mono.len(), pcm.sample_rate);
    Ok(
        detect_fundamental_hz(&mono[start..end], pcm.sample_rate).map(|hz| {
            let (base_pitch, tune_cents) = calibration_for_hz(hz);
            PitchEstimate {
                base_pitch,
                tune_cents,
                hz,
            }
        }),
    )
}

fn quality_for(performance: bool) -> RenderQuality {
    if performance {
        RenderQuality::Performance
    } else {
        RenderQuality::Full
    }
}

#[tauri::command]
fn preview_sample(
    state: State<AppState>,
    mut sample: Sample,
    layers: Option<Vec<Sample>>,
    pitch: Option<i32>,
    drum_mode: Option<bool>,
    performance: Option<bool>,
) -> Result<(), String> {
    let note_pitch = pitch.unwrap_or(sample.base_pitch);
    let drum_mode = drum_mode.unwrap_or(false);
    let duration = {
        let bank = state
            .bank
            .lock()
            .map_err(|_| "バンクのロックに失敗".to_string())?;
        let pcm = bank.get(&sample.id).ok_or("素材がデコードされていません")?;
        let clip_sec = pcm.frames as f64 / pcm.sample_rate;
        let trimmed_sec = if sample.trim.enabled {
            let start = sample.trim.start_sec.max(0.0);
            let end = if sample.trim.end_sec > start {
                sample.trim.end_sec
            } else {
                clip_sec
            };
            (end.min(clip_sec) - start).max(0.0)
        } else {
            clip_sec
        };
        let natural = trimmed_sec.min(2.2);
        if sample.loop_region.enabled && !sample.one_shot {
            1.4
        } else {
            natural
        }
        .max(0.05)
    };
    if sample.one_shot {
        sample.one_shot = false;
        sample.loop_region.enabled = false;
    }
    // レイヤー先の素材も同梱してプレビューでも重ねて鳴らす。
    // 上限はミキサーの collect_sources と同じ（ルート込み MAX_LAYERS）。
    let mut sample_values = vec![serde_json::to_value(&sample).map_err(|e| e.to_string())?];
    let mut seen = std::collections::HashSet::new();
    seen.insert(sample.id.clone());
    for layer in layers.unwrap_or_default() {
        if sample_values.len() >= MAX_LAYERS {
            break;
        }
        if seen.insert(layer.id.clone()) {
            sample_values.push(serde_json::to_value(&layer).map_err(|e| e.to_string())?);
        }
    }
    let project = parse_project(json!({
        "version": 1,
        "name": "preview",
        "sampleRate": 48000,
        "samples": sample_values,
        "tracks": [{
            "id": "preview",
            "name": "preview",
            "defaultSampleId": sample.id,
            "drumMode": drum_mode,
            "notes": [{ "pitch": note_pitch, "startSec": 0, "durationSec": duration, "velocity": 127 }]
        }]
    }))?;
    let mix = state.render(
        &project,
        MixOptions {
            limiter: Some(false),
            tail_sec: Some(0.1),
            quality: quality_for(performance.unwrap_or(false)),
            target_rate: state.engine_rate(),
        },
    )?;
    load_into_player(&state, &mix);
    if let Some(player) = &state.player {
        player.play(Some(0.0));
    }
    Ok(())
}

#[tauri::command]
fn set_mix(
    state: State<AppState>,
    project: Project,
    performance: Option<bool>,
) -> Result<MixSummary, String> {
    let mix = state.render(
        &project,
        MixOptions {
            quality: quality_for(performance.unwrap_or(false)),
            target_rate: state.engine_rate(),
            ..Default::default()
        },
    )?;
    load_into_player(&state, &mix);
    Ok(MixSummary {
        duration_sec: mix.duration_sec,
        peak: mix.peak,
    })
}

#[tauri::command]
fn play(state: State<AppState>, from_sec: Option<f64>) {
    if let Some(player) = &state.player {
        player.play(from_sec);
    }
}

#[tauri::command]
fn pause(state: State<AppState>) {
    if let Some(player) = &state.player {
        player.pause();
    }
}

#[tauri::command]
fn stop(state: State<AppState>) {
    if let Some(player) = &state.player {
        player.stop();
    }
}

#[tauri::command]
fn seek(state: State<AppState>, sec: f64) {
    if let Some(player) = &state.player {
        player.seek(sec);
    }
}

#[tauri::command]
fn set_loop(state: State<AppState>, enabled: bool) {
    if let Some(player) = &state.player {
        player.set_looping(enabled);
    }
}

/// フロントエンドの Undo 履歴からも参照されなくなった素材をバンクから
/// 解放する（プロジェクト読込時にフロントが到達可能 ID を渡してくる）。
#[tauri::command]
fn prune_samples(state: State<AppState>, keep: Vec<String>) {
    let keep: std::collections::HashSet<String> = keep.into_iter().collect();
    if let Ok(mut bank) = state.bank.lock() {
        bank.retain(|id, _| keep.contains(id));
    }
}

#[tauri::command]
fn status(state: State<AppState>) -> PlayerStatus {
    state
        .player
        .as_ref()
        .map(|p| p.status())
        .unwrap_or(PlayerStatus {
            playing: false,
            position: 0.0,
            duration: 0.0,
            level: 0.0,
        })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    project: Project,
    format: String,
    wav_bit_depth: Option<u16>,
    mp3_bitrate: Option<u32>,
}

#[tauri::command]
fn export(
    app: AppHandle,
    state: State<AppState>,
    request: ExportRequest,
) -> Result<Option<ExportResult>, String> {
    let mix = state.render(&request.project, MixOptions::default())?;
    if mix.peak < 1e-5 {
        return Err("書き出す音がありません。MIDI と音声素材を読み込んでください。".to_string());
    }
    let ext = if request.format == "mp3" {
        "mp3"
    } else {
        "wav"
    };
    let base = sanitize_file_name(&request.project.name);
    let suggested = if base.to_lowercase().ends_with(&format!(".{ext}")) {
        base
    } else {
        format!("{base}.{ext}")
    };
    let picked = app
        .dialog()
        .file()
        .set_file_name(&suggested)
        .add_filter(ext.to_uppercase(), &[ext])
        .blocking_save_file();
    let Some(target) = picked else {
        return Ok(None);
    };
    let path = target.into_path().map_err(|e| e.to_string())?;

    let bytes = if ext == "wav" {
        // ビット深度の正規化は encode_wav が一元的に担う。
        encode_wav(
            mix.sample_rate as u32,
            &mix.left,
            &mix.right,
            mix.frames,
            request.wav_bit_depth.unwrap_or(24),
        )
    } else {
        encode_mp3_bytes(&mix, request.mp3_bitrate.unwrap_or(320))?
    };
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(Some(ExportResult {
        path: path.display().to_string(),
        bytes: bytes.len() as u64,
        duration_sec: mix.duration_sec,
    }))
}

fn encode_mp3_bytes(mix: &MixResult, kbps: u32) -> Result<Vec<u8>, String> {
    midi2otomad_core::media::encode_mp3(
        mix.sample_rate as u32,
        &mix.left,
        &mix.right,
        mix.frames,
        kbps,
    )
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            default_project,
            probe_media,
            open_midi,
            open_audio,
            ingest_paths,
            save_project,
            load_project,
            detect_pitch,
            preview_sample,
            set_mix,
            play,
            pause,
            stop,
            seek,
            set_loop,
            prune_samples,
            status,
            export
        ])
        .run(tauri::generate_context!())
        .expect("Tauri アプリの起動に失敗しました");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn file_stem_name_extracts_final_component() {
        assert_eq!(file_stem_name(Path::new("/home/user/song.mid")), "song.mid");
        assert_eq!(file_stem_name(Path::new("clip.wav")), "clip.wav");
        assert_eq!(file_stem_name(Path::new("/a/b/c.flac")), "c.flac");
        assert_eq!(file_stem_name(Path::new("")), "");
    }

    #[test]
    fn is_midi_matches_case_insensitively() {
        assert!(is_midi(Path::new("song.mid")));
        assert!(is_midi(Path::new("song.midi")));
        assert!(is_midi(Path::new("SONG.MID")));
        assert!(is_midi(Path::new("/path/to/Track.Midi")));
    }

    #[test]
    fn is_midi_rejects_other_extensions() {
        assert!(!is_midi(Path::new("song.wav")));
        assert!(!is_midi(Path::new("song.mp3")));
        assert!(!is_midi(Path::new("song")));
        assert!(!is_midi(Path::new("midi")));
    }

    #[test]
    fn is_project_file_matches_extension() {
        assert!(is_project_file(Path::new("song.m2oproj")));
        assert!(is_project_file(Path::new("/a/b/Song.M2OPROJ")));
        assert!(!is_project_file(Path::new("song.json")));
        assert!(!is_project_file(Path::new("song.mid")));
    }

    #[test]
    fn sanitize_file_name_strips_invalid_characters() {
        assert_eq!(sanitize_file_name("my song"), "my song");
        assert_eq!(
            sanitize_file_name("a/b\\c:d*e?f\"g<h>i|j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(sanitize_file_name("  .hidden.  "), "hidden");
        assert_eq!(sanitize_file_name(""), "音MAD");
        assert_eq!(sanitize_file_name("..."), "音MAD");
        assert_eq!(sanitize_file_name("曲名 (final).v2"), "曲名 (final).v2");
    }

    #[test]
    fn sanitize_file_name_escapes_windows_reserved_names() {
        assert_eq!(sanitize_file_name("CON"), "_CON");
        assert_eq!(sanitize_file_name("con"), "_con");
        assert_eq!(sanitize_file_name("nul.wav"), "_nul.wav");
        assert_eq!(sanitize_file_name("COM3"), "_COM3");
        assert_eq!(sanitize_file_name("LPT9.mid"), "_LPT9.mid");
        assert_eq!(sanitize_file_name("CONCERT"), "CONCERT");
        assert_eq!(sanitize_file_name("record"), "record");
    }

    #[test]
    fn default_project_is_valid_and_named() {
        let p = default_project();
        assert_eq!(p.name, DEFAULT_PROJECT_NAME);
        p.validate().expect("default project should validate");
    }

    #[test]
    fn probe_media_reports_version() {
        let probe = probe_media();
        assert!(probe.backend.contains("rust"));
        assert_eq!(probe.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn downmix_mono_averages_channels() {
        let mono = downmix_mono(&PcmAudio {
            sample_rate: 48000.0,
            channels: vec![vec![0.5, 0.5]],
            frames: 2,
        });
        assert_eq!(mono, vec![0.5, 0.5]);
        let stereo = downmix_mono(&PcmAudio {
            sample_rate: 48000.0,
            channels: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            frames: 2,
        });
        assert_eq!(stereo, vec![0.5, 0.5]);
        let empty = downmix_mono(&PcmAudio {
            sample_rate: 48000.0,
            channels: vec![],
            frames: 0,
        });
        assert!(empty.is_empty());
    }

    #[test]
    fn trim_region_respects_settings() {
        use midi2otomad_core::schema::{create_sample, Trim};
        let with_trim = |enabled: bool, start_sec: f64, end_sec: f64| {
            let mut s = create_sample("id", "n");
            s.trim = Trim {
                enabled,
                start_sec,
                end_sec,
            };
            s
        };
        assert_eq!(
            trim_region(&with_trim(false, 0.1, 0.5), 1000, 1000.0),
            (0, 1000)
        );
        assert_eq!(
            trim_region(&with_trim(true, 0.1, 0.5), 1000, 1000.0),
            (100, 500)
        );
        assert_eq!(
            trim_region(&with_trim(true, 0.2, 0.0), 1000, 1000.0),
            (200, 1000)
        );
        assert_eq!(trim_region(&with_trim(true, 0.0, 0.0), 1, 1000.0), (0, 1));
    }
}
