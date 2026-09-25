//! Privilege elevation adapters.
//!
//! OMNIX never handles passwords. Elevation is delegated to the platform's
//! graphical authentication prompt:
//!
//! | Platform | Mechanism |
//! |----------|-----------|
//! | Linux    | `pkexec /bin/sh -c <cmd>` (polkit agent prompt) |
//! | macOS    | `osascript -e 'do shell script "<cmd>" with administrator privileges'` |
//! | Windows  | PowerShell `Start-Process -Verb RunAs` (ShellExecuteEx "runas" → UAC) |
//!
//! Elevation is only reachable when `security.enable_sudo` is on (default
//! off) and after a native confirmation dialog. When the tool is missing a
//! clear [`AppError::Unavailable`] is returned instead of falling back to
//! anything else.

use crate::error::{AppError, AppResult};
use std::path::PathBuf;

/// Build `(program, args)` that runs `raw` elevated on the current platform.
pub fn elevated_command(raw: &str) -> AppResult<(String, Vec<String>)> {
    #[cfg(target_os = "linux")]
    {
        let pkexec = find_in_path("pkexec").ok_or_else(|| {
            AppError::Unavailable(
                "pkexec was not found; install polkit (policykit-1) to run privileged commands"
                    .into(),
            )
        })?;
        Ok(linux_pkexec(&pkexec.to_string_lossy(), raw))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(macos_osascript(raw))
    }
    #[cfg(windows)]
    {
        Ok(windows_runas(raw))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = raw;
        Err(AppError::Unavailable(
            "privilege elevation is not supported on this platform".into(),
        ))
    }
}

/// Linux: pkexec runs `/bin/sh -c raw` as root after polkit authentication.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))] // per-platform builder
pub fn linux_pkexec(pkexec: &str, raw: &str) -> (String, Vec<String>) {
    (
        pkexec.to_string(),
        vec!["/bin/sh".into(), "-c".into(), raw.to_string()],
    )
}

/// macOS: AppleScript `do shell script ... with administrator privileges`.
/// The command is embedded in an AppleScript string literal, so `\` and `"`
/// are escaped to prevent breaking out of the literal.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))] // per-platform builder
pub fn macos_osascript(raw: &str) -> (String, Vec<String>) {
    let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
    (
        "/usr/bin/osascript".into(),
        vec![
            "-e".into(),
            format!("do shell script \"{escaped}\" with administrator privileges"),
        ],
    )
}

/// Windows: `Start-Process -Verb RunAs` triggers UAC via ShellExecuteEx. The
/// command is embedded in a PowerShell single-quoted literal, where the only
/// escape is doubling `'`. The elevated process's exit code is propagated;
/// its output cannot be captured across the UAC boundary.
#[cfg_attr(not(any(windows, test)), allow(dead_code))] // per-platform builder
pub fn windows_runas(raw: &str) -> (String, Vec<String>) {
    let quoted = raw.replace('\'', "''");
    (
        "powershell.exe".into(),
        vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            format!(
                "$p = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/C', '{quoted}') \
                 -Verb RunAs -Wait -PassThru; exit $p.ExitCode"
            ),
        ],
    )
}

/// Locate an executable on `PATH`.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))] // only Linux needs lookup
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| is_executable(p))
}

#[cfg(unix)]
fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &std::path::Path) -> bool {
    p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkexec_wraps_in_sh() {
        let (p, a) = linux_pkexec("/usr/bin/pkexec", "apt update && apt upgrade");
        assert_eq!(p, "/usr/bin/pkexec");
        assert_eq!(a, vec!["/bin/sh", "-c", "apt update && apt upgrade"]);
    }

    #[test]
    fn osascript_escapes_quotes_and_backslashes() {
        let (_, a) = macos_osascript(r#"echo "hi" \ there"#);
        assert_eq!(
            a[1],
            r#"do shell script "echo \"hi\" \\ there" with administrator privileges"#
        );
    }

    #[test]
    fn powershell_doubles_single_quotes() {
        let (_, a) = windows_runas("echo it's");
        assert!(a[3].contains("'echo it''s'"));
    }

    #[cfg(unix)]
    #[test]
    fn finds_sh_on_path() {
        assert!(find_in_path("sh").is_some());
        assert!(find_in_path("definitely-not-a-real-binary-omnix").is_none());
    }
}
