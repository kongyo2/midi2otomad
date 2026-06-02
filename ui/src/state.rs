//! グローバル状態とアクション。React の StudioContext を Leptos のシグナルへ移植したもの。
//! プロジェクト本体はフロントエンドが保持し、ミックス・再生・書き出しはバックエンドへ委譲する。

use std::collections::HashMap;

use leptos::prelude::*;
use midi2otomad_core::schema::{create_sample, Loop, Project, Sample, Track};
use wasm_bindgen_futures::spawn_local;

use crate::api::{self, ImportResult, PlayerStatus, SampleDto};

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
}

fn sample_from_dto(dto: &SampleDto) -> Sample {
    let mut s = create_sample(&dto.id, &dto.name);
    s.file_name = dto.file_name.clone();
    s.duration_sec = dto.duration_sec;
    s.loop_region = Loop {
        enabled: false,
        start_sec: 0.0,
        end_sec: dto.duration_sec,
    };
    s
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
        }
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

    fn apply_import(&self, import: ImportResult) {
        let first_track = import.project.tracks.first().map(|t| t.id.clone());
        self.project.set(import.project);
        self.selected_track.set(first_track);
        self.mark_dirty();
        self.show_toast(format!(
            "読み込みました — {} トラック / {} ノート",
            import.track_count, import.note_count
        ));
    }

    /// 取り込み結果を適用する。ただし呼び出し中（ファイル選択ダイアログやデコード待ち）に
    /// ユーザー編集が入っていた場合は、古いスナップショット由来の結果で現在の編集を
    /// 上書きしないよう取り込みを中止する。
    fn apply_import_guarded(&self, import: ImportResult, edit_gen: u64) {
        if self.edit_seq.get_untracked() != edit_gen {
            self.show_toast(
                "読み込み中に編集があったため取り込みを中止しました。もう一度読み込んでください。",
            );
            return;
        }
        self.apply_import(import);
    }

    fn apply_samples(&self, samples: Vec<SampleDto>) {
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
        self.project.update(|p| {
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
        });
        self.selected_sample.set(Some(first_id));
        self.mark_dirty();
        self.show_toast(format!("{count} 個の音声素材を追加しました"));
    }

    pub fn open_midi(&self) {
        let this = *self;
        spawn_local(async move {
            let edit_gen = this.edit_seq.get_untracked();
            let previous = this.snapshot();
            match api::open_midi(&previous).await {
                Ok(Some(import)) => this.apply_import_guarded(import, edit_gen),
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
            let edit_gen = this.edit_seq.get_untracked();
            this.busy.set(Some("ファイルを読み込み中…".into()));
            let previous = this.snapshot();
            match api::ingest_paths(paths, &previous).await {
                Ok(result) => {
                    if let Some(import) = result.import {
                        this.apply_import_guarded(import, edit_gen);
                    }
                    this.apply_samples(result.samples);
                }
                Err(e) => this.show_toast(format!("読み込みに失敗しました: {e}")),
            }
            this.busy.set(None);
        });
    }

    pub fn remove_sample(&self, id: String) {
        if self.busy.get_untracked().is_some() {
            self.show_toast("処理中は素材を削除できません");
            return;
        }
        let this = *self;
        spawn_local(async move {
            let _ = api::remove_sample(&id).await;
            this.project.update(|p| {
                p.samples.retain(|s| s.id != id);
                for track in &mut p.tracks {
                    if track.default_sample_id.as_deref() == Some(id.as_str()) {
                        track.default_sample_id = None;
                    }
                    track.note_sample_map.retain(|_, v| v != &id);
                }
            });
            this.peaks.update(|map| {
                map.remove(&id);
            });
            if this.selected_sample.get_untracked().as_deref() == Some(id.as_str()) {
                this.selected_sample.set(None);
            }
            this.mark_dirty();
        });
    }

    pub fn update_sample(&self, id: &str, f: impl FnOnce(&mut Sample)) {
        self.project.update(|p| {
            if let Some(s) = p.samples.iter_mut().find(|s| s.id == id) {
                f(s);
            }
        });
        self.mark_dirty();
    }

    pub fn update_track(&self, id: &str, f: impl FnOnce(&mut Track)) {
        self.project.update(|p| {
            if let Some(t) = p.tracks.iter_mut().find(|t| t.id == id) {
                f(t);
            }
        });
        self.mark_dirty();
    }

    pub fn set_note_sample(&self, track_id: &str, note: i32, sample_id: Option<String>) {
        self.project.update(|p| {
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
        self.mark_dirty();
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
                // より新しいレンダリングに追い越されてプロジェクトのミックスが実際には
                // 再生バッファへ載らなかった場合は「ミックス済み」と記録せず、次の再生で
                // 再レンダリングさせる。
                if let Ok(summary) = api::set_mix(&project).await {
                    if summary.loaded {
                        this.mixed_seq.set(seq);
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

    pub fn seek(&self, sec: f64) {
        self.ensure_mix_then(async move {
            let _ = api::seek(sec).await;
        });
    }

    pub fn preview_sample(&self, sample: Sample) {
        let this = *self;
        spawn_local(async move {
            let _ = api::preview_sample(&sample, None).await;
            // 試聴はプレイヤーのバッファを差し替えるので、次の再生/シークで
            // プロジェクトのミックスを必ず再レンダリングさせる。
            this.mark_dirty();
        });
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

    /// バックエンドの再生状態を定期取得して `status` を更新する。
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

/// プロジェクト中の素材を id から取得（追跡あり）。
pub fn find_sample(project: &Project, id: &str) -> Option<Sample> {
    project.samples.iter().find(|s| s.id == id).cloned()
}

/// 全ノートの最終終了時刻（秒）。
pub fn project_duration(project: &Project) -> f64 {
    project
        .tracks
        .iter()
        .flat_map(|t| t.notes.iter())
        .map(|n| n.start_sec + n.duration_sec)
        .fold(0.0, f64::max)
}
