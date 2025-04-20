use chrono::Local;
use fern::Dispatch;
use log::LevelFilter;
use std::fs;
use std::path::PathBuf;

use std::sync::atomic::{AtomicBool, Ordering};
static STDOUT_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn init() {
    let path: PathBuf = dirs::home_dir().unwrap().join(".dockup").join("logs");
    fs::create_dir_all(&path).unwrap();

    let log_file_path = path.join("output.log");

    // Formatter for file: includes timestamp
    let file_config = Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .chain(fern::log_file(log_file_path).unwrap());

    // Formatter for stdout: no timestamp
    let stdout_config = Dispatch::new()
        .format(|out, message, record| {
            if STDOUT_ENABLED.load(Ordering::Relaxed) {
                out.finish(format_args!("[{}] {}", record.level(), message))
            } else {
                out.finish(format_args!("")) // or drop silently
            }
        })
        .chain(std::io::stdout());

    Dispatch::new()
        .level(LevelFilter::Debug)
        .chain(stdout_config)
        .chain(file_config)
        .apply()
        .unwrap();
}

pub fn disable_stdout_logging() {
    STDOUT_ENABLED.store(false, Ordering::Relaxed);
}

pub fn enable_stdout_logging() {
    STDOUT_ENABLED.store(true, Ordering::Relaxed);
}
