mod api;
mod components;
mod enums;
mod format;
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
        if let Some(target) = ev.target() {
            if let Ok(el) = target.dyn_into::<web_sys::Element>() {
                let tag = el.tag_name().to_uppercase();
                if tag == "INPUT" || tag == "SELECT" || tag == "TEXTAREA" {
                    return;
                }
            }
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
                        <div class="dropzone-overlay__icon">"🎼"</div>
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
