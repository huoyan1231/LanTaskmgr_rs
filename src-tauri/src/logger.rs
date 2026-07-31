//! 追加式文本日志，对应原程序的 log.txt。

use std::fmt::Display;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// 超过这个大小就滚动一次，避免像原程序那样无限增长。
const MAX_LOG_BYTES: u64 = 1024 * 1024;

static LOCK: Mutex<()> = Mutex::new(());

pub fn log_path() -> PathBuf {
    crate::settings::data_dir().join("log.txt")
}

pub fn log(msg: impl Display) {
    let _guard = LOCK.lock();
    let path = log_path();

    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::rename(&path, path.with_extension("old.txt"));
        }
    }

    let line = format!("{} | {}\r\n", chrono::Local::now().format("%Y/%m/%d %H:%M:%S"), msg);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}
