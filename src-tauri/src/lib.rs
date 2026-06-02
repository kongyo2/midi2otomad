//! Tauri 2 バックエンド。デコード済み音声バンクと cpal 再生エンジンを保持し、
//! フロントエンド (Leptos) からのコマンドで MIDI 取り込み・音声デコード・ミックス・
//! 再生・書き出しを行う。重い処理（DSP・コーデック）はすべて `midi2otomad-core`。

mod player;

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use midi2otomad_core::audio::{build_waveform_peaks, mix_project, MixOptions, MixResult, PcmAudio};
use midi2otomad_core::id::make_id;
use midi2otomad_core::media::{decode_audio, encode_wav};
use midi2otomad_core::midi::midi_to_project;
use midi2otomad_core::schema::{
    create_empty_project, parse_project, Project, Sample, DEFAULT_PROJECT_NAME,
};

use player::{Player, PlayerStatus};

const PEAK_BUCKETS: usize = 600;
const AUDIO_EXTENSIONS: [&str; 8] = ["wav", "mp3", "ogg", "flac", "m4a", "aac", "aif", "aiff"];

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
}

// --- DTO ------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleDto {
    id: String,
    name: String,
    file_name: String,
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

// --- ヘルパー --------------------------------------------------------------

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

fn decode_and_store(state: &AppState, path: &std::path::Path) -> Result<SampleDto, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let pcm = decode_audio(&bytes)?;
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
        duration_sec,
        peaks,
    })
}

fn load_into_player(state: &AppState, mix: &MixResult) {
    if let Some(player) = &state.player {
        player.set_mix(&mix.left, &mix.right, mix.sample_rate);
    }
}

// --- コマンド --------------------------------------------------------------

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

#[tauri::command(async)]
fn open_midi(app: AppHandle, previous: Option<Project>) -> Result<Option<ImportResult>, String> {
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
    let result = midi_to_project(&bytes, &file_stem_name(&path), previous.as_ref())?;
    Ok(Some(ImportResult {
        project: result.project,
        track_count: result.track_count,
        note_count: result.note_count,
    }))
}

#[tauri::command(async)]
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

#[tauri::command(async)]
fn ingest_paths(
    state: State<AppState>,
    paths: Vec<String>,
    previous: Option<Project>,
) -> Result<IngestResult, String> {
    let mut import = None;
    let mut samples = Vec::new();
    for path_str in paths {
        let path = std::path::PathBuf::from(&path_str);
        if is_midi(&path) {
            if import.is_none() {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                let result = midi_to_project(&bytes, &file_stem_name(&path), previous.as_ref())?;
                import = Some(ImportResult {
                    project: result.project,
                    track_count: result.track_count,
                    note_count: result.note_count,
                });
            }
        } else {
            samples.push(decode_and_store(&state, &path)?);
        }
    }
    Ok(IngestResult { import, samples })
}

#[tauri::command]
fn remove_sample(state: State<AppState>, id: String) -> Result<(), String> {
    state
        .bank
        .lock()
        .map_err(|_| "バンクのロックに失敗".to_string())?
        .remove(&id);
    Ok(())
}

#[tauri::command(async)]
fn preview_sample(
    state: State<AppState>,
    sample: Sample,
    pitch: Option<i32>,
) -> Result<(), String> {
    let note_pitch = pitch.unwrap_or(sample.base_pitch);
    let duration = {
        let bank = state
            .bank
            .lock()
            .map_err(|_| "バンクのロックに失敗".to_string())?;
        let pcm = bank.get(&sample.id).ok_or("素材がデコードされていません")?;
        let natural = (pcm.frames as f64 / pcm.sample_rate).min(2.2);
        if sample.loop_region.enabled {
            1.4
        } else {
            natural
        }
        .max(0.05)
    };
    let sample_value = serde_json::to_value(&sample).map_err(|e| e.to_string())?;
    let project = parse_project(json!({
        "version": 1,
        "name": "preview",
        "sampleRate": 48000,
        "samples": [sample_value],
        "tracks": [{
            "id": "preview",
            "name": "preview",
            "defaultSampleId": sample.id,
            "notes": [{ "pitch": note_pitch, "startSec": 0, "durationSec": duration, "velocity": 127 }]
        }]
    }))?;
    let mix = state.render(
        &project,
        MixOptions {
            limiter: Some(false),
            tail_sec: Some(0.1),
        },
    )?;
    load_into_player(&state, &mix);
    if let Some(player) = &state.player {
        player.play(Some(0.0));
    }
    Ok(())
}

#[tauri::command(async)]
fn set_mix(state: State<AppState>, project: Project) -> Result<MixSummary, String> {
    let mix = state.render(&project, MixOptions::default())?;
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

#[tauri::command(async)]
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
    let suggested = if request
        .project
        .name
        .to_lowercase()
        .ends_with(&format!(".{ext}"))
    {
        request.project.name.clone()
    } else {
        format!("{}.{ext}", request.project.name)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
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
            remove_sample,
            preview_sample,
            set_mix,
            play,
            pause,
            stop,
            seek,
            status,
            export
        ])
        .run(tauri::generate_context!())
        .expect("Tauri アプリの起動に失敗しました");
}
