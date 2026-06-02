//! Tauri バックエンドとの橋渡し。`window.__TAURI__.core.invoke` を wasm-bindgen で取り込み、
//! 各コマンドを型付き async 関数として公開する。

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
    // Tauri の invoke 引数はプレーンな JS オブジェクトを期待する。既定の to_value は
    // Rust の Map を JS の `Map` にしてしまい note_sample_map などが失われるため、
    // json 互換シリアライザでオブジェクトとして直列化する。
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap_or(JsValue::NULL)
}

// --- DTO ------------------------------------------------------------------

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SampleDto {
    pub id: String,
    pub name: String,
    pub file_name: String,
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

// --- 引数構造体 -----------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviousArg<'a> {
    previous: &'a Project,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectArg<'a> {
    project: &'a Project,
}

#[derive(Serialize)]
struct IdArg<'a> {
    id: &'a str,
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
    pitch: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestArg<'a> {
    paths: Vec<String>,
    previous: &'a Project,
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

// --- コマンド --------------------------------------------------------------

pub async fn open_midi(previous: &Project) -> Result<Option<ImportResult>, String> {
    invoke("open_midi", to_args(&PreviousArg { previous })).await
}

pub async fn open_audio() -> Result<Vec<SampleDto>, String> {
    invoke("open_audio", JsValue::NULL).await
}

pub async fn ingest_paths(paths: Vec<String>, previous: &Project) -> Result<IngestResult, String> {
    invoke("ingest_paths", to_args(&IngestArg { paths, previous })).await
}

pub async fn remove_sample(id: &str) -> Result<(), String> {
    invoke_void("remove_sample", to_args(&IdArg { id })).await
}

pub async fn preview_sample(sample: &Sample, pitch: Option<i32>) -> Result<(), String> {
    invoke_void("preview_sample", to_args(&PreviewArg { sample, pitch })).await
}

pub async fn set_mix(project: &Project) -> Result<MixSummary, String> {
    invoke("set_mix", to_args(&ProjectArg { project })).await
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

// --- ドラッグ&ドロップ ------------------------------------------------------

#[derive(Deserialize)]
struct DragPayload {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct DragEvent {
    payload: DragPayload,
}

/// `tauri://drag-drop` を購読し、ドロップされたファイルパスをコールバックへ渡す。
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

/// 単純なウィンドウイベント（drag-enter / drag-leave）を購読する。
pub fn on_window_event(event: &'static str, cb: impl Fn() + 'static) {
    let closure = Closure::wrap(Box::new(move |_ev: JsValue| cb()) as Box<dyn FnMut(JsValue)>);
    wasm_bindgen_futures::spawn_local(async move {
        let _ = listen_raw(event, &closure).await;
        closure.forget();
    });
}
