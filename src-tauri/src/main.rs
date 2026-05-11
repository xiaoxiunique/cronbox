#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let has_cli_args = std::env::args_os()
        .skip(1)
        .any(|arg| !arg.to_string_lossy().starts_with("-psn_"));

    if has_cli_args {
        std::process::exit(cronbox_lib::cli::run_from_env());
    }
    cronbox_lib::run()
}
