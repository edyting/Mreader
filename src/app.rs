//! Application bootstrap.

use gtk::prelude::*;
use gtk::{gio, Application};

use crate::ui::window::build_ui;

pub const APP_ID: &str = "com.mreader.Mreader";

pub fn run() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| {
        let _ = std::fs::create_dir_all(crate::config::AppConfig::config_dir());
        let _ = std::fs::create_dir_all(crate::config::AppConfig::data_dir());
    });

    app.connect_activate(build_ui);

    app.connect_open(|app, files, _hint| {
        for file in files {
            if let Some(path) = file.path() {
                unsafe {
                    std::env::set_var("MREADER_OPEN", path.to_string_lossy().as_ref());
                }
                break;
            }
        }
        build_ui(app);
    });

    app.set_flags(gio::ApplicationFlags::HANDLES_OPEN);

    let args: Vec<String> = std::env::args().collect();
    app.run_with_args(&args)
}
