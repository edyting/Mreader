#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Mreader — smart visual pacing reader.

mod app;
mod config;
mod database;
mod document;
mod pacer;
mod reader;
mod statistics;
mod ui;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let code = app::run();
    std::process::exit(code.into());
}
