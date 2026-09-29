//! Parse selected documents in a separate restricted process so corrupt files cannot kill the UI.
use app_core::{AppError, AppResult};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

fn failure(message: impl ToString) -> AppError {
    AppError::new(
        app_core::AppErrorCode::DocumentParseFailed,
        message.to_string(),
        "Check the document format and size, or split it into smaller files.",
    )
}
fn escape(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

pub async fn parse(
    path: PathBuf,
    cancel: CancellationToken,
) -> AppResult<documents::ParsedDocument> {
    let binary = std::env::current_exe().map_err(failure)?;
    parse_with_binary(&binary, &path, cancel).await
}
async fn parse_with_binary(
    binary: &Path,
    path: &Path,
    cancel: CancellationToken,
) -> AppResult<documents::ParsedDocument> {
    if cancel.is_cancelled() {
        return Err(failure("Document parsing cancelled."));
    }
    let path = path.canonicalize().map_err(failure)?;
    let binary = binary.canonicalize().map_err(failure)?;
    let executable = escape(&binary);
    let source = escape(&path);
    let directory = escape(
        binary
            .parent()
            .ok_or_else(|| failure("Parser location unavailable"))?,
    );
    let profile = format!(
        r#"(version 1)
(allow default)
(deny network* file-read* file-write* process-fork)
(allow file-read-metadata file-map-executable)
(allow process-exec (literal "{executable}"))
(allow file-read* (literal "/") (subpath "/System") (subpath "/usr") (subpath "/Library") (literal "/dev/urandom") (literal "/dev/random") (literal "/dev/null") (subpath "{directory}") (literal "{source}"))
"#
    );
    let mut child = tokio::process::Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile])
        .arg(&binary)
        .arg("--parse-document")
        .arg(&path)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(if cfg!(test) {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .kill_on_drop(true)
        .spawn()
        .map_err(failure)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| failure("Parser output unavailable"))?;
    let reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await
            .map(|_| bytes)
    });
    let pid = sysinfo::Pid::from_u32(
        child
            .id()
            .ok_or_else(|| failure("Parser process unavailable"))?,
    );
    let mut system = sysinfo::System::new();
    let mut interval = tokio::time::interval(Duration::from_millis(25));
    let deadline = tokio::time::sleep(Duration::from_secs(20));
    tokio::pin!(deadline);
    let status = loop {
        tokio::select! {
            _=cancel.cancelled()=>{let _=child.kill().await;break Err(failure("Document parsing cancelled."));},
            _=&mut deadline=>{let _=child.kill().await;break Err(failure("Document parsing exceeded 20 seconds. Split the document and retry."));},
            _=interval.tick()=>{
                system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]),true);
                if system.process(pid).is_some_and(|p|p.memory()>512*1024*1024) {let _=child.kill().await;break Err(failure("Document parsing exceeded the memory budget. Split the document and retry."));}
            },
            status=child.wait()=>break status.map_err(failure),
        }
    };
    let bytes = reader.await.map_err(failure)?.map_err(failure)?;
    let status = status?;
    if !status.success() {
        return Err(failure(format!(
            "The isolated document parser stopped unexpectedly ({status})."
        )));
    }
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(failure("Parser output exceeded the size limit."));
    }
    serde_json::from_slice::<Result<documents::ParsedDocument, String>>(&bytes)
        .map_err(failure)?
        .map_err(failure)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires a built native executable and macOS sandbox execution"]
    async fn isolated_parser_roundtrips_and_rejects_invalid_files() {
        let binary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/debug/offline-ai-assistant");
        let directory =
            std::env::temp_dir().join(format!("offline-ai-parser-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("notes.txt");
        std::fs::write(&file, "Private station notes: opens at 9 AM.").unwrap();
        let parsed = parse_with_binary(&binary, &file, CancellationToken::new())
            .await
            .unwrap();
        assert!(parsed.pages[0].contains("9 AM"));
        let bad = directory.join("bad.pdf");
        std::fs::write(&bad, "not a PDF").unwrap();
        assert!(parse_with_binary(&binary, &bad, CancellationToken::new())
            .await
            .is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
