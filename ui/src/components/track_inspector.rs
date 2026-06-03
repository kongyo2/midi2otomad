use leptos::prelude::*;
use midi2otomad_core::music::midi_to_note_name;
use midi2otomad_core::schema::{StopMode, VoicePriority};

use crate::enums::SelectValue;
use crate::format::format_db;
use crate::state::Studio;
use crate::widgets::range_row;

const PRIORITY_OPTIONS: [(&str, &str); 4] = [
    ("newest", "新しい音を優先"),
    ("oldest", "古い音を優先"),
    ("highest", "高い音を優先"),
    ("lowest", "低い音を優先"),
];

const STOP_OPTIONS: [(&str, &str); 4] = [
    ("none", "重ねる（停止しない）"),
    ("pitch", "同じ音程を停止"),
    ("sample", "同じ素材を停止"),
    ("track", "トラック全体を停止"),
];

fn pan_display(pan: f64) -> String {
    if pan == 0.0 {
        "C".to_string()
    } else if pan < 0.0 {
        format!("L{}", (-pan * 100.0).round() as i64)
    } else {
        format!("R{}", (pan * 100.0).round() as i64)
    }
}

#[component]
pub fn TrackInspector() -> impl IntoView {
    let s = expect_context::<Studio>();

    view! {
        <section class="panel">
            {move || {
                let Some(id) = s.selected_track.get() else {
                    return view! {
                        <div>
                            <h2 class="panel__heading">"トラック設定"</h2>
                            <p class="panel__muted">
                                "タイムラインのトラック名をクリックすると、音量・パン・素材割り当てを編集できます。"
                            </p>
                        </div>
                    }
                        .into_any();
                };

                macro_rules! tget {
                    (|$x:ident| $body:expr) => {{
                        let id = id.clone();
                        Signal::derive(move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id).map(|$x| $body).unwrap_or(0.0)))
                    }};
                }
                macro_rules! tupd {
                    (|$x:ident, $v:ident| $body:expr) => {{
                        let id = id.clone();
                        move |$v: f64| s.update_track(&id, move |$x| { $body; })
                    }};
                }

                let id_name = id.clone();
                let id_name2 = id.clone();
                let id_default = id.clone();
                let id_default2 = id.clone();
                let id_prio = id.clone();
                let id_prio2 = id.clone();
                let id_stop = id.clone();
                let id_stop2 = id.clone();
                let id_voices = id.clone();
                let id_voices2 = id.clone();
                let id_voices3 = id.clone();
                let id_pitches = id.clone();
                let id_count = id.clone();
                let id_color = id.clone();
                let id_hint = id.clone();
                let id_panlabel = id.clone();
                let id_fixed = id.clone();
                let id_bendinfo = id.clone();

                let fixed_pitch = {
                    let id = id.clone();
                    Signal::derive(move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id).map(|t| t.fixed_pitch).unwrap_or(false)))
                };
                let bend_count = move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_bendinfo).map(|t| t.pitch_bend.len()).unwrap_or(0));

                let note_count = move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_count).map(|t| t.notes.len()).unwrap_or(0));
                let color = move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_color).map(|t| t.color.clone()).unwrap_or_default());
                let has_expression = move || {
                    s.project.with(|p| {
                        p.tracks.iter().find(|t| t.id == id_hint).map(|t| !t.dynamics.expression.is_empty() || !t.dynamics.volume.is_empty()).unwrap_or(false)
                    })
                };
                let distinct_pitches = move || {
                    s.project.with(|p| {
                        p.tracks
                            .iter()
                            .find(|t| t.id == id_pitches)
                            .map(|t| {
                                let mut v: Vec<i32> = t.notes.iter().map(|n| n.pitch).collect();
                                v.sort_unstable();
                                v.dedup();
                                v
                            })
                            .unwrap_or_default()
                    })
                };

                view! {
                    <div class="panel__head">
                        <h2 class="panel__heading">"トラック設定"</h2>
                        <span class="pill" style:background=color>
                            {move || format!("{} ノート", note_count())}
                        </span>
                    </div>

                    <label class="field">
                        <span class="field__label">"名前"</span>
                        <input
                            class="input"
                            prop:value=move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_name).map(|t| t.name.clone()).unwrap_or_default())
                            on:input=move |ev| {
                                let v = event_target_value(&ev);
                                s.update_track(&id_name2, move |t| t.name = v);
                            }
                        />
                    </label>

                    <label class="field">
                        <span class="field__label">"既定の音声素材"</span>
                        <select
                            class="select"
                            prop:value=move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_default).and_then(|t| t.default_sample_id.clone()).unwrap_or_default())
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                let val = if v.is_empty() { None } else { Some(v) };
                                s.update_track(&id_default2, move |t| t.default_sample_id = val);
                            }
                        >
                            <option value="">"（素材なし）"</option>
                            {move || s.project.get().samples.iter().map(|sm| view! { <option value=sm.id.clone()>{sm.name.clone()}</option> }).collect_view()}
                        </select>
                    </label>

                    <div class="grid2">
                        {range_row("音量", tget!(|t| t.gain), 0.0, 4.0, 0.01, format_db, tupd!(|t, v| t.gain = v))}
                        {range_row("パン", tget!(|t| t.pan), -1.0, 1.0, 0.01, pan_display, tupd!(|t, v| t.pan = v))}
                    </div>

                    {range_row("リバーブ送り", tget!(|t| t.reverb_send), 0.0, 1.0, 0.01, |v| format!("{}%", (v * 100.0).round() as i64), tupd!(|t, v| t.reverb_send = v))}

                    <p class="hintline">
                        {move || if has_expression() {
                            "🎚 ベロシティ＋エクスプレッション(CC11)/ボリューム(CC7) を音量に反映します。"
                        } else {
                            "🎚 各ノートのベロシティを音量に反映します。"
                        }}
                    </p>

                    <h3 class="subheading">"ピッチ / ベンド"</h3>
                    <label class="checkline">
                        <input
                            type="checkbox"
                            prop:checked=move || fixed_pitch.get()
                            on:change=move |ev| {
                                let c = event_target_checked(&ev);
                                s.update_track(&id_fixed, move |t| t.fixed_pitch = c);
                            }
                        />
                        "音程を固定（ドラム/ワンショットキット保護）"
                    </label>
                    {range_row("ベンドレンジ", tget!(|t| t.bend_range), 0.0, 24.0, 1.0, |v| format!("±{} st", v as i64), tupd!(|t, v| t.bend_range = v))}
                    <p class="panel__muted small">
                        {move || {
                            let n = bend_count();
                            if n > 0 {
                                format!("MIDI ピッチベンド {n} 点をトラック全体に反映します。")
                            } else {
                                "MIDI のピッチベンドを取り込むとトラック全体に反映されます。".to_string()
                            }
                        }}
                    </p>

                    <h3 class="subheading">"ボイス（同時発音）管理"</h3>
                    <label class="field">
                        <span class="field__label">
                            "最大同時発音数 "
                            <em>{move || {
                                let n = s.project.with(|p| p.tracks.iter().find(|t| t.id == id_voices).map(|t| t.polyphony.max_voices).unwrap_or(0));
                                if n == 0 { "無制限".to_string() } else { format!("{n} 音") }
                            }}</em>
                        </span>
                        <input
                            class="input"
                            type="number"
                            min=0
                            max=64
                            step=1
                            prop:value=move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_voices2).map(|t| t.polyphony.max_voices).unwrap_or(0))
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<i32>() {
                                    let clamped = v.clamp(0, 64);
                                    s.update_track(&id_voices3, move |t| t.polyphony.max_voices = clamped);
                                }
                            }
                        />
                    </label>

                    <div class="grid2">
                        <label class="field">
                            <span class="field__label">"優先再生"</span>
                            <select
                                class="select"
                                prop:value=move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_prio).map(|t| t.polyphony.priority.as_value()).unwrap_or("newest"))
                                on:change=move |ev| {
                                    let p = VoicePriority::from_value(&event_target_value(&ev));
                                    s.update_track(&id_prio2, move |t| t.polyphony.priority = p);
                                }
                            >
                                {PRIORITY_OPTIONS.iter().map(|(v, l)| view! { <option value=*v>{*l}</option> }).collect_view()}
                            </select>
                        </label>
                        <label class="field">
                            <span class="field__label">"停止方法"</span>
                            <select
                                class="select"
                                prop:value=move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_stop).map(|t| t.polyphony.stop_mode.as_value()).unwrap_or("none"))
                                on:change=move |ev| {
                                    let m = StopMode::from_value(&event_target_value(&ev));
                                    s.update_track(&id_stop2, move |t| t.polyphony.stop_mode = m);
                                }
                            >
                                {STOP_OPTIONS.iter().map(|(v, l)| view! { <option value=*v>{*l}</option> }).collect_view()}
                            </select>
                        </label>
                    </div>

                    <h3 class="subheading">"ノート番号ごとの素材割り当て"</h3>
                    <p class="panel__muted small">
                        "特定の音だけ別素材に差し替えできます（ドラムキットや音域別の貼り替えに）。"
                    </p>
                    <div class="notemap">
                        {move || {
                            let pitches = distinct_pitches();
                            if pitches.is_empty() {
                                view! { <p class="panel__muted">"ノートがありません。"</p> }.into_any()
                            } else {
                                let id = id_panlabel.clone();
                                pitches
                                    .into_iter()
                                    .map(|pitch| {
                                        let id_row = id.clone();
                                        let id_assigned = id.clone();
                                        let assigned = Signal::derive(move || s.project.with(|p| p.tracks.iter().find(|t| t.id == id_assigned).and_then(|t| t.note_sample_map.get(&pitch.to_string()).cloned()).unwrap_or_default()));
                                        view! {
                                            <div class="notemap__row" class:notemap__row--override=move || !assigned.get().is_empty()>
                                                <span class="notemap__pitch">{midi_to_note_name(pitch as f64)}</span>
                                                <select
                                                    class="select select--mini"
                                                    prop:value=move || assigned.get()
                                                    on:change=move |ev| {
                                                        let v = event_target_value(&ev);
                                                        if v.is_empty() {
                                                            s.set_note_sample(&id_row, pitch, None);
                                                        } else {
                                                            s.set_note_sample(&id_row, pitch, Some(v.clone()));
                                                            s.selected_sample.set(Some(v));
                                                        }
                                                    }
                                                >
                                                    <option value="">"（既定）"</option>
                                                    {move || s.project.get().samples.iter().map(|sm| view! { <option value=sm.id.clone()>{sm.name.clone()}</option> }).collect_view()}
                                                </select>
                                            </div>
                                        }
                                    })
                                    .collect_view()
                                    .into_any()
                            }
                        }}
                    </div>
                }
                    .into_any()
            }}
        </section>
    }
}
