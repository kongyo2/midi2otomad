use std::collections::{HashMap, HashSet, VecDeque};

use leptos::prelude::*;
use midi2otomad_core::audio::MAX_LAYERS;
use midi2otomad_core::music::midi_to_note_name;
use midi2otomad_core::schema::{
    create_empty_project, create_sample, Loop, Project, Sample, Track, Trim, DEFAULT_PROJECT_NAME,
};
use wasm_bindgen_futures::spawn_local;

use crate::api::{self, ImportResult, PlayerStatus, ProjectLoad, SampleDto};

/// Undo 履歴の最大保持数。
const UNDO_LIMIT: usize = 100;
/// この時間内の連続編集（スライダードラッグ等）は 1 つの Undo ステップにまとめる。
const SNAPSHOT_COALESCE_MS: f64 = 400.0;

#[derive(Clone, Copy)]
pub struct Studio {
    pub project: RwSignal<Project>,
    pub peaks: RwSignal<HashMap<String, Vec<f32>>>,
    pub selected_track: RwSignal<Option<String>>,
    pub selected_sample: RwSignal<Option<String>>,
    pub status: RwSignal<PlayerStatus>,
    pub busy: RwSignal<Option<String>>,
    pub toast: RwSignal<Option<String>>,
    pub edit_seq: RwSignal<u64>,
    pub mixed_seq: RwSignal<u64>,
    pub drag_active: RwSignal<bool>,
    pub import_mode: RwSignal<String>,
    pub performance_mode: RwSignal<bool>,
    pub loop_enabled: RwSignal<bool>,
    pub undo_stack: RwSignal<Vec<Project>>,
    pub redo_stack: RwSignal<Vec<Project>>,
    last_snapshot_ms: RwSignal<f64>,
}

fn sample_from_dto(dto: &SampleDto) -> Sample {
    let mut s = create_sample(&dto.id, &dto.name);
    s.file_name = dto.file_name.clone();
    s.source_path = dto.source_path.clone();
    s.duration_sec = dto.duration_sec;
    s.trim = Trim {
        enabled: false,
        start_sec: 0.0,
        end_sec: dto.duration_sec,
    };
    s.loop_region = Loop {
        enabled: false,
        start_sec: 0.0,
        end_sec: dto.duration_sec,
    };
    s
}

/// link_ids をたどって重ねる素材を集める（循環・重複は除外）。
/// ミキサーの collect_sources と同じく「ルートを含めて MAX_LAYERS まで」。
fn collect_layers(project: &Project, root: &Sample) -> Vec<Sample> {
    if root.link_ids.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(root.id.clone());
    let mut queue: VecDeque<String> = root.link_ids.iter().cloned().collect();
    while let Some(id) = queue.pop_front() {
        if out.len() + 1 >= MAX_LAYERS {
            break;
        }
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(sample) = project.samples.iter().find(|s| s.id == id) {
            for link in &sample.link_ids {
                queue.push_back(link.clone());
            }
            out.push(sample.clone());
        }
    }
    out
}

impl Studio {
    pub fn new(project: Project) -> Self {
        Self {
            project: RwSignal::new(project),
            peaks: RwSignal::new(HashMap::new()),
            selected_track: RwSignal::new(None),
            selected_sample: RwSignal::new(None),
            status: RwSignal::new(PlayerStatus::default()),
            busy: RwSignal::new(None),
            toast: RwSignal::new(None),
            edit_seq: RwSignal::new(1),
            mixed_seq: RwSignal::new(0),
            drag_active: RwSignal::new(false),
            import_mode: RwSignal::new("auto".to_string()),
            performance_mode: RwSignal::new(false),
            loop_enabled: RwSignal::new(false),
            undo_stack: RwSignal::new(Vec::new()),
            redo_stack: RwSignal::new(Vec::new()),
            last_snapshot_ms: RwSignal::new(f64::NEG_INFINITY),
        }
    }

    pub fn set_performance_mode(&self, on: bool) {
        self.performance_mode.set(on);
        self.mark_dirty();
    }

    pub fn show_toast(&self, message: impl Into<String>) {
        let toast = self.toast;
        toast.set(Some(message.into()));
        gloo_timers::callback::Timeout::new(3200, move || toast.set(None)).forget();
    }

    pub fn mark_dirty(&self) {
        self.edit_seq.update(|n| *n += 1);
    }

    fn snapshot(&self) -> Project {
        self.project.get_untracked()
    }

    /// 現在の状態を Undo スタックへ積む。`coalesce` はスライダードラッグ等の
    /// 連続編集を 1 ステップにまとめるためのフラグ。まとめるのは連続する
    /// ライブ編集同士だけで、離散編集 (edit) を巻き込まない。
    fn push_undo(&self, coalesce: bool) {
        if coalesce {
            let now = js_sys::Date::now();
            let last = self.last_snapshot_ms.get_untracked();
            self.last_snapshot_ms.set(now);
            if now - last < SNAPSHOT_COALESCE_MS {
                return;
            }
        } else {
            self.last_snapshot_ms.set(f64::NEG_INFINITY);
        }
        let snapshot = self.snapshot();
        self.undo_stack.update(|stack| {
            stack.push(snapshot);
            if stack.len() > UNDO_LIMIT {
                stack.remove(0);
            }
        });
        self.redo_stack.update(|stack| stack.clear());
    }

    /// Undo 1 ステップとして記録しつつプロジェクトを編集する。
    /// チェックボックスやセレクトなど離散的な操作向け。
    pub fn edit(&self, f: impl FnOnce(&mut Project)) {
        self.push_undo(false);
        self.project.update(f);
        self.mark_dirty();
    }

    /// スライダードラッグなどの連続編集向け。短時間の連続操作を
    /// 1 つの Undo ステップにまとめる。
    pub fn edit_live(&self, f: impl FnOnce(&mut Project)) {
        self.push_undo(true);
        self.project.update(f);
        self.mark_dirty();
    }

    pub fn undo(&self) {
        let Some(previous) = self.undo_stack.try_update(|stack| stack.pop()).flatten() else {
            return;
        };
        let current = self.snapshot();
        self.redo_stack.update(|stack| stack.push(current));
        self.project.set(previous);
        self.last_snapshot_ms.set(f64::NEG_INFINITY);
        self.mark_dirty();
    }

    pub fn redo(&self) {
        let Some(next) = self.redo_stack.try_update(|stack| stack.pop()).flatten() else {
            return;
        };
        let current = self.snapshot();
        self.undo_stack.update(|stack| stack.push(current));
        self.project.set(next);
        self.last_snapshot_ms.set(f64::NEG_INFINITY);
        self.mark_dirty();
    }

    fn apply_import(&self, import: ImportResult) {
        self.push_undo(false);
        self.stop();
        let first_track = import.project.tracks.first().map(|t| t.id.clone());
        self.project.set(import.project);
        self.selected_track.set(first_track);
        self.mark_dirty();
        self.show_toast(format!(
            "読み込みました — {} トラック / {} ノート",
            import.track_count, import.note_count
        ));
    }

    fn apply_samples(&self, samples: Vec<SampleDto>) {
        self.apply_samples_inner(samples, true);
    }

    /// `record_undo = false` は、直前の apply_import と同じ Undo ステップに
    /// まとめる場合（1 回のドロップで MIDI と音声を同時に読み込むケース）。
    fn apply_samples_inner(&self, samples: Vec<SampleDto>, record_undo: bool) {
        if samples.is_empty() {
            return;
        }
        let count = samples.len();
        let first_id = samples[0].id.clone();
        self.peaks.update(|map| {
            for dto in &samples {
                map.insert(dto.id.clone(), dto.peaks.clone());
            }
        });
        let mutate = |p: &mut Project| {
            for dto in &samples {
                let assign = p.samples.is_empty();
                let sample = sample_from_dto(dto);
                let id = sample.id.clone();
                p.samples.push(sample);
                if assign {
                    for track in &mut p.tracks {
                        if track.default_sample_id.is_none() {
                            track.default_sample_id = Some(id.clone());
                        }
                    }
                }
            }
        };
        if record_undo {
            self.edit(mutate);
        } else {
            self.project.update(mutate);
            self.mark_dirty();
        }
        self.selected_sample.set(Some(first_id));
        self.show_toast(format!("{count} 個の音声素材を追加しました"));
    }

    fn apply_loaded(&self, load: ProjectLoad) {
        self.push_undo(false);
        // 旧プロジェクトのミックスが鳴り続けないように止める。
        self.stop();
        self.peaks.update(|map| {
            for dto in &load.samples {
                map.insert(dto.id.clone(), dto.peaks.clone());
            }
        });
        let name = load.project.name.clone();
        let first_track = load.project.tracks.first().map(|t| t.id.clone());
        let first_sample = load.project.samples.first().map(|s| s.id.clone());
        self.project.set(load.project);
        self.selected_track.set(first_track);
        self.selected_sample.set(first_sample);
        self.mark_dirty();
        self.prune_bank();
        if load.missing.is_empty() {
            self.show_toast(format!("プロジェクト「{name}」を読み込みました"));
        } else {
            self.show_toast(format!(
                "「{name}」を読み込みました — 音声が見つからない素材: {}",
                load.missing.join("、")
            ));
        }
    }

    /// 現在のプロジェクトと Undo/Redo 履歴のどこからも参照されなくなった
    /// デコード済み音声をバックエンドのバンクから解放する。
    /// プロジェクト読込のタイミングで呼び、セッション中のメモリ増加を抑える。
    fn prune_bank(&self) {
        let mut keep: HashSet<String> = HashSet::new();
        let collect = |keep: &mut HashSet<String>, p: &Project| {
            keep.extend(p.samples.iter().map(|s| s.id.clone()));
        };
        self.project.with_untracked(|p| collect(&mut keep, p));
        self.undo_stack.with_untracked(|stack| {
            for p in stack {
                collect(&mut keep, p);
            }
        });
        self.redo_stack.with_untracked(|stack| {
            for p in stack {
                collect(&mut keep, p);
            }
        });
        let keep: Vec<String> = keep.into_iter().collect();
        spawn_local(async move {
            let _ = api::prune_samples(&keep).await;
        });
    }

    pub fn new_project(&self) {
        self.push_undo(false);
        self.stop();
        self.project.set(create_empty_project(DEFAULT_PROJECT_NAME));
        self.selected_track.set(None);
        self.selected_sample.set(None);
        self.mark_dirty();
        self.show_toast("新規プロジェクトを作成しました（元に戻す: Ctrl+Z）");
    }

    pub fn save_project(&self) {
        let this = *self;
        spawn_local(async move {
            let project = this.snapshot();
            match api::save_project(&project).await {
                Ok(Some(path)) => this.show_toast(format!("保存しました: {path}")),
                Ok(None) => {}
                Err(e) => this.show_toast(format!("保存に失敗しました: {e}")),
            }
        });
    }

    pub fn load_project(&self) {
        let this = *self;
        spawn_local(async move {
            this.busy.set(Some("プロジェクトを読み込み中…".into()));
            match api::load_project().await {
                Ok(Some(load)) => this.apply_loaded(load),
                Ok(None) => {}
                Err(e) => this.show_toast(format!("プロジェクトの読み込みに失敗しました: {e}")),
            }
            this.busy.set(None);
        });
    }

    pub fn open_midi(&self) {
        let this = *self;
        spawn_local(async move {
            let previous = this.snapshot();
            let mode = this.import_mode.get_untracked();
            match api::open_midi(&previous, mode).await {
                Ok(Some(import)) => this.apply_import(import),
                Ok(None) => {}
                Err(e) => this.show_toast(format!("MIDI 読み込みに失敗しました: {e}")),
            }
        });
    }

    pub fn open_audio(&self) {
        let this = *self;
        spawn_local(async move {
            this.busy.set(Some("音声素材をデコード中…".into()));
            match api::open_audio().await {
                Ok(samples) => this.apply_samples(samples),
                Err(e) => this.show_toast(format!("音声の読み込みに失敗しました: {e}")),
            }
            this.busy.set(None);
        });
    }

    pub fn ingest_dropped(&self, paths: Vec<String>) {
        let this = *self;
        spawn_local(async move {
            this.busy.set(Some("ファイルを読み込み中…".into()));
            let previous = this.snapshot();
            let mode = this.import_mode.get_untracked();
            match api::ingest_paths(paths, &previous, mode).await {
                Ok(result) => {
                    if let Some(loaded) = result.loaded {
                        this.apply_loaded(loaded);
                    } else {
                        // MIDI と音声を同時にドロップしたときは 1 つの
                        // Undo ステップにまとめる。
                        let had_import = result.import.is_some();
                        if let Some(import) = result.import {
                            this.apply_import(import);
                        }
                        this.apply_samples_inner(result.samples, !had_import);
                    }
                    if !result.failed.is_empty() {
                        this.show_toast(format!(
                            "読み込めなかったファイル: {}",
                            result.failed.join(" ／ ")
                        ));
                    }
                }
                Err(e) => this.show_toast(format!("読み込みに失敗しました: {e}")),
            }
            this.busy.set(None);
        });
    }

    /// 素材をプロジェクトから外す。デコード済み音声はバンクに残したままに
    /// して、Undo で戻したときに音が出るようにする。
    pub fn remove_sample(&self, id: String) {
        self.edit(|p| {
            p.samples.retain(|s| s.id != id);
            for track in &mut p.tracks {
                if track.default_sample_id.as_deref() == Some(id.as_str()) {
                    track.default_sample_id = None;
                }
                track.note_sample_map.retain(|_, v| v != &id);
            }
            for sample in &mut p.samples {
                sample.link_ids.retain(|l| l != &id);
            }
        });
        if self.selected_sample.get_untracked().as_deref() == Some(id.as_str()) {
            self.selected_sample.set(None);
        }
    }

    pub fn detect_pitch(&self, id: String) {
        let this = *self;
        spawn_local(async move {
            let Some(sample) = this.project.with_untracked(|p| find_sample(p, &id)) else {
                return;
            };
            match api::detect_pitch(&sample).await {
                Ok(Some(est)) => {
                    this.update_sample(&id, move |s| {
                        s.base_pitch = est.base_pitch.clamp(0, 127);
                        s.tune_cents = est.tune_cents.clamp(-100.0, 100.0);
                    });
                    this.show_toast(format!(
                        "基準ピッチを推定: {} ({:.1} Hz)",
                        midi_to_note_name(est.base_pitch as f64),
                        est.hz
                    ));
                }
                Ok(None) => this.show_toast("ピッチを推定できませんでした"),
                Err(e) => this.show_toast(format!("ピッチ推定に失敗しました: {e}")),
            }
        });
    }

    pub fn update_sample(&self, id: &str, f: impl FnOnce(&mut Sample)) {
        self.edit_live(|p| {
            if let Some(s) = p.samples.iter_mut().find(|s| s.id == id) {
                f(s);
            }
        });
    }

    pub fn update_track(&self, id: &str, f: impl FnOnce(&mut Track)) {
        self.edit_live(|p| {
            if let Some(t) = p.tracks.iter_mut().find(|t| t.id == id) {
                f(t);
            }
        });
    }

    pub fn set_note_sample(&self, track_id: &str, note: i32, sample_id: Option<String>) {
        self.edit(|p| {
            if let Some(t) = p.tracks.iter_mut().find(|t| t.id == track_id) {
                match sample_id {
                    Some(sid) => {
                        t.note_sample_map.insert(note.to_string(), sid);
                    }
                    None => {
                        t.note_sample_map.remove(&note.to_string());
                    }
                }
            }
        });
    }

    fn ensure_mix_then<F>(&self, after: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        let this = *self;
        spawn_local(async move {
            let seq = this.edit_seq.get_untracked();
            if this.mixed_seq.get_untracked() != seq {
                let project = this.snapshot();
                match api::set_mix(&project, this.performance_mode.get_untracked()).await {
                    Ok(_) => this.mixed_seq.set(seq),
                    Err(e) => {
                        this.show_toast(format!("ミックスの生成に失敗しました: {e}"));
                        return;
                    }
                }
            }
            after.await;
        });
    }

    pub fn play(&self, from_sec: Option<f64>) {
        self.ensure_mix_then(async move {
            let _ = api::play(from_sec).await;
        });
    }

    pub fn pause(&self) {
        spawn_local(async move {
            let _ = api::pause().await;
        });
    }

    pub fn stop(&self) {
        spawn_local(async move {
            let _ = api::stop().await;
        });
    }

    pub fn toggle_play(&self) {
        if self.status.get_untracked().playing {
            self.pause();
        } else {
            self.play(None);
        }
    }

    pub fn toggle_loop(&self) {
        let on = !self.loop_enabled.get_untracked();
        self.loop_enabled.set(on);
        spawn_local(async move {
            let _ = api::set_loop(on).await;
        });
    }

    pub fn seek(&self, sec: f64) {
        self.ensure_mix_then(async move {
            let _ = api::seek(sec).await;
        });
    }

    pub fn preview_sample(&self, sample: Sample) {
        self.preview_with(sample, None, false);
    }

    pub fn preview_sample_at(&self, sample: Sample, pitch: Option<i32>) {
        self.preview_with(sample, pitch, false);
    }

    /// 素材を指定ピッチで試聴する（レイヤー先も同時に鳴らす）。
    /// `drum_mode` はトラックの設定をプレビューにも反映し、レイヤー素材が
    /// 実再生と同じピッチで鳴るようにする。プレビューはプレイヤーの
    /// バッファを書き換えるため、次回再生時にミックスを作り直すよう
    /// dirty マークを付ける。
    fn preview_with(&self, sample: Sample, pitch: Option<i32>, drum_mode: bool) {
        let this = *self;
        spawn_local(async move {
            let layers = this.project.with_untracked(|p| collect_layers(p, &sample));
            let performance = this.performance_mode.get_untracked();
            if let Err(e) =
                api::preview_sample(&sample, &layers, pitch, drum_mode, performance).await
            {
                this.show_toast(format!("試聴に失敗しました: {e}"));
            }
            this.mark_dirty();
        });
    }

    /// トラック上のノート番号を、割り当てられた素材で試聴する。
    pub fn preview_note(&self, track_id: &str, pitch: i32) {
        let resolved = self.project.with_untracked(|p| {
            let track = p.tracks.iter().find(|t| t.id == track_id)?;
            let sample_id = track.sample_id_for_pitch(pitch)?;
            let sample = p.samples.iter().find(|s| &s.id == sample_id)?.clone();
            Some((sample, track.drum_mode))
        });
        match resolved {
            Some((sample, drum_mode)) => self.preview_with(sample, Some(pitch), drum_mode),
            None => self.show_toast("このノートに割り当てられた素材がありません"),
        }
    }

    pub fn export(&self, format: String, wav_bit_depth: Option<u16>, mp3_bitrate: Option<u32>) {
        let this = *self;
        spawn_local(async move {
            this.busy.set(Some("ミックスを書き出し中…".into()));
            let project = this.snapshot();
            match api::export(&project, format, wav_bit_depth, mp3_bitrate).await {
                Ok(Some(result)) => {
                    let kb = result.bytes / 1024;
                    this.show_toast(format!("書き出し完了: {} ({} KB)", result.path, kb));
                }
                Ok(None) => this.show_toast("書き出しをキャンセルしました"),
                Err(e) => this.show_toast(format!("書き出しに失敗しました: {e}")),
            }
            this.busy.set(None);
        });
    }

    pub fn start_status_polling(&self) {
        let this = *self;
        gloo_timers::callback::Interval::new(60, move || {
            spawn_local(async move {
                if let Ok(s) = api::status().await {
                    this.status.set(s);
                }
            });
        })
        .forget();
    }
}

pub fn find_sample(project: &Project, id: &str) -> Option<Sample> {
    project.samples.iter().find(|s| s.id == id).cloned()
}

pub fn project_duration(project: &Project) -> f64 {
    project
        .tracks
        .iter()
        .flat_map(|t| t.notes.iter())
        .map(|n| n.start_sec + n.duration_sec)
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midi2otomad_core::schema::parse_project;
    use serde_json::json;

    fn project_with_notes() -> Project {
        parse_project(json!({
            "version": 1, "name": "t",
            "samples": [{ "id": "s1", "name": "kick" }, { "id": "s2", "name": "snare" }],
            "tracks": [
                { "id": "t1", "name": "a", "notes": [
                    { "pitch": 60, "startSec": 0.0, "durationSec": 1.0 },
                    { "pitch": 62, "startSec": 2.0, "durationSec": 0.5 }
                ] },
                { "id": "t2", "name": "b", "notes": [
                    { "pitch": 48, "startSec": 1.0, "durationSec": 3.0 }
                ] }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn project_duration_is_last_note_end() {
        assert!((project_duration(&project_with_notes()) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn project_duration_zero_without_notes() {
        let empty = parse_project(json!({ "version": 1, "name": "e" })).unwrap();
        assert_eq!(project_duration(&empty), 0.0);
    }

    #[test]
    fn find_sample_by_id() {
        let p = project_with_notes();
        assert_eq!(
            find_sample(&p, "s2").map(|s| s.name),
            Some("snare".to_string())
        );
        assert!(find_sample(&p, "missing").is_none());
    }

    #[test]
    fn sample_from_dto_sets_loop_to_full_clip() {
        let dto = SampleDto {
            id: "x".to_string(),
            name: "clip".to_string(),
            file_name: "clip.wav".to_string(),
            source_path: "/tmp/clip.wav".to_string(),
            duration_sec: 2.5,
            peaks: vec![0.1, 0.2],
        };
        let s = sample_from_dto(&dto);
        assert_eq!(s.id, "x");
        assert_eq!(s.name, "clip");
        assert_eq!(s.file_name, "clip.wav");
        assert_eq!(s.source_path, "/tmp/clip.wav");
        assert_eq!(s.duration_sec, 2.5);
        assert!(!s.loop_region.enabled);
        assert_eq!(s.loop_region.end_sec, 2.5);
        assert!(!s.trim.enabled);
        assert_eq!(s.trim.end_sec, 2.5);
    }

    #[test]
    fn collect_layers_follows_links_without_cycles() {
        let p = parse_project(json!({
            "version": 1, "name": "L",
            "samples": [
                { "id": "a", "name": "a", "linkIds": ["b", "a"] },
                { "id": "b", "name": "b", "linkIds": ["c", "a"] },
                { "id": "c", "name": "c" },
                { "id": "unrelated", "name": "u" }
            ]
        }))
        .unwrap();
        let root = p.samples.iter().find(|s| s.id == "a").unwrap();
        let layers = collect_layers(&p, root);
        let ids: Vec<&str> = layers.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c"]);

        let solo = p.samples.iter().find(|s| s.id == "c").unwrap();
        assert!(collect_layers(&p, solo).is_empty());
    }
}
