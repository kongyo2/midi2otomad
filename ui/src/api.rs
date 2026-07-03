use midi2otomad_core::schema::{Project, Sample};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn invoke_raw(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    async fn listen_raw(
        event: &str,
        handler: &Closure<dyn FnMut(JsValue)>,
    ) -> Result<JsValue, JsValue>;
}

fn js_err(v: JsValue) -> String {
    v.as_string()
        .or_else(|| js_sys::JSON::stringify(&v).ok().and_then(|s| s.as_string()))
        .unwrap_or_else(|| "不明なエラー".to_string())
}

async fn invoke<R: DeserializeOwned>(cmd: &str, args: JsValue) -> Result<R, String> {
    let res = invoke_raw(cmd, args).await.map_err(js_err)?;
    serde_wasm_bindgen::from_value(res).map_err(|e| e.to_string())
}

async fn invoke_void(cmd: &str, args: JsValue) -> Result<(), String> {
    invoke_raw(cmd, args).await.map_err(js_err)?;
    Ok(())
}

fn to_args(value: &impl Serialize) -> JsValue {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap_or(JsValue::NULL)
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SampleDto {
    pub id: String,
    pub name: String,
    pub file_name: String,
    #[serde(default)]
    pub source_path: String,
    pub duration_sec: f64,
    pub peaks: Vec<f32>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub project: Project,
    pub track_count: usize,
    pub note_count: usize,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IngestResult {
    pub import: Option<ImportResult>,
    pub samples: Vec<SampleDto>,
    #[serde(default)]
    pub loaded: Option<ProjectLoad>,
    #[serde(default)]
    pub failed: Vec<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoad {
    pub project: Project,
    pub samples: Vec<SampleDto>,
    pub missing: Vec<String>,
}

#[derive(Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct MixSummary {
    pub duration_sec: f64,
    pub peak: f64,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ExportResult {
    pub path: String,
    pub bytes: u64,
    pub duration_sec: f64,
}

#[derive(Deserialize, Clone, Copy, Default)]
pub struct PlayerStatus {
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub level: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenMidiArg<'a> {
    previous: &'a Project,
    mode: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectArg<'a> {
    project: &'a Project,
    performance: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FromSecArg {
    from_sec: Option<f64>,
}

#[derive(Serialize)]
struct SecArg {
    sec: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewArg<'a> {
    sample: &'a Sample,
    layers: &'a [Sample],
    pitch: Option<i32>,
    drum_mode: bool,
    performance: bool,
}

#[derive(Serialize)]
struct SaveProjectArg<'a> {
    project: &'a Project,
}

#[derive(Serialize)]
struct EnabledArg {
    enabled: bool,
}

#[derive(Serialize)]
struct KeepArg<'a> {
    keep: &'a [String],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestArg<'a> {
    paths: Vec<String>,
    previous: &'a Project,
    mode: String,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct PitchEstimate {
    pub base_pitch: i32,
    pub tune_cents: f64,
    pub hz: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportRequestDto<'a> {
    project: &'a Project,
    format: String,
    wav_bit_depth: Option<u16>,
    mp3_bitrate: Option<u32>,
}

#[derive(Serialize)]
struct ExportArg<'a> {
    request: ExportRequestDto<'a>,
}

pub async fn open_midi(previous: &Project, mode: String) -> Result<Option<ImportResult>, String> {
    invoke("open_midi", to_args(&OpenMidiArg { previous, mode })).await
}

pub async fn open_audio() -> Result<Vec<SampleDto>, String> {
    invoke("open_audio", JsValue::NULL).await
}

pub async fn ingest_paths(
    paths: Vec<String>,
    previous: &Project,
    mode: String,
) -> Result<IngestResult, String> {
    invoke(
        "ingest_paths",
        to_args(&IngestArg {
            paths,
            previous,
            mode,
        }),
    )
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DetectArg<'a> {
    sample: &'a Sample,
}

pub async fn detect_pitch(sample: &Sample) -> Result<Option<PitchEstimate>, String> {
    invoke("detect_pitch", to_args(&DetectArg { sample })).await
}

pub async fn preview_sample(
    sample: &Sample,
    layers: &[Sample],
    pitch: Option<i32>,
    drum_mode: bool,
    performance: bool,
) -> Result<(), String> {
    invoke_void(
        "preview_sample",
        to_args(&PreviewArg {
            sample,
            layers,
            pitch,
            drum_mode,
            performance,
        }),
    )
    .await
}

pub async fn save_project(project: &Project) -> Result<Option<String>, String> {
    invoke("save_project", to_args(&SaveProjectArg { project })).await
}

pub async fn load_project() -> Result<Option<ProjectLoad>, String> {
    invoke("load_project", JsValue::NULL).await
}

pub async fn set_loop(enabled: bool) -> Result<(), String> {
    invoke_void("set_loop", to_args(&EnabledArg { enabled })).await
}

pub async fn prune_samples(keep: &[String]) -> Result<(), String> {
    invoke_void("prune_samples", to_args(&KeepArg { keep })).await
}

pub async fn set_mix(project: &Project, performance: bool) -> Result<MixSummary, String> {
    invoke(
        "set_mix",
        to_args(&ProjectArg {
            project,
            performance,
        }),
    )
    .await
}

pub async fn play(from_sec: Option<f64>) -> Result<(), String> {
    invoke_void("play", to_args(&FromSecArg { from_sec })).await
}

pub async fn pause() -> Result<(), String> {
    invoke_void("pause", JsValue::NULL).await
}

pub async fn stop() -> Result<(), String> {
    invoke_void("stop", JsValue::NULL).await
}

pub async fn seek(sec: f64) -> Result<(), String> {
    invoke_void("seek", to_args(&SecArg { sec })).await
}

pub async fn status() -> Result<PlayerStatus, String> {
    invoke("status", JsValue::NULL).await
}

pub async fn export(
    project: &Project,
    format: String,
    wav_bit_depth: Option<u16>,
    mp3_bitrate: Option<u32>,
) -> Result<Option<ExportResult>, String> {
    invoke(
        "export",
        to_args(&ExportArg {
            request: ExportRequestDto {
                project,
                format,
                wav_bit_depth,
                mp3_bitrate,
            },
        }),
    )
    .await
}

#[derive(Deserialize)]
struct DragPayload {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct DragEvent {
    payload: DragPayload,
}

pub fn on_drag_drop(cb: impl Fn(Vec<String>) + 'static) {
    let closure = Closure::wrap(Box::new(move |ev: JsValue| {
        if let Ok(e) = serde_wasm_bindgen::from_value::<DragEvent>(ev) {
            cb(e.payload.paths);
        }
    }) as Box<dyn FnMut(JsValue)>);
    wasm_bindgen_futures::spawn_local(async move {
        let _ = listen_raw("tauri://drag-drop", &closure).await;
        closure.forget();
    });
}

pub fn on_window_event(event: &'static str, cb: impl Fn() + 'static) {
    let closure = Closure::wrap(Box::new(move |_ev: JsValue| cb()) as Box<dyn FnMut(JsValue)>);
    wasm_bindgen_futures::spawn_local(async move {
        let _ = listen_raw(event, &closure).await;
        closure.forget();
    });
}
