//! Internal failures: never show Rust text or backtraces to the user.

use std::io::Write;
use std::path::PathBuf;

/// Where internal details are written so the user can share them if needed.
pub fn log_path() -> PathBuf {
    std::env::temp_dir().join("metagente-internal.log")
}

fn append_log(text: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(file, "{}", text);
    }
}

/// Replaces the default panic output with one plain sentence.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        append_log(&format!("internal panic: {}", info));
        eprintln!(
            "Something went wrong inside Metagente. This is not a mistake in your agent.\n\
             Details were saved in {}",
            log_path().display()
        );
    }));
}

/// Maps any unexpected error to one plain sentence and a log file.
pub fn report_unexpected(detail: &str) -> String {
    append_log(&format!("internal error: {}", detail));
    format!(
        "Something went wrong inside Metagente. This is not a mistake in your agent.\n\
         Details were saved in {}",
        log_path().display()
    )
}
