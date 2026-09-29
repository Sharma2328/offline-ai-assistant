//! macOS code runner: OS-level network/filesystem restrictions plus hard process limits.
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

const HARNESS: &str = r#"
import sys, json, resource
resource.setrlimit(resource.RLIMIT_CPU, (2, 2))
resource.setrlimit(resource.RLIMIT_FSIZE, (1048576, 1048576))
resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))
resource.setrlimit(resource.RLIMIT_NPROC, (0, 0))
# macOS does not support reducing RLIMIT_AS/DATA. The parent enforces RSS.
open('ready', 'w').close()
data = json.load(sys.stdin)
namespace = {}
try:
    exec(compile(data['code'], '<model>', 'exec'), namespace)
    exec(compile(data['tests'], '<tests>', 'exec'), namespace)
except BaseException:
    sys.exit(1)
"#;
fn source(output: &str) -> &str {
    let trimmed = output.trim();
    if let Some(code) = trimmed
        .strip_prefix("```python")
        .or_else(|| trimmed.strip_prefix("```"))
    {
        code.trim().strip_suffix("```").unwrap_or(code).trim()
    } else {
        trimmed
    }
}

pub async fn run(output: &str, tests: &str, cancel: CancellationToken) -> Result<f64, String> {
    if !cfg!(target_os = "macos") {
        return Err("Restricted coding execution is available on macOS only.".into());
    }
    if !std::path::Path::new("/usr/bin/sandbox-exec").is_file() {
        return Err(
            "The required OS sandbox is unavailable. Coding cases were not executed.".into(),
        );
    }
    let python = std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|p| p.join("python3"))
                .find(|p| p.is_file())
        })
        .unwrap_or_else(|| PathBuf::from("/usr/bin/python3"));
    // Resolve version-manager shims before applying the restricted child environment.
    let resolved = std::process::Command::new(&python)
        .args(["-I", "-c", "import sys; print(sys.executable)"])
        .output()
        .map_err(|e| e.to_string())?;
    if !resolved.status.success() {
        return Err("Python 3 is required for coding benchmarks.".into());
    }
    let python = PathBuf::from(
        String::from_utf8(resolved.stdout)
            .map_err(|e| e.to_string())?
            .trim(),
    );
    if !python.is_absolute() || !python.is_file() {
        return Err("Could not resolve a Python 3 executable.".into());
    }
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = scratch.path().canonicalize().map_err(|e| e.to_string())?;
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let python_root = python
        .canonicalize()
        .map_err(|e| e.to_string())?
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Python installation directory unavailable")?
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let profile = format!(
        r#"(version 1) (deny default)
(allow process-exec sysctl-read)
(allow process-info* signal mach-priv-task-port (target same-sandbox))
(allow ipc-posix-shm ipc-posix-sem)
(allow file-ioctl (literal "/dev/null") (literal "/dev/random") (literal "/dev/urandom"))
(allow mach-lookup (global-name "com.apple.system.logger") (global-name "com.apple.logd"))
(allow file-read-metadata file-map-executable)
(allow file-read* (literal "/") (subpath "/System") (subpath "/usr") (subpath "/bin") (subpath "/private/preboot/Cryptexes") (literal "/dev/urandom") (literal "/dev/random") (subpath "/Library/Developer") (subpath "/opt/homebrew") (subpath "{python_root}") (subpath "{escaped}"))
(allow file-write* (subpath "{escaped}") (literal "/dev/null"))
(deny network*)
"#
    );
    let started = std::time::Instant::now();
    let mut child = tokio::process::Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile])
        .arg(python)
        .args(["-I", "-c", HARNESS])
        .current_dir(&path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/opt/homebrew/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(
            std::fs::File::create(path.join("stderr.txt")).map_err(|e| e.to_string())?,
        ))
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    let input = serde_json::to_vec(&serde_json::json!({"code":source(output),"tests":tests}))
        .map_err(|e| e.to_string())?;
    if input.len() > 128 * 1024 {
        return Err("Generated code exceeds the runner input limit.".into());
    }
    let mut stdin = child.stdin.take().ok_or("Runner stdin unavailable.")?;
    stdin.write_all(&input).await.map_err(|e| e.to_string())?;
    drop(stdin);
    let pid = sysinfo::Pid::from_u32(child.id().ok_or("Runner PID unavailable")?);
    let mut system = sysinfo::System::new();
    let deadline = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(deadline);
    let mut monitor = tokio::time::interval(Duration::from_millis(25));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => { let _ = child.kill().await; return Err("Cancelled".into()); },
            _ = &mut deadline => { let _ = child.kill().await; return Err("BENCH_CASE_TIMEOUT".into()); },
            _ = monitor.tick() => {
                system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                if system.process(pid).is_some_and(|process| process.memory() > 512 * 1024 * 1024) {
                    let _ = child.kill().await;
                    return Err("BENCH_MEMORY_LIMIT".into());
                }
            },
            result = child.wait() => {
                let status = result.map_err(|error|error.to_string())?;
                if !path.join("ready").is_file() {
                    return Err("The restricted Python runtime could not initialize. No score was assigned.".into());
                }
                #[cfg(unix)] {
                    use std::os::unix::process::ExitStatusExt;
                    if matches!(status.signal(),Some(9|24)) && started.elapsed()>Duration::from_millis(1800) {return Err("BENCH_CASE_TIMEOUT".into());}
                }
                return Ok(if status.success() {100.0} else {0.0});
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fenced_python_is_unwrapped() {
        assert_eq!(source("```python\ndef f(): pass\n```"), "def f(): pass");
    }
    #[tokio::test]
    #[ignore = "requires macOS sandbox execution"]
    async fn sandbox_denies_private_reads_and_stops_resource_abuse() {
        assert_eq!(
            run("while True: pass", "", CancellationToken::new())
                .await
                .unwrap_err(),
            "BENCH_CASE_TIMEOUT"
        );
        let private = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(private.path(), "private fixture").unwrap();
        let code = format!("def f():\n    try:\n        open({:?}).read()\n        return False\n    except OSError:\n        return True",private.path().to_string_lossy());
        assert_eq!(
            run(&code, "assert f()", CancellationToken::new())
                .await
                .unwrap(),
            100.0
        );
        let cancel = CancellationToken::new();
        cancel.cancel();
        let start = std::time::Instant::now();
        assert_eq!(
            run("while True: pass", "", cancel).await.unwrap_err(),
            "Cancelled"
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        assert_eq!(
            run(
                "data = bytearray(800 * 1024 * 1024)\nwhile True: pass",
                "",
                CancellationToken::new()
            )
            .await
            .unwrap_err(),
            "BENCH_MEMORY_LIMIT"
        );
    }

    #[tokio::test]
    #[ignore = "requires macOS sandbox execution; run explicitly during release validation"]
    async fn sandbox_enforces_network_and_filesystem_limits() {
        let code="import socket\ndef f():\n    try:\n        socket.socket().connect(('1.1.1.1', 443))\n        return False\n    except OSError:\n        pass\n    try:\n        open('/tmp/offline-ai-sandbox-escape', 'w')\n        return False\n    except OSError:\n        return True";
        assert_eq!(
            run(code, "assert f()", CancellationToken::new())
                .await
                .unwrap(),
            100.0
        );
        assert_eq!(
            run(
                "def f(): return 4",
                "assert f() == 4",
                CancellationToken::new()
            )
            .await
            .unwrap(),
            100.0
        );
    }
}
