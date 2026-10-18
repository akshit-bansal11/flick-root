//! Desktop shell entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    desktop_shell_lib::run();
}
