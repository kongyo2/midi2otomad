mod api;
mod components;
mod enums;
mod format;
mod icons;
mod state;
mod widgets;

use leptos::prelude::*;
use midi2otomad_core::schema::{create_empty_project, DEFAULT_PROJECT_NAME};
use wasm_bindgen::prelude::*;

use components::{
    HelpPanel, OutputPanel, ReverbPanel, SampleInspector, SampleLibrary, Timeline, TopBar,
    TrackInspector,
};
use state::Studio;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

fn install_keyboard(studio: Studio) {
    let Some(win) = web_sys::window() else { return };
    let closure = Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
        let in_form_control = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .map(|el| {
                let tag = el.tag_name().to_uppercase();
                tag == "INPUT" || tag == "SELECT" || tag == "TEXTAREA"
            })
            .unwrap_or(false);
        let ctrl = ev.ctrl_key() || ev.meta_key();
        if ctrl {
            match ev.key().to_ascii_lowercase().as_str() {
                // テキスト入力中の Ctrl+Z/Y はブラウザ標準のテキスト Undo に任せる
                "z" | "y" if in_form_control => {}
                "z" => {
                    ev.prevent_default();
                    if ev.shift_key() {
                        studio.redo();
                    } else {
                        studio.undo();
                    }
                }
                "y" => {
                    ev.prevent_default();
                    studio.redo();
                }
                "s" => {
                    ev.prevent_default();
                    studio.save_project();
                }
                _ => {}
            }
            return;
        }
        if in_form_control {
            return;
        }
        if ev.code() == "Space" {
            ev.prevent_default();
            studio.toggle_play();
        }
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
    let _ = win.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
    closure.forget();
}

#[component]
fn App() -> impl IntoView {
    let studio = Studio::new(create_empty_project(DEFAULT_PROJECT_NAME));
    provide_context(studio);

    studio.start_status_polling();
    install_keyboard(studio);
    api::on_drag_drop(move |paths| {
        studio.drag_active.set(false);
        studio.ingest_dropped(paths);
    });
    api::on_window_event("tauri://drag-enter", move || studio.drag_active.set(true));
    api::on_window_event("tauri://drag-leave", move || studio.drag_active.set(false));

    view! {
        <div class="studio">
            <TopBar />
            <div class="studio__body">
                <aside class="studio__left">
                    <SampleLibrary />
                    <SampleInspector />
                </aside>
                <main class="studio__center">
                    <Timeline />
                </main>
                <aside class="studio__right">
                    <TrackInspector />
                    <ReverbPanel />
                    <OutputPanel />
                    <HelpPanel />
                </aside>
            </div>

            <Show when=move || studio.drag_active.get()>
                <div class="dropzone-overlay">
                    <div class="dropzone-overlay__card">
                        <div class="dropzone-overlay__icon">{icons::icon_import()}</div>
                        <p class="dropzone-overlay__title">"ここにドロップ"</p>
                        <p class="dropzone-overlay__sub">
                            ".mid → アレンジ読込 ／ wav・mp3 → 音声素材として追加"
                        </p>
                    </div>
                </div>
            </Show>

            <Show when=move || studio.busy.get().is_some()>
                <div class="busybar">
                    <span class="busybar__spinner"></span>
                    {move || studio.busy.get().unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || studio.toast.get().is_some()>
                <div class="toast">{move || studio.toast.get().unwrap_or_default()}</div>
            </Show>
        </div>
    }
}
