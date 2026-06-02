use leptos::prelude::*;
use midi2otomad_core::schema::Track;
use wasm_bindgen::JsCast;

use crate::format::format_time;
use crate::state::{project_duration, Studio};
use crate::widgets::context_2d;

const HEADER_WIDTH: f64 = 200.0;
const ROW_HEIGHT: f64 = 96.0;
const MAX_CANVAS_WIDTH: f64 = 30000.0;

fn lane_seek(ev: &web_sys::MouseEvent, px_per_sec: f64, s: Studio) {
    if let Some(target) = ev.current_target() {
        if let Ok(el) = target.dyn_into::<web_sys::Element>() {
            let rect = el.get_bounding_client_rect();
            let x = ev.client_x() as f64 - rect.left();
            s.seek((x / px_per_sec).max(0.0));
        }
    }
}

fn draw_piano_roll(
    ctx: &web_sys::CanvasRenderingContext2d,
    track: &Track,
    px_per_sec: f64,
    canvas_width: f64,
) {
    ctx.clear_rect(0.0, 0.0, canvas_width, ROW_HEIGHT);
    if track.notes.is_empty() {
        return;
    }
    let mut min_p = 127i32;
    let mut max_p = 0i32;
    for n in &track.notes {
        min_p = min_p.min(n.pitch);
        max_p = max_p.max(n.pitch);
    }
    min_p -= 1;
    max_p += 1;
    let range = (max_p - min_p + 1).max(1) as f64;
    let note_h = ROW_HEIGHT / range;

    ctx.set_fill_style_str("rgba(255,255,255,0.04)");
    for p in min_p..=max_p {
        if p % 12 == 0 {
            let y = (max_p - p) as f64 * note_h;
            ctx.fill_rect(0.0, y, canvas_width, note_h);
        }
    }

    for n in &track.notes {
        let x = n.start_sec * px_per_sec;
        if x > canvas_width {
            continue;
        }
        let w = (n.duration_sec * px_per_sec).max(2.0);
        let y = (max_p - n.pitch) as f64 * note_h;
        let h = (note_h - 1.0).max(2.0);
        let overridden = track.note_sample_map.contains_key(&n.pitch.to_string());
        let alpha = 0.4 + 0.6 * (n.velocity as f64 / 127.0);
        if overridden {
            ctx.set_fill_style_str("#ffd34d");
        } else {
            ctx.set_fill_style_str(&track.color);
        }
        ctx.set_global_alpha(alpha);
        ctx.fill_rect(x, y, w, h);
    }
    ctx.set_global_alpha(1.0);
}

#[component]
fn TrackRow(
    track_id: String,
    #[prop(into)] px_per_sec: Signal<f64>,
    #[prop(into)] canvas_width: Signal<f64>,
) -> impl IntoView {
    let s = expect_context::<Studio>();
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    let tid = track_id.clone();
    let track_for = move || {
        s.project
            .with(|p| p.tracks.iter().find(|t| t.id == tid).cloned())
    };

    {
        let track_for = track_for.clone();
        Effect::new(move |_| {
            let width = canvas_width.get();
            let px = px_per_sec.get();
            let Some(track) = track_for() else { return };
            let Some(canvas) = canvas_ref.get() else {
                return;
            };
            canvas.set_width(width as u32);
            canvas.set_height(ROW_HEIGHT as u32);
            if let Some(ctx) = context_2d(&canvas) {
                draw_piano_roll(&ctx, &track, px, width);
            }
        });
    }

    let id_select = track_id.clone();
    let id_mute = track_id.clone();
    let id_solo = track_id.clone();
    let id_sample = track_id.clone();
    let tid_name = track_id.clone();
    let tid_color = track_id.clone();
    let tid_muted = track_id.clone();
    let tid_solo = track_id.clone();
    let tid_sel = track_id.clone();
    let tid_dim = track_id.clone();
    let tid_sampleval = track_id.clone();

    let name = move || {
        s.project.with(|p| {
            p.tracks
                .iter()
                .find(|t| t.id == tid_name)
                .map(|t| t.name.clone())
                .unwrap_or_default()
        })
    };
    let color = move || {
        s.project.with(|p| {
            p.tracks
                .iter()
                .find(|t| t.id == tid_color)
                .map(|t| t.color.clone())
                .unwrap_or_default()
        })
    };
    let muted = move || {
        s.project.with(|p| {
            p.tracks
                .iter()
                .find(|t| t.id == tid_muted)
                .map(|t| t.muted)
                .unwrap_or(false)
        })
    };
    let soloed = move || {
        s.project.with(|p| {
            p.tracks
                .iter()
                .find(|t| t.id == tid_solo)
                .map(|t| t.solo)
                .unwrap_or(false)
        })
    };
    let selected = move || s.selected_track.get().as_deref() == Some(tid_sel.as_str());
    let dimmed = move || {
        s.project.with(|p| {
            let some_solo = p.tracks.iter().any(|t| t.solo);
            p.tracks
                .iter()
                .find(|t| t.id == tid_dim)
                .map(|t| t.muted || (some_solo && !t.solo))
                .unwrap_or(false)
        })
    };
    let sample_value = move || {
        s.project.with(|p| {
            p.tracks
                .iter()
                .find(|t| t.id == tid_sampleval)
                .and_then(|t| t.default_sample_id.clone())
                .unwrap_or_default()
        })
    };

    view! {
        <div class="trackrow" class:trackrow--selected=selected style:height=format!("{ROW_HEIGHT}px")>
            <div class="trackrow__header" style:width=format!("{HEADER_WIDTH}px")>
                <button
                    class="trackrow__name"
                    title="トラックを選択"
                    on:click=move |_| s.selected_track.set(Some(id_select.clone()))
                >
                    <span class="trackrow__swatch" style:background=color></span>
                    <span class="trackrow__label">{name}</span>
                </button>
                <div class="trackrow__controls">
                    <button
                        class="tag"
                        class:tag--on=muted
                        on:click=move |_| {
                            let cur = s.project.with_untracked(|p| p.tracks.iter().find(|t| t.id == id_mute).map(|t| t.muted).unwrap_or(false));
                            s.update_track(&id_mute, move |t| t.muted = !cur);
                        }
                    >
                        "M"
                    </button>
                    <button
                        class="tag"
                        class:tag--solo=soloed
                        on:click=move |_| {
                            let cur = s.project.with_untracked(|p| p.tracks.iter().find(|t| t.id == id_solo).map(|t| t.solo).unwrap_or(false));
                            s.update_track(&id_solo, move |t| t.solo = !cur);
                        }
                    >
                        "S"
                    </button>
                    <select
                        class="select select--mini"
                        prop:value=sample_value
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            let val = if v.is_empty() { None } else { Some(v) };
                            s.update_track(&id_sample, move |t| t.default_sample_id = val);
                        }
                    >
                        <option value="">"（素材なし）"</option>
                        {move || {
                            s.project.get().samples.iter().map(|sm| view! { <option value=sm.id.clone()>{sm.name.clone()}</option> }).collect_view()
                        }}
                    </select>
                </div>
            </div>
            <div
                class="trackrow__lane"
                class:trackrow__lane--dim=dimmed
                on:click=move |ev| lane_seek(&ev, px_per_sec.get_untracked(), s)
            >
                <canvas node_ref=canvas_ref></canvas>
            </div>
        </div>
    }
}

#[component]
pub fn Timeline() -> impl IntoView {
    let s = expect_context::<Studio>();
    let px_requested = RwSignal::new(80.0_f64);
    let duration = Memo::new(move |_| project_duration(&s.project.get()).max(8.0));
    let px_per_sec = Memo::new(move |_| {
        px_requested
            .get()
            .min(MAX_CANVAS_WIDTH / duration.get().max(1.0))
    });
    let canvas_width =
        Memo::new(move |_| MAX_CANVAS_WIDTH.min((duration.get() * px_per_sec.get()).ceil()));
    let content_width = move || HEADER_WIDTH + canvas_width.get();
    let playhead_x = move || HEADER_WIDTH + s.status.get().position * px_per_sec.get();

    view! {
        <section class="timeline-panel">
            <div class="timeline-toolbar">
                <span class="timeline-toolbar__title">"タイムライン / ピアノロール"</span>
                <div class="timeline-toolbar__zoom">
                    <button class="iconbtn" on:click=move |_| px_requested.update(|v| *v = (*v - 16.0).max(24.0))>
                        "－"
                    </button>
                    <span class="zoomlabel">{move || format!("{}px/s", px_per_sec.get().round() as i64)}</span>
                    <button class="iconbtn" on:click=move |_| px_requested.update(|v| *v = (*v + 16.0).min(200.0))>
                        "＋"
                    </button>
                </div>
            </div>

            <div class="timeline">
                <div class="timeline__content" style:width=move || format!("{}px", content_width())>
                    <div class="timeline__rulerrow">
                        <div class="timeline__corner" style:width=format!("{HEADER_WIDTH}px")>
                            {move || format_time(s.status.get().position)}
                        </div>
                        <div
                            class="ruler"
                            style:width=move || format!("{}px", canvas_width.get())
                            on:click=move |ev| lane_seek(&ev, px_per_sec.get_untracked(), s)
                        >
                            {move || {
                                let px = px_per_sec.get();
                                let dur = duration.get() as i64;
                                let step = if px < 30.0 { 5 } else if px < 80.0 { 2 } else { 1 };
                                (0..=dur)
                                    .step_by(step)
                                    .map(|t| {
                                        view! {
                                            <span class="ruler__tick" style:left=format!("{}px", t as f64 * px)>
                                                {format!("{t}s")}
                                            </span>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </div>
                    </div>

                    {move || {
                        let tracks = s.project.get().tracks;
                        if tracks.is_empty() {
                            view! {
                                <div class="timeline__empty">
                                    <p>
                                        <strong>".mid"</strong>
                                        " ファイルをドラッグ＆ドロップして始めましょう。"
                                    </p>
                                    <p class="panel__muted">
                                        "トラック・ノート・テンポを解析し、ここにピアノロールを表示します。"
                                    </p>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! {
                                <For
                                    each=move || s.project.get().tracks
                                    key=|t| t.id.clone()
                                    let:track
                                >
                                    <TrackRow
                                        track_id=track.id.clone()
                                        px_per_sec=px_per_sec
                                        canvas_width=canvas_width
                                    />
                                </For>
                                <div class="playhead" style:left=move || format!("{}px", playhead_x())></div>
                            }
                                .into_any()
                        }
                    }}
                </div>
            </div>
        </section>
    }
}
