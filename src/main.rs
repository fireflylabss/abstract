#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod io;
mod ui;

fn main() {
    ui::app::run();
}
