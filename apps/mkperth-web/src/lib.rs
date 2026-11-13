//! MK Perth in a browser: the whole app as WebAssembly, drawn on a canvas. Files are picked with
//! the browser's file picker and arrive a moment later; saving and exporting download the file.
//! The scheduler and everything else run in the page; nothing is sent anywhere.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use mkperth_ui_egui::{Date, PerthApp, Services};
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen::prelude::wasm_bindgen;

    struct App(PerthApp);

    impl eframe::App for App {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.0.ui(ui);
        }
    }

    /// The page's language: Portuguese for any `pt`, else English.
    fn language() -> mkperth_ui_egui::i18n::Lang {
        let locale = web_sys::window().and_then(|w| w.navigator().language()).unwrap_or_default();
        mkperth_ui_egui::i18n::Lang::from_locale(&locale.replace('-', "_"))
    }

    /// Hands the person `bytes` as the file `name`: a download.
    fn download(name: &str, bytes: &[u8]) {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/octet-stream");
        let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options) else { return };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
        if let Ok(a) = document.create_element("a").map(|e| e.unchecked_into::<web_sys::HtmlAnchorElement>()) {
            a.set_href(&url);
            a.set_download(name);
            a.click();
        }
        let _ = web_sys::Url::revoke_object_url(&url);
    }

    /// Where the activity table's width is kept, in the page's local storage.
    const TABLE_WIDTH: &str = "mkperth.table-width";

    /// The page's local storage, when the browser allows it.
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window().and_then(|w| w.local_storage().ok().flatten())
    }

    /// Runs the app on the page's `mkperth` canvas.
    #[allow(unsafe_code)] // `wasm_bindgen(start)` exports it with `#[unsafe(no_mangle)]`
    #[wasm_bindgen(start)]
    pub fn start() {
        wasm_bindgen_futures::spawn_local(run());
    }

    async fn run() {
        mkperth_ui_egui::i18n::set(language());
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let Some(canvas) = document.get_element_by_id("mkperth").and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else { return };
        // A file picked arrives here, and the app takes it on its next frame.
        let arrived: Rc<RefCell<Option<(String, String)>>> = Rc::default();
        let slot = arrived.clone();
        let services = Services {
            start_open: Some(Box::new(move || {
                let slot = slot.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let picked = rfd::AsyncFileDialog::new().add_filter("Projects (.json, .xml, .xer)", &["json", "xml", "xer"]).pick_file().await;
                    if let Some(file) = picked {
                        let bytes = file.read().await;
                        *slot.borrow_mut() = Some((file.file_name(), mkperth_ui_egui::file_text(&bytes)));
                    }
                });
            })),
            take_opened: Some(Box::new(move || arrived.borrow_mut().take())),
            download: Some(Box::new(download)),
            save_table_width: Some(Box::new(|width: Option<f32>| {
                if let Some(storage) = storage() {
                    let _ = match width {
                        Some(w) => storage.set_item(TABLE_WIDTH, &format!("{w:.0}")),
                        None => storage.remove_item(TABLE_WIDTH),
                    };
                }
            })),
            today: Some(Box::new(|| {
                let now = js_sys::Date::new_0();
                Date::from_ymd(now.get_full_year() as i32, now.get_month() + 1, now.get_date())
            })),
            ..Services::default()
        };
        let mut app = PerthApp::new(services);
        // The table's width as it was left in this browser.
        app.options.table_width = storage()
            .and_then(|s| s.get_item(TABLE_WIDTH).ok().flatten())
            .and_then(|text| text.trim().parse::<f32>().ok())
            .filter(|w| w.is_finite() && *w > 0.0);
        let runner = eframe::WebRunner::new();
        let _ = runner.start(canvas, eframe::WebOptions::default(), Box::new(|_cc| Ok(Box::new(App(app))))).await;
    }
}
