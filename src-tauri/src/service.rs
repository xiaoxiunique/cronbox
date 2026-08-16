use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

#[cfg(target_os = "macos")]
const LABEL: &str = "com.cronbox.service";
#[cfg(target_os = "linux")]
const SYSTEMD_UNIT: &str = "cronbox.service";

pub fn run(args: &[String]) -> Result<i32, String> {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = args;
        return Err("cronbox service is currently supported on macOS and Linux only".to_string());
    }

    #[cfg(target_os = "macos")]
    return run_macos(args);

    #[cfg(target_os = "linux")]
    run_linux(args)
}

/// Access token picked up from the environment at `service install` time and
/// baked into the generated unit/plist so the service keeps serving behind
/// auth even though it never sees the user's shell environment.
fn configured_auth_token() -> Option<String> {
    std::env::var("CRONBOX_AUTH_TOKEN")
        .ok()
        .filter(|token| !token.is_empty())
}

#[cfg(target_os = "macos")]
fn run_macos(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        Some("install") => install(),
        Some("status") | None => status(),
        Some("start") => start(),
        Some("stop") => stop(),
        Some("restart") => restart(),
        Some("uninstall") => uninstall(),
        Some("logs") => logs(&args[1..]),
        Some(other) => Err(format!(
            "unknown service command: {other}\n\nUsage: cronbox service <install|status|start|stop|restart|logs|uninstall>"
        )),
    }
}

#[cfg(target_os = "linux")]
fn run_linux(args: &[String]) -> Result<i32, String> {
    match args.first().map(String::as_str) {
        Some("install") => install(),
        Some("status") | None => status(),
        Some("start") => start(),
        Some("stop") => stop(),
        Some("restart") => restart(),
        Some("uninstall") => uninstall(),
        Some("logs") => logs(&args[1..]),
        Some(other) => Err(format!(
            "unknown service command: {other}\n\nUsage: cronbox service <install|status|start|stop|restart|logs|uninstall>"
        )),
    }
}

#[cfg(target_os = "macos")]
fn install() -> Result<i32, String> {
    let executable = std::env::current_exe()
        .map_err(|e| format!("cannot resolve current executable: {e}"))?
        .canonicalize()
        .map_err(|e| format!("cannot resolve executable path: {e}"))?;
    let auth_token = configured_auth_token();
    let paths = ServicePaths::resolve()?;
    fs::create_dir_all(&paths.logs_dir)
        .map_err(|e| format!("cannot create {}: {e}", paths.logs_dir.display()))?;
    if let Some(parent) = paths.plist.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    let plist = render_plist(&executable, &paths, auth_token.as_deref());
    let temporary = paths.plist.with_extension("plist.tmp");
    fs::write(&temporary, plist)
        .map_err(|e| format!("cannot write {}: {e}", temporary.display()))?;
    fs::rename(&temporary, &paths.plist)
        .map_err(|e| format!("cannot install {}: {e}", paths.plist.display()))?;

    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&paths.plist, fs::Permissions::from_mode(0o644))
        .map_err(|e| format!("cannot set {} permissions: {e}", paths.plist.display()))?;

    let domain = launch_domain()?;
    let target = launch_target(&domain);
    if is_loaded(&target) {
        let _ = run_launchctl(&["bootout", &target]);
    }
    checked_launchctl_with_retry(&["bootstrap", &domain, &paths.plist.to_string_lossy()])?;
    checked_launchctl(&["enable", &target])?;

    println!("CronBox service installed and started");
    println!("  executable: {}", executable.display());
    println!("  plist:      {}", paths.plist.display());
    if auth_token.is_some() {
        println!("  auth:       enabled (CRONBOX_AUTH_TOKEN set)");
    }
    println!("  web:        http://127.0.0.1:4317");
    Ok(0)
}

#[cfg(target_os = "macos")]
fn status() -> Result<i32, String> {
    let paths = ServicePaths::resolve()?;
    let domain = launch_domain()?;
    let target = launch_target(&domain);

    if !paths.plist.exists() {
        println!("CronBox service is not installed");
        return Ok(0);
    }

    let output = run_launchctl(&["print", &target])?;
    if !output.status.success() {
        println!("CronBox service is installed but stopped");
        println!("  plist: {}", paths.plist.display());
        return Ok(0);
    }

    let details = String::from_utf8_lossy(&output.stdout);
    let state = launchctl_value(&details, "state").unwrap_or("unknown");
    let pid = launchctl_value(&details, "pid");
    println!("CronBox service is {state}");
    if let Some(pid) = pid {
        println!("  pid:  {pid}");
    }
    println!("  web:  http://127.0.0.1:4317");
    println!("  logs: {}", paths.stdout.display());
    Ok(0)
}

#[cfg(target_os = "macos")]
fn start() -> Result<i32, String> {
    let paths = ServicePaths::resolve()?;
    if !paths.plist.exists() {
        return Err("service is not installed; run `cronbox service install` first".to_string());
    }
    let domain = launch_domain()?;
    let target = launch_target(&domain);
    if is_loaded(&target) {
        println!("CronBox service is already running");
        return Ok(0);
    }
    checked_launchctl_with_retry(&["bootstrap", &domain, &paths.plist.to_string_lossy()])?;
    println!("CronBox service started");
    Ok(0)
}

#[cfg(target_os = "macos")]
fn stop() -> Result<i32, String> {
    let domain = launch_domain()?;
    let target = launch_target(&domain);
    if !is_loaded(&target) {
        println!("CronBox service is already stopped");
        return Ok(0);
    }
    checked_launchctl(&["bootout", &target])?;
    println!("CronBox service stopped");
    Ok(0)
}

#[cfg(target_os = "macos")]
fn restart() -> Result<i32, String> {
    let paths = ServicePaths::resolve()?;
    if !paths.plist.exists() {
        return Err("service is not installed; run `cronbox service install` first".to_string());
    }
    let domain = launch_domain()?;
    let target = launch_target(&domain);
    if is_loaded(&target) {
        checked_launchctl(&["kickstart", "-k", &target])?;
    } else {
        checked_launchctl_with_retry(&["bootstrap", &domain, &paths.plist.to_string_lossy()])?;
    }
    println!("CronBox service restarted");
    Ok(0)
}

#[cfg(target_os = "macos")]
fn uninstall() -> Result<i32, String> {
    let paths = ServicePaths::resolve()?;
    let domain = launch_domain()?;
    let target = launch_target(&domain);
    if is_loaded(&target) {
        checked_launchctl(&["bootout", &target])?;
    }
    if paths.plist.exists() {
        fs::remove_file(&paths.plist)
            .map_err(|e| format!("cannot remove {}: {e}", paths.plist.display()))?;
    }
    println!("CronBox service uninstalled");
    println!("  logs were kept at {}", paths.logs_dir.display());
    Ok(0)
}

#[cfg(target_os = "macos")]
fn logs(args: &[String]) -> Result<i32, String> {
    let follow = match args {
        [] => false,
        [arg] if arg == "--follow" || arg == "-f" => true,
        _ => return Err("usage: cronbox service logs [--follow]".to_string()),
    };
    let paths = ServicePaths::resolve()?;
    if follow {
        let status = Command::new("tail")
            .args(["-n", "100", "-f"])
            .arg(&paths.stdout)
            .arg(&paths.stderr)
            .status()
            .map_err(|e| format!("cannot follow service logs: {e}"))?;
        return Ok(status.code().unwrap_or(1));
    }

    print_log("stdout", &paths.stdout)?;
    print_log("stderr", &paths.stderr)?;
    Ok(0)
}

#[cfg(target_os = "macos")]
fn print_log(label: &str, path: &Path) -> Result<(), String> {
    println!("==> {label}: {} <==", path.display());
    match fs::read_to_string(path) {
        Ok(contents) => {
            let lines: Vec<&str> = contents.lines().collect();
            for line in lines.iter().skip(lines.len().saturating_sub(100)) {
                println!("{line}");
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => println!("(no log yet)"),
        Err(err) => return Err(format!("cannot read {}: {err}", path.display())),
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install() -> Result<i32, String> {
    let executable = std::env::current_exe()
        .map_err(|e| format!("cannot resolve current executable: {e}"))?
        .canonicalize()
        .map_err(|e| format!("cannot resolve executable path: {e}"))?;
    let unit_path = systemd_unit_path()?;
    if let Some(parent) = unit_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    let unit = render_systemd_unit(&executable)?;
    let temporary = unit_path.with_extension("service.tmp");
    fs::write(&temporary, unit)
        .map_err(|e| format!("cannot write {}: {e}", temporary.display()))?;
    fs::rename(&temporary, &unit_path)
        .map_err(|e| format!("cannot install {}: {e}", unit_path.display()))?;

    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&unit_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| format!("cannot set {} permissions: {e}", unit_path.display()))?;

    checked_systemctl(&["daemon-reload"])?;
    checked_systemctl(&["enable", "--now", SYSTEMD_UNIT])?;

    println!("CronBox service installed and started");
    println!("  executable: {}", executable.display());
    println!("  unit:       {}", unit_path.display());
    println!("  web:        http://127.0.0.1:4317");
    Ok(0)
}

#[cfg(target_os = "linux")]
fn status() -> Result<i32, String> {
    let unit_path = systemd_unit_path()?;
    if !unit_path.exists() {
        println!("CronBox service is not installed");
        return Ok(0);
    }

    let output = run_systemctl(&["is-active", SYSTEMD_UNIT])?;
    let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
    println!(
        "CronBox service is {}",
        if state.is_empty() { "unknown" } else { &state }
    );
    if output.status.success() {
        let pid = run_systemctl(&["show", SYSTEMD_UNIT, "--property", "MainPID", "--value"])?;
        let pid = String::from_utf8_lossy(&pid.stdout).trim().to_string();
        if !pid.is_empty() && pid != "0" {
            println!("  pid:  {pid}");
        }
        println!("  web:  http://127.0.0.1:4317");
    }
    println!("  logs: journalctl --user -u {SYSTEMD_UNIT}");
    Ok(0)
}

#[cfg(target_os = "linux")]
fn start() -> Result<i32, String> {
    require_systemd_unit()?;
    checked_systemctl(&["start", SYSTEMD_UNIT])?;
    println!("CronBox service started");
    Ok(0)
}

#[cfg(target_os = "linux")]
fn stop() -> Result<i32, String> {
    require_systemd_unit()?;
    checked_systemctl(&["stop", SYSTEMD_UNIT])?;
    println!("CronBox service stopped");
    Ok(0)
}

#[cfg(target_os = "linux")]
fn restart() -> Result<i32, String> {
    require_systemd_unit()?;
    checked_systemctl(&["restart", SYSTEMD_UNIT])?;
    println!("CronBox service restarted");
    Ok(0)
}

#[cfg(target_os = "linux")]
fn uninstall() -> Result<i32, String> {
    let unit_path = systemd_unit_path()?;
    if unit_path.exists() {
        let _ = run_systemctl(&["disable", "--now", SYSTEMD_UNIT]);
        fs::remove_file(&unit_path)
            .map_err(|e| format!("cannot remove {}: {e}", unit_path.display()))?;
        checked_systemctl(&["daemon-reload"])?;
        let _ = run_systemctl(&["reset-failed", SYSTEMD_UNIT]);
    }
    println!("CronBox service uninstalled");
    println!("  journal logs were kept by systemd");
    Ok(0)
}

#[cfg(target_os = "linux")]
fn logs(args: &[String]) -> Result<i32, String> {
    let follow = match args {
        [] => false,
        [arg] if arg == "--follow" || arg == "-f" => true,
        _ => return Err("usage: cronbox service logs [--follow]".to_string()),
    };
    let mut command = Command::new("journalctl");
    command.args([
        "--user",
        "--unit",
        SYSTEMD_UNIT,
        "--lines",
        "100",
        "--no-pager",
    ]);
    if follow {
        command.arg("--follow");
    }
    let status = command
        .status()
        .map_err(|e| format!("cannot read systemd journal: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(target_os = "linux")]
fn systemd_unit_path() -> Result<PathBuf, String> {
    let config_dir = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => dirs_next::home_dir()
            .ok_or("cannot resolve home directory")?
            .join(".config"),
    };
    Ok(config_dir.join("systemd/user").join(SYSTEMD_UNIT))
}

#[cfg(target_os = "linux")]
fn require_systemd_unit() -> Result<PathBuf, String> {
    let path = systemd_unit_path()?;
    if path.exists() {
        Ok(path)
    } else {
        Err("service is not installed; run `cronbox service install` first".to_string())
    }
}

#[cfg(any(target_os = "linux", test))]
fn render_systemd_unit(executable: &Path, auth_token: Option<&str>) -> Result<String, String> {
    let executable = quote_systemd_argument(&executable.to_string_lossy())?;
    let environment = match auth_token {
        Some(token) => format!(
            "Environment=\"CRONBOX_AUTH_TOKEN={}\"\n",
            escape_systemd_env_value(token)
        ),
        None => String::new(),
    };
    Ok(format!(
        r#"[Unit]
Description=CronBox local scheduler and web console
After=network.target

[Service]
Type=simple
{environment}ExecStart={executable} serve --no-open
Restart=on-failure
RestartSec=5
KillMode=control-group
TimeoutStopSec=15

[Install]
WantedBy=default.target
"#
    ))
}

#[cfg(any(target_os = "linux", test))]
/// Escape a value for a double-quoted `Environment=` assignment in systemd
/// units: backslash, double quote, and dollar must be backslash-escaped.
fn escape_systemd_env_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
}

#[cfg(any(target_os = "linux", test))]
fn quote_systemd_argument(value: &str) -> Result<String, String> {
    if value.contains(['\n', '\r', '\0']) {
        return Err("executable path contains unsupported control characters".to_string());
    }
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    Ok(format!("\"{escaped}\""))
}

#[cfg(target_os = "linux")]
fn checked_systemctl(args: &[&str]) -> Result<Output, String> {
    let output = run_systemctl(args)?;
    if output.status.success() {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(format!(
        "systemctl --user {} failed: {}",
        args.join(" "),
        if stderr.is_empty() {
            "user systemd session is unavailable"
        } else {
            &stderr
        }
    ))
}

#[cfg(target_os = "linux")]
fn run_systemctl(args: &[&str]) -> Result<Output, String> {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run systemctl --user: {e}"))
}

#[cfg(target_os = "macos")]
struct ServicePaths {
    plist: PathBuf,
    logs_dir: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}

#[cfg(target_os = "macos")]
impl ServicePaths {
    fn resolve() -> Result<Self, String> {
        let home = dirs_next::home_dir().ok_or("cannot resolve home directory")?;
        let logs_dir = home.join("Library/Logs/CronBox");
        Ok(Self {
            plist: home.join(format!("Library/LaunchAgents/{LABEL}.plist")),
            stdout: logs_dir.join("service.log"),
            stderr: logs_dir.join("service.error.log"),
            logs_dir,
        })
    }
}

#[cfg(target_os = "macos")]
fn render_plist(executable: &Path, paths: &ServicePaths, auth_token: Option<&str>) -> String {
    let environment = match auth_token {
        Some(token) => format!(
            "  <key>EnvironmentVariables</key>\n  <dict>\n    <key>CRONBOX_AUTH_TOKEN</key>\n    <string>{token}</string>\n  </dict>\n",
            token = escape_xml(token)
        ),
        None => String::new(),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{executable}</string>
    <string>serve</string>
    <string>--no-open</string>
  </array>
{environment}  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
  <key>ThrottleInterval</key>
  <integer>5</integer>
  <key>StandardOutPath</key>
  <string>{stdout}</string>
  <key>StandardErrorPath</key>
  <string>{stderr}</string>
</dict>
</plist>
"#,
        label = LABEL,
        executable = escape_xml(&executable.to_string_lossy()),
        environment = environment,
        stdout = escape_xml(&paths.stdout.to_string_lossy()),
        stderr = escape_xml(&paths.stderr.to_string_lossy()),
    )
}

#[cfg(target_os = "macos")]
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "macos")]
fn launch_domain() -> Result<String, String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|e| format!("cannot determine user id: {e}"))?;
    if !output.status.success() {
        return Err("cannot determine user id".to_string());
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("gui/{uid}"))
}

#[cfg(target_os = "macos")]
fn launch_target(domain: &str) -> String {
    format!("{domain}/{LABEL}")
}

#[cfg(target_os = "macos")]
fn is_loaded(target: &str) -> bool {
    run_launchctl(&["print", target]).is_ok_and(|output| output.status.success())
}

#[cfg(target_os = "macos")]
fn checked_launchctl(args: &[&str]) -> Result<Output, String> {
    let output = run_launchctl(args)?;
    if output.status.success() {
        return Ok(output);
    }
    Err(launchctl_error(args, &output))
}

#[cfg(target_os = "macos")]
fn checked_launchctl_with_retry(args: &[&str]) -> Result<Output, String> {
    let mut last_output = None;
    for attempt in 0..10 {
        let output = run_launchctl(args)?;
        if output.status.success() {
            return Ok(output);
        }
        last_output = Some(output);
        if attempt < 9 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    Err(launchctl_error(
        args,
        last_output.as_ref().expect("retry loop always runs"),
    ))
}

#[cfg(target_os = "macos")]
fn launchctl_error(args: &[&str], output: &Output) -> String {
    let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
    format!(
        "launchctl {} failed: {}",
        args.join(" "),
        if error.is_empty() {
            "unknown error"
        } else {
            &error
        }
    )
}

#[cfg(target_os = "macos")]
fn run_launchctl(args: &[&str]) -> Result<Output, String> {
    Command::new("launchctl")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run launchctl: {e}"))
}

#[cfg(target_os = "macos")]
fn launchctl_value<'a>(details: &'a str, key: &str) -> Option<&'a str> {
    details.lines().find_map(|line| {
        let (candidate, value) = line.trim().split_once(" = ")?;
        (candidate == key).then_some(value)
    })
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn plist_runs_web_service_without_opening_browser() {
        let paths = ServicePaths {
            plist: PathBuf::from("/tmp/service.plist"),
            logs_dir: PathBuf::from("/tmp/logs"),
            stdout: PathBuf::from("/tmp/logs/out.log"),
            stderr: PathBuf::from("/tmp/logs/error.log"),
        };
        let plist = render_plist(Path::new("/tmp/Cron & Box/cronbox"), &paths, None);
        assert!(plist.contains("/tmp/Cron &amp; Box/cronbox"));
        assert!(plist.contains("<string>serve</string>"));
        assert!(plist.contains("<string>--no-open</string>"));
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(!plist.contains("EnvironmentVariables"));
    }

    #[test]
    fn plist_embeds_auth_token_when_configured() {
        let paths = ServicePaths {
            plist: PathBuf::from("/tmp/service.plist"),
            logs_dir: PathBuf::from("/tmp/logs"),
            stdout: PathBuf::from("/tmp/logs/out.log"),
            stderr: PathBuf::from("/tmp/logs/error.log"),
        };
        let plist = render_plist(
            Path::new("/tmp/cronbox"),
            &paths,
            Some("tok&en<1>\"quoted\""),
        );
        assert!(plist.contains("<key>EnvironmentVariables</key>"));
        assert!(plist.contains("<string>tok&amp;en&lt;1&gt;&quot;quoted&quot;</string>"));
    }

    #[test]
    fn launchctl_output_values_are_parsed() {
        let details = "\tstate = running\n\tpid = 123\n";
        assert_eq!(launchctl_value(details, "state"), Some("running"));
        assert_eq!(launchctl_value(details, "pid"), Some("123"));
    }
}

#[cfg(test)]
mod systemd_tests {
    use super::*;

    #[test]
    fn systemd_unit_runs_the_same_web_service() {
        let unit = render_systemd_unit(Path::new("/tmp/Cron Box/100%/cronbox"), None).unwrap();
        assert!(unit.contains("ExecStart=\"/tmp/Cron Box/100%%/cronbox\" serve --no-open"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(unit.contains("KillMode=control-group"));
        assert!(!unit.contains("Environment="));
    }

    #[test]
    fn systemd_unit_embeds_auth_token_when_configured() {
        let unit = render_systemd_unit(Path::new("/tmp/cronbox"), Some("tok\"en\\$x")).unwrap();
        assert!(unit.contains("Environment=\"CRONBOX_AUTH_TOKEN=tok\\\"en\\\\\\$x\""));
    }

    #[test]
    fn systemd_env_escapes_backslash_quote_and_dollar() {
        assert_eq!(escape_systemd_env_value(r#"a"b\c$d"#), r#"a\"b\\c\$d"#);
    }
}
