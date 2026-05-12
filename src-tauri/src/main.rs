#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let argv0 = std::env::args_os()
        .next()
        .map(|arg| arg.to_string_lossy().into_owned());
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    if should_run_cli(&args, argv0.as_deref(), launched_by_tauri_dev()) {
        std::process::exit(cronbox_lib::cli::run_from_env());
    }
    cronbox_lib::run()
}

fn should_run_cli(args: &[String], argv0: Option<&str>, launched_by_tauri_dev: bool) -> bool {
    if launched_by_tauri_dev {
        return false;
    }

    if args.iter().any(|arg| !arg.starts_with("-psn_")) {
        return true;
    }

    if args.iter().any(|arg| arg.starts_with("-psn_")) {
        return false;
    }

    !launched_from_app_bundle(argv0)
}

fn launched_from_app_bundle(argv0: Option<&str>) -> bool {
    argv0.is_some_and(|path| {
        std::path::Path::new(path)
            .components()
            .any(|component| component.as_os_str().to_string_lossy().ends_with(".app"))
    })
}

#[cfg(debug_assertions)]
fn launched_by_tauri_dev() -> bool {
    std::env::var_os("TAURI_ENV_TARGET_TRIPLE").is_some()
}

#[cfg(not(debug_assertions))]
fn launched_by_tauri_dev() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::should_run_cli;

    #[test]
    fn terminal_invocation_without_args_runs_cli_help() {
        assert!(should_run_cli(&[], Some("/usr/local/bin/cronbox"), false));
    }

    #[test]
    fn terminal_invocation_with_args_runs_cli() {
        assert!(should_run_cli(
            &["scripts".to_string(), "list".to_string()],
            Some("/usr/local/bin/cronbox"),
            false
        ));
    }

    #[test]
    fn macos_app_launch_runs_gui() {
        assert!(!should_run_cli(
            &["-psn_0_12345".to_string()],
            Some("/Applications/CronBox.app/Contents/MacOS/cronbox"),
            false
        ));
    }

    #[test]
    fn app_bundle_launch_without_psn_runs_gui() {
        assert!(!should_run_cli(
            &[],
            Some("/Applications/CronBox.app/Contents/MacOS/cronbox"),
            false
        ));
    }

    #[test]
    fn tauri_dev_launch_runs_gui() {
        assert!(!should_run_cli(&[], Some("target/debug/cronbox"), true));
    }
}
