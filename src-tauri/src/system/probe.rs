//! Fixed-argv, read-only host queries (`nvidia-smi`, `systemctl list-units`,
//! `docker ps`, `lspci`, …).
//!
//! Security: these helpers exist for *inspection only*. Every call site
//! passes a hardcoded program and hardcoded arguments (no user or model text
//! ever reaches argv), there is no shell, the environment is the executor's
//! minimal [`safe_env`](crate::security::executor::safe_env), output is
//! capped and the process is killed on timeout. Anything that changes the
//! system goes through `security::executor` (policy → confirmation → audit)
//! instead, so these reads are not audited, like `sysinfo` reads.

use crate::security::executor;
use std::path::Path;
use std::time::Duration;

/// Output cap for probes (bytes).
const PROBE_CAP: usize = 4 * 1024 * 1024;

/// Run `program args` and return stdout if it exited 0 within `timeout`.
/// Missing programs, failures and timeouts are `None` (the caller treats the
/// data as unavailable rather than guessing).
pub async fn run(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let cwd = std::env::temp_dir();
    match executor::run_process(program, &args, Path::new(&cwd), timeout, PROBE_CAP).await {
        Ok(out) if out.exit_code == Some(0) => Some(out.stdout),
        Ok(out) => {
            tracing::debug!(program, code = ?out.exit_code, stderr = %out.stderr.chars().take(200).collect::<String>(), "probe failed");
            None
        }
        Err(e) => {
            tracing::debug!(program, error = %e, "probe unavailable");
            None
        }
    }
}

/// Whether `program` is on the minimal PATH.
pub fn available(program: &str) -> bool {
    let path = executor::safe_env()
        .into_iter()
        .find(|(k, _)| k == "PATH")
        .map(|(_, v)| v)
        .unwrap_or_default();
    std::env::split_paths(&path).any(|d| d.join(program).is_file())
}
