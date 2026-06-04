use leptos::prelude::*;
use midi2otomad_core::music::midi_to_note_name;
use midi2otomad_core::schema::{
    Envelope, Filter, FilterType, InterpolationMode, LfoShape, PitchMod,
};

use crate::enums::SelectValue;
use crate::format::format_db;
use crate::state::{find_sample, Studio};
use crate::widgets::{range_row, Waveform};

const FILTER_OPTIONS: [(&str, &str); 8] = [
    ("lowpass", "ローパス"),
    ("highpass", "ハイパス"),
    ("bandpass", "バンドパス"),
    ("notch", "ノッチ"),
    ("peaking", "ピーキング"),
    ("lowshelf", "ローシェルフ"),
    ("highshelf", "ハイシェルフ"),
    ("allpass", "オールパス"),
];

const SHAPE_OPTIONS: [(&str, &str); 4] = [
    ("sine", "サイン"),
    ("triangle", "三角"),
    ("square", "矩形"),
    ("saw", "ノコギリ"),
];

fn ms(v: f64) -> String {
    format!("{} ms", v as i64)
}

#[component]
pub fn SampleInspector() -> impl IntoView {
    let s = expect_context::<Studio>();

    view! {
        <section class="panel">
            {move || {
                let Some(id) = s.selected_sample.get() else {
                    return view! {
                        <div>
                            <h2 class="panel__heading">"素材エディタ"</h2>
                            <p class="panel__muted">
                                "ライブラリから音声素材を選択すると、基準ピッチ・エンベロープ・ループを編集できます。"
                            </p>
                        </div>
                    }
                        .into_any();
                };
                let Some(sample0) = s.project.with_untracked(|p| find_sample(p, &id)) else {
                    return view! { <div></div> }.into_any();
                };
                let duration = if sample0.duration_sec > 0.0 { sample0.duration_sec } else { 1.0 };

                macro_rules! sget {
                    (|$x:ident| $body:expr) => {{
                        let id = id.clone();
                        Signal::derive(move || {
                            s.project.with(|p| p.samples.iter().find(|t| t.id == id).map(|$x| $body).unwrap_or(0.0))
                        })
                    }};
                }
                macro_rules! supd {
                    (|$x:ident, $v:ident| $body:expr) => {{
                        let id = id.clone();
                        move |$v: f64| s.update_sample(&id, move |$x| { $body; })
                    }};
                }

                let peaks_sig = {
                    let id = id.clone();
                    Signal::derive(move || s.peaks.get().get(&id).cloned().unwrap_or_default())
                };
                let loop_sig = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| {
                            p.samples.iter().find(|t| t.id == id).map(|x| {
                                let dur = if x.duration_sec > 0.0 { x.duration_sec } else { 1.0 };
                                let end = if x.loop_region.end_sec > x.loop_region.start_sec {
                                    x.loop_region.end_sec
                                } else {
                                    dur
                                };
                                (
                                    (x.loop_region.start_sec / dur).clamp(0.0, 1.0),
                                    (end / dur).clamp(0.0, 1.0),
                                    x.loop_region.enabled,
                                )
                            })
                        })
                    })
                };
                let loop_enabled = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| p.samples.iter().find(|t| t.id == id).map(|x| x.loop_region.enabled).unwrap_or(false))
                    })
                };
                let trim_sig = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| {
                            p.samples.iter().find(|t| t.id == id).map(|x| {
                                let dur = if x.duration_sec > 0.0 { x.duration_sec } else { 1.0 };
                                let end = if x.trim.end_sec > x.trim.start_sec {
                                    x.trim.end_sec
                                } else {
                                    dur
                                };
                                (
                                    (x.trim.start_sec / dur).clamp(0.0, 1.0),
                                    (end / dur).clamp(0.0, 1.0),
                                    x.trim.enabled,
                                )
                            })
                        })
                    })
                };
                let trim_enabled = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| p.samples.iter().find(|t| t.id == id).map(|x| x.trim.enabled).unwrap_or(false))
                    })
                };
                let filter_enabled = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| p.samples.iter().find(|t| t.id == id).map(|x| x.filter.enabled).unwrap_or(false))
                    })
                };
                let envelope_modified = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| p.samples.iter().find(|t| t.id == id).map(|x| x.envelope != Envelope::default()).unwrap_or(false))
                    })
                };
                let filter_modified = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| p.samples.iter().find(|t| t.id == id).map(|x| x.filter != Filter { enabled: x.filter.enabled, ..Filter::default() }).unwrap_or(false))
                    })
                };
                let pitch_modified = {
                    let id = id.clone();
                    Signal::derive(move || {
                        s.project.with(|p| p.samples.iter().find(|t| t.id == id).map(|x| x.pitch_mod != PitchMod::default()).unwrap_or(false))
                    })
                };

                let id_name = id.clone();
                let id_name2 = id.clone();
                let id_preview = id.clone();
                let id_detect = id.clone();
                let id_env_reset = id.clone();
                let id_filter_reset = id.clone();
                let id_pitch_reset = id.clone();
                let id_layer = id.clone();
                let id_loop_en = id.clone();
                let id_loop_all = id.clone();
                let id_trim_en = id.clone();
                let id_trim_all = id.clone();
                let id_interp = id.clone();
                let id_interp2 = id.clone();
                let id_ftype = id.clone();
                let id_ftype2 = id.clone();
                let id_fen = id.clone();
                let id_fshape = id.clone();
                let id_fshape2 = id.clone();
                let id_vshape = id.clone();
                let id_vshape2 = id.clone();

                view! {
                    <div class="panel__head">
                        <h2 class="panel__heading">"素材エディタ"</h2>
                        <button
                            class="btn btn--sm"
                            on:click=move |_| {
                                if let Some(sm) = s.project.with_untracked(|p| find_sample(p, &id_preview)) {
                                    s.preview_sample(sm);
                                }
                            }
                        >
                            "▶ 試聴"
                        </button>
                    </div>

                    <label class="field">
                        <span class="field__label">"名前"</span>
                        <input
                            class="input"
                            prop:value=move || {
                                s.project.with(|p| p.samples.iter().find(|t| t.id == id_name).map(|x| x.name.clone()).unwrap_or_default())
                            }
                            on:input=move |ev| {
                                let v = event_target_value(&ev);
                                s.update_sample(&id_name2, move |x| x.name = v);
                            }
                        />
                    </label>

                    <div class="loopeditor">
                        <div class="loopeditor__wave">
                            <Waveform
                                peaks=peaks_sig
                                loop_region=loop_sig
                                trim=trim_sig
                                color="#ffb27a".to_string()
                                height=96.0
                            />
                        </div>
                        <div class="loopeditor__head">
                            <label class="checkline">
                                <input
                                    type="checkbox"
                                    prop:checked=move || trim_enabled.get()
                                    on:change=move |ev| {
                                        let c = event_target_checked(&ev);
                                        s.update_sample(&id_trim_en, move |x| x.trim.enabled = c);
                                    }
                                />
                                "トリミング（不要部分をカット）"
                            </label>
                            <button
                                class="linkbtn"
                                on:click=move |_| {
                                    s.update_sample(&id_trim_all, move |x| {
                                        x.trim.start_sec = 0.0;
                                        x.trim.end_sec = duration;
                                    })
                                }
                            >
                                "全体"
                            </button>
                        </div>
                        <div class="grid2">
                            {range_row(
                                "トリム開始",
                                sget!(|x| x.trim.start_sec),
                                0.0,
                                duration,
                                duration / 1000.0,
                                |v| format!("{v:.3}s"),
                                supd!(|x, v| x.trim.start_sec = v),
                            )}
                            {range_row(
                                "トリム終了",
                                sget!(|x| if x.trim.end_sec > x.trim.start_sec { x.trim.end_sec } else { duration }),
                                0.0,
                                duration,
                                duration / 1000.0,
                                |v| format!("{v:.3}s"),
                                supd!(|x, v| x.trim.end_sec = v),
                            )}
                        </div>
                        <div class="loopeditor__head loopeditor__head--gap">
                            <label class="checkline">
                                <input
                                    type="checkbox"
                                    prop:checked=move || loop_enabled.get()
                                    on:change=move |ev| {
                                        let c = event_target_checked(&ev);
                                        s.update_sample(&id_loop_en, move |x| x.loop_region.enabled = c);
                                    }
                                />
                                "ループ（ロングトーン対応）"
                            </label>
                            <button
                                class="linkbtn"
                                on:click=move |_| {
                                    s.update_sample(&id_loop_all, move |x| {
                                        x.loop_region.start_sec = 0.0;
                                        x.loop_region.end_sec = duration;
                                    })
                                }
                            >
                                "全体"
                            </button>
                        </div>
                        <div class="grid2">
                            {range_row(
                                "ループ開始",
                                sget!(|x| x.loop_region.start_sec),
                                0.0,
                                duration,
                                duration / 1000.0,
                                |v| format!("{v:.3}s"),
                                supd!(|x, v| x.loop_region.start_sec = v),
                            )}
                            {range_row(
                                "ループ終了",
                                sget!(|x| if x.loop_region.end_sec > x.loop_region.start_sec { x.loop_region.end_sec } else { duration }),
                                0.0,
                                duration,
                                duration / 1000.0,
                                |v| format!("{v:.3}s"),
                                supd!(|x, v| x.loop_region.end_sec = v),
                            )}
                        </div>
                    </div>

                    <div class="panel__head">
                        <h3 class="subheading">"ピッチ"</h3>
                        <button
                            class="linkbtn"
                            title="波形から基準ピッチを自動検出"
                            on:click=move |_| s.detect_pitch(id_detect.clone())
                        >
                            "🎯 自動検出"
                        </button>
                    </div>
                    <div class="grid2">
                        {range_row(
                            "基準ピッチ",
                            sget!(|x| x.base_pitch as f64),
                            24.0,
                            96.0,
                            1.0,
                            midi_to_note_name,
                            supd!(|x, v| x.base_pitch = v as i32),
                        )}
                        {range_row(
                            "微調整",
                            sget!(|x| x.tune_cents),
                            -100.0,
                            100.0,
                            1.0,
                            |v| format!("{} cent", v as i64),
                            supd!(|x, v| x.tune_cents = v),
                        )}
                    </div>

                    <div class="grid2">
                        {range_row(
                            "ゲイン",
                            sget!(|x| x.gain),
                            0.0,
                            4.0,
                            0.01,
                            format_db,
                            supd!(|x, v| x.gain = v),
                        )}
                        <label class="field">
                            <span class="field__label">"補間方式"</span>
                            <select
                                class="select"
                                prop:value=move || {
                                    s.project.with(|p| p.samples.iter().find(|t| t.id == id_interp).map(|x| x.interpolation.as_value()).unwrap_or("hermite"))
                                }
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    let m = InterpolationMode::from_value(&v);
                                    s.update_sample(&id_interp2, move |x| x.interpolation = m);
                                }
                            >
                                <option value="hermite">"エルミート（高品質）"</option>
                                <option value="sinc">"sinc（最高品質）"</option>
                                <option value="linear">"リニア（軽量）"</option>
                            </select>
                        </label>
                    </div>

                    <div class="panel__head">
                        <h3 class="subheading">"エンベロープ (DAHDSR)"</h3>
                        <button
                            class="linkbtn"
                            title="エンベロープを初期値に戻す"
                            style:visibility=move || if envelope_modified.get() { "visible" } else { "hidden" }
                            on:click=move |_| s.update_sample(&id_env_reset, |x| x.envelope = Envelope::default())
                        >
                            "↺ リセット"
                        </button>
                    </div>
                    <div class="grid2">
                        {range_row("ディレイ", sget!(|x| x.envelope.delay_ms), 0.0, 2000.0, 1.0, ms, supd!(|x, v| x.envelope.delay_ms = v))}
                        {range_row("アタック", sget!(|x| x.envelope.attack_ms), 0.0, 2000.0, 1.0, ms, supd!(|x, v| x.envelope.attack_ms = v))}
                        {range_row("ホールド", sget!(|x| x.envelope.hold_ms), 0.0, 2000.0, 1.0, ms, supd!(|x, v| x.envelope.hold_ms = v))}
                        {range_row("ディケイ", sget!(|x| x.envelope.decay_ms), 0.0, 4000.0, 1.0, ms, supd!(|x, v| x.envelope.decay_ms = v))}
                        {range_row("サステイン", sget!(|x| x.envelope.sustain), 0.0, 1.0, 0.01, |v| format!("{}%", (v * 100.0).round() as i64), supd!(|x, v| x.envelope.sustain = v))}
                        {range_row("リリース", sget!(|x| x.envelope.release_ms), 0.0, 8000.0, 1.0, ms, supd!(|x, v| x.envelope.release_ms = v))}
                        {range_row("アタックカーブ", sget!(|x| x.envelope.attack_curve), -8.0, 8.0, 0.1, |v| format!("{v:.1}"), supd!(|x, v| x.envelope.attack_curve = v))}
                        {range_row("ディケイカーブ", sget!(|x| x.envelope.decay_curve), -8.0, 8.0, 0.1, |v| format!("{v:.1}"), supd!(|x, v| x.envelope.decay_curve = v))}
                        {range_row("リリースカーブ", sget!(|x| x.envelope.release_curve), -8.0, 8.0, 0.1, |v| format!("{v:.1}"), supd!(|x, v| x.envelope.release_curve = v))}
                    </div>

                    <div class="panel__head">
                        <h3 class="subheading">"音色フィルター"</h3>
                        <button
                            class="linkbtn"
                            title="フィルターのパラメータを初期値に戻す（オン/オフは保持）"
                            style:visibility=move || if filter_modified.get() { "visible" } else { "hidden" }
                            on:click=move |_| s.update_sample(&id_filter_reset, |x| {
                                x.filter = Filter { enabled: x.filter.enabled, ..Filter::default() };
                            })
                        >
                            "↺ リセット"
                        </button>
                    </div>
                    <div class="grid2">
                        <label class="checkline">
                            <input
                                type="checkbox"
                                prop:checked=move || filter_enabled.get()
                                on:change=move |ev| {
                                    let c = event_target_checked(&ev);
                                    s.update_sample(&id_fen, move |x| x.filter.enabled = c);
                                }
                            />
                            "有効"
                        </label>
                        <label class="field">
                            <span class="field__label">"タイプ"</span>
                            <select
                                class="select"
                                prop:value=move || {
                                    s.project.with(|p| p.samples.iter().find(|t| t.id == id_ftype).map(|x| x.filter.kind.as_value()).unwrap_or("lowpass"))
                                }
                                on:change=move |ev| {
                                    let k = FilterType::from_value(&event_target_value(&ev));
                                    s.update_sample(&id_ftype2, move |x| x.filter.kind = k);
                                }
                            >
                                {FILTER_OPTIONS.iter().map(|(v, l)| view! { <option value=*v>{*l}</option> }).collect_view()}
                            </select>
                        </label>
                        {range_row("カットオフ", sget!(|x| x.filter.cutoff_hz), 20.0, 20000.0, 1.0, |v| format!("{} Hz", v as i64), supd!(|x, v| x.filter.cutoff_hz = v))}
                        {range_row("レゾナンス", sget!(|x| x.filter.q), 0.1, 24.0, 0.1, |v| format!("Q {v:.2}"), supd!(|x, v| x.filter.q = v))}
                        {range_row("フィルターゲイン", sget!(|x| x.filter.gain_db), -24.0, 24.0, 0.5, |v| format!("{v:.1} dB"), supd!(|x, v| x.filter.gain_db = v))}
                        {range_row("フィルターEG", sget!(|x| x.filter.env_amount), -8.0, 8.0, 0.1, |v| format!("{v:.1} oct"), supd!(|x, v| x.filter.env_amount = v))}
                        {range_row("フィルターLFO深さ", sget!(|x| x.filter.lfo_depth), 0.0, 8.0, 0.1, |v| format!("{v:.1} oct"), supd!(|x, v| x.filter.lfo_depth = v))}
                        {range_row("フィルターLFO速度", sget!(|x| x.filter.lfo_hz), 0.0, 16.0, 0.1, |v| format!("{v:.1} Hz"), supd!(|x, v| x.filter.lfo_hz = v))}
                        <label class="field">
                            <span class="field__label">"LFO波形"</span>
                            <select
                                class="select"
                                prop:value=move || {
                                    s.project.with(|p| p.samples.iter().find(|t| t.id == id_fshape).map(|x| x.filter.lfo_shape.as_value()).unwrap_or("sine"))
                                }
                                on:change=move |ev| {
                                    let sh = LfoShape::from_value(&event_target_value(&ev));
                                    s.update_sample(&id_fshape2, move |x| x.filter.lfo_shape = sh);
                                }
                            >
                                {SHAPE_OPTIONS.iter().map(|(v, l)| view! { <option value=*v>{*l}</option> }).collect_view()}
                            </select>
                        </label>
                    </div>

                    <div class="panel__head">
                        <h3 class="subheading">"ダイナミックピッチ"</h3>
                        <button
                            class="linkbtn"
                            title="ダイナミックピッチを初期値に戻す"
                            style:visibility=move || if pitch_modified.get() { "visible" } else { "hidden" }
                            on:click=move |_| s.update_sample(&id_pitch_reset, |x| x.pitch_mod = PitchMod::default())
                        >
                            "↺ リセット"
                        </button>
                    </div>
                    <div class="grid2">
                        {range_row("グライド量", sget!(|x| x.pitch_mod.glide_semitones), -36.0, 36.0, 1.0, |v| format!("{} st", v as i64), supd!(|x, v| x.pitch_mod.glide_semitones = v))}
                        {range_row("グライド時間", sget!(|x| x.pitch_mod.glide_ms), 0.0, 4000.0, 1.0, ms, supd!(|x, v| x.pitch_mod.glide_ms = v))}
                        {range_row("グライドカーブ", sget!(|x| x.pitch_mod.glide_curve), -8.0, 8.0, 0.1, |v| format!("{v:.1}"), supd!(|x, v| x.pitch_mod.glide_curve = v))}
                        {range_row("ビブラート深さ", sget!(|x| x.pitch_mod.vibrato_cents), 0.0, 600.0, 1.0, |v| format!("{} cent", v as i64), supd!(|x, v| x.pitch_mod.vibrato_cents = v))}
                        {range_row("ビブラート速度", sget!(|x| x.pitch_mod.vibrato_hz), 0.0, 16.0, 0.1, |v| format!("{v:.1} Hz"), supd!(|x, v| x.pitch_mod.vibrato_hz = v))}
                        {range_row("ビブラート遅延", sget!(|x| x.pitch_mod.vibrato_delay_ms), 0.0, 2000.0, 1.0, ms, supd!(|x, v| x.pitch_mod.vibrato_delay_ms = v))}
                        {range_row("ビブラートフェード", sget!(|x| x.pitch_mod.vibrato_fade_ms), 0.0, 2000.0, 1.0, ms, supd!(|x, v| x.pitch_mod.vibrato_fade_ms = v))}
                        <label class="field">
                            <span class="field__label">"波形"</span>
                            <select
                                class="select"
                                prop:value=move || {
                                    s.project.with(|p| p.samples.iter().find(|t| t.id == id_vshape).map(|x| x.pitch_mod.vibrato_shape.as_value()).unwrap_or("sine"))
                                }
                                on:change=move |ev| {
                                    let sh = LfoShape::from_value(&event_target_value(&ev));
                                    s.update_sample(&id_vshape2, move |x| x.pitch_mod.vibrato_shape = sh);
                                }
                            >
                                {SHAPE_OPTIONS.iter().map(|(v, l)| view! { <option value=*v>{*l}</option> }).collect_view()}
                            </select>
                        </label>
                    </div>

                    <h3 class="subheading">"レイヤー（重ねて鳴らす素材）"</h3>
                    {move || {
                        let self_id = id_layer.clone();
                        let others: Vec<(String, String)> = s.project.with(|p| {
                            p.samples
                                .iter()
                                .filter(|x| x.id != self_id)
                                .map(|x| (x.id.clone(), x.name.clone()))
                                .collect()
                        });
                        if others.is_empty() {
                            return view! {
                                <p class="panel__muted small">
                                    "他の素材を追加すると、1 つのノートで重ねて発音できます。"
                                </p>
                            }
                                .into_any();
                        }
                        let edit_id = id_layer.clone();
                        view! {
                            <div class="grid2">
                                {others
                                    .into_iter()
                                    .map(move |(oid, oname)| {
                                        let target = edit_id.clone();
                                        let target2 = edit_id.clone();
                                        let oid_check = oid.clone();
                                        let checked = Signal::derive(move || {
                                            s.project
                                                .with(|p| {
                                                    p.samples
                                                        .iter()
                                                        .find(|t| t.id == target)
                                                        .map(|x| x.link_ids.contains(&oid_check))
                                                        .unwrap_or(false)
                                                })
                                        });
                                        view! {
                                            <label class="checkline">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=move || checked.get()
                                                    on:change=move |ev| {
                                                        let on = event_target_checked(&ev);
                                                        let oid = oid.clone();
                                                        s.update_sample(
                                                            &target2,
                                                            move |x| {
                                                                if on {
                                                                    if !x.link_ids.contains(&oid) {
                                                                        x.link_ids.push(oid);
                                                                    }
                                                                } else {
                                                                    x.link_ids.retain(|l| l != &oid);
                                                                }
                                                            },
                                                        );
                                                    }
                                                />
                                                <span>{oname}</span>
                                            </label>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }}
                }
                    .into_any()
            }}
        </section>
    }
}
