// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--parse-document")) {
        let result = args
            .next()
            .ok_or_else(|| "Missing document path".to_string())
            .and_then(|path| documents::parse(std::path::Path::new(&path)));
        serde_json::to_writer(std::io::stdout().lock(), &result).expect("write parser response");
        return;
    }
    offline_ai_assistant_lib::run();
}
