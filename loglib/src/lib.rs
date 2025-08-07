use chrono::Local;
use once_cell::sync::Lazy;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ===== Уровни логгирования =====

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }

    #[cfg(target_os = "linux")]
    fn to_syslog_level(&self) -> syslog::Severity {
        use syslog::Severity::*;
        match self {
            LogLevel::Trace | LogLevel::Debug | LogLevel::Info => LOG_INFO,
            LogLevel::Warning => LOG_WARNING,
            LogLevel::Error | LogLevel::Fatal => LOG_ERR,
        }
    }

    #[cfg(target_os = "windows")]
    fn to_winlog_level(&self) -> winlog_rs::LogLevel {
        match self {
            LogLevel::Trace => winlog_rs::LogLevel::Trace,
            LogLevel::Debug | LogLevel::Info => winlog_rs::LogLevel::Debug,
            LogLevel::Warning => winlog_rs::LogLevel::Warning,
            LogLevel::Error | LogLevel::Fatal => winlog_rs::LogLevel::Error,
        }
    }
}

// ===== Глобальный уровень фильтрации =====

static GLOBAL_LOG_LEVEL: AtomicUsize = AtomicUsize::new(1); // по умолчанию Debug

pub fn set_global_log_level(level: LogLevel) {
    GLOBAL_LOG_LEVEL.store(level as usize, Ordering::SeqCst);
}

fn should_log(level: LogLevel) -> bool {
    (level as usize) >= GLOBAL_LOG_LEVEL.load(Ordering::SeqCst)
}

// ===== Системные логгеры (платформозависимо) =====

#[cfg(target_os = "linux")]
type SystemLogger = syslog::Logger;

#[cfg(target_os = "windows")]
type SystemLogger = winlog_rs::WinEventLogger;

// ===== Кастомный ротирующий писатель =====

struct RotatingWriter {
    dir: PathBuf,
    basename: String,
    max_size: u64,
    max_files: usize,
    file: Arc<Mutex<Option<File>>>,
    app_info: String,
    system_logger: Option<SystemLogger>, // для логов об ошибках
}

impl RotatingWriter {
    const MIN_SIZE: u64 = 256; // минимальный размер, чтобы вместить заголовок + пару строк

    fn new<P: AsRef<Path>>(
        dir: P,
        basename: &str,
        max_size: u64,
        max_files: usize,
        app_info: &str,
        system_logger: Option<SystemLogger>,
    ) -> io::Result<Self> {
        if max_size < Self::MIN_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("max_size must be at least {} bytes", Self::MIN_SIZE),
            ));
        }

        let dir = dir.as_ref().to_path_buf();
        let path = dir.join(basename);

        fs::create_dir_all(&dir)?;

        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        Ok(RotatingWriter {
            dir,
            basename: basename.to_owned(),
            max_size,
            max_files,
            file: Arc::new(Mutex::new(Some(file))),
            app_info: app_info.to_owned(),
            system_logger,
        })
    }

    fn write(&self, level: LogLevel, message: &str) {
        if !should_log(level) {
            return;
        }

        let mut file_lock = self.file.lock().unwrap();

        // Проверяем размер
        let need_rotate = if let Some(ref mut file) = *file_lock {
            let pos = file.seek(SeekFrom::End(0)).unwrap_or(0);
            pos >= self.max_size
        } else {
            false
        };

        if need_rotate {
            drop(file_lock); // освобождаем

            if let Err(e) = self.rotate() {
                self.log_to_system(LogLevel::Error, &format!("Failed to rotate log: {}", e));
            }

            let mut file_lock = self.file.lock().unwrap();
            *file_lock = match self.reopen_with_header() {
                Ok(f) => f,
                Err(e) => {
                    self.log_to_system(LogLevel::Error, &format!("Failed to reopen log: {}", e));
                    return;
                }
            };

            // Пишем в новый файл
            if let Some(ref mut file) = *file_lock {
                let line = self.format_log_line(level, message);
                let _ = writeln!(file, "{}", line);
                let _ = file.flush();
            }
        } else {
            // Пишем в текущий файл
            if let Some(ref mut file) = *file_lock {
                let line = self.format_log_line(level, message);
                let _ = writeln!(file, "{}", line);
                let _ = file.flush();
            }
        }
    }

    fn format_log_line(&self, level: LogLevel, message: &str) -> String {
        let now = Local::now();
        let pid = process::id();
        let thread_id = format!("{:?}", std::thread::current().id());
        format!(
            "[{}] {} PID:{} TID:{} {}",
            now.format("%Y-%m-%d %H:%M:%S%.3f"),
            level.as_str(),
            pid,
            thread_id,
            message
        )
    }

    fn reopen(&self) -> io::Result<Option<File>> {
        let path = self.dir.join(&self.basename);
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map(Some)
            .map_err(|e| {
                self.log_to_system(
                    LogLevel::Error,
                    &format!("Failed to reopen log file: {}", e),
                );
                e
            })
    }

    fn reopen_with_header(&self) -> io::Result<Option<File>> {
        let mut file = self.reopen()?; // <-- добавили mut

        if let Some(ref mut f) = file {
            let header = format!("[ROTATION] Logger restarted — {}", self.app_info);
            let line = self.format_log_line(LogLevel::Debug, &header);
            let _ = writeln!(f, "{}", line);
            let _ = f.flush();
        }

        Ok(file)
    }

    fn rotate(&self) -> io::Result<()> {
        // Удаляем самый старый
        let old = self
            .dir
            .join(format!("{}.{}", self.basename, self.max_files));
        let _ = fs::remove_file(&old);

        // Сдвигаем файлы: .3 → .4, .2 → .3, ..., .1 → .2
        for i in (1..self.max_files).rev() {
            let src = self.dir.join(format!("{}.{}", self.basename, i));
            if src.exists() {
                let dst = self.dir.join(format!("{}.{}", self.basename, i + 1));
                let _ = fs::remove_file(&dst);
                if let Err(e) = fs::rename(&src, &dst) {
                    return Err(e);
                }
            }
        }

        // Текущий файл → становится .1
        let current = self.dir.join(&self.basename);
        if current.exists() {
            let dst = self.dir.join(format!("{}.1", self.basename));
            let _ = fs::remove_file(&dst);
            if let Err(e) = fs::rename(&current, &dst) {
                return Err(e);
            }
        }

        Ok(())
    }

    fn log_to_system(&self, level: LogLevel, msg: &str) {
        if let Some(ref logger) = self.system_logger {
            self.log_to_system_impl(logger, level, msg);
        }
    }

    #[cfg(target_os = "linux")]
    fn log_to_system_impl(&self, logger: &SystemLogger, level: LogLevel, msg: &str) {
        let severity = level.to_syslog_level();
        let _ = syslog::write(logger, severity, msg);
    }

    #[cfg(target_os = "windows")]
    fn log_to_system_impl(&self, logger: &SystemLogger, level: LogLevel, msg: &str) {
        logger.report(level.to_winlog_level(), msg);
    }
}

// ===== Основной логгер =====

pub struct Logger {
    rotating_writer: Option<Arc<RotatingWriter>>,
    system_logger: Option<SystemLogger>,
    app_name: String,
}

impl Logger {
    pub fn system_only(app_name: &str) -> std::io::Result<Self> {
        let system_logger = Self::init_system_logger(app_name)?;
        Ok(Logger {
            rotating_writer: None,
            system_logger,
            app_name: app_name.to_owned(),
        })
    }

    pub fn file_only<P: AsRef<Path>>(
        directory: P,
        filename: &str,
        max_size_bytes: u64,
        max_files: usize,
    ) -> std::io::Result<Self> {
        let writer = Arc::new(RotatingWriter::new(
            directory,
            filename,
            max_size_bytes,
            max_files,
            "UnknownApp",
            None,
        )?);
        Ok(Logger {
            rotating_writer: Some(writer),
            system_logger: None,
            app_name: "unnamed".to_owned(),
        })
    }

    pub fn file_and_system<P: AsRef<Path>>(
        app_name: &str,
        directory: P,
        filename: &str,
        max_size_bytes: u64,
        max_files: usize,
    ) -> std::io::Result<Self> {
        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("dev");
        let app_info = format!("{} v{}", app_name, version);

        let system_logger = Self::init_system_logger(app_name)?;
        let writer = Arc::new(RotatingWriter::new(
            directory,
            filename,
            max_size_bytes,
            max_files,
            &app_info,
            system_logger.clone(),
        )?);

        Ok(Logger {
            rotating_writer: Some(writer),
            system_logger,
            app_name: app_name.to_owned(),
        })
    }

    #[cfg(target_os = "linux")]
    fn init_system_logger(app_name: &str) -> std::io::Result<Option<SystemLogger>> {
        match syslog::unix(syslog::Facility::LOG_USER) {
            Ok(logger) => Ok(Some(logger)),
            Err(_) => Ok(None),
        }
    }

    #[cfg(target_os = "windows")]
    fn init_system_logger(app_name: &str) -> std::io::Result<Option<SystemLogger>> {
        Ok(Some(winlog_rs::WinEventLogger::new(app_name)))
    }

    pub fn set_log_level(&self, level: LogLevel) {
        set_global_log_level(level);
    }

    pub fn log(&self, args: std::fmt::Arguments) {
        if self.rotating_writer.is_none() {
            return;
        }
        let message = format!("{}", args);
        self.write_to_file(LogLevel::Debug, &message);
    }

    pub fn platform_log(&self, level: LogLevel, message: &str) {
        if self.system_logger.is_none() {
            return;
        }
        if should_log(level) {
            if let Some(ref logger) = self.system_logger {
                self.log_to_system(logger, level, message);
            }
        }
    }

    pub fn write_to_file(&self, level: LogLevel, message: &str) {
        if let Some(ref writer) = self.rotating_writer {
            writer.write(level, message);
        }
    }

    #[cfg(target_os = "linux")]
    fn log_to_system(&self, logger: &SystemLogger, level: LogLevel, msg: &str) {
        let severity = level.to_syslog_level();
        let _ = syslog::write(logger, severity, msg);
    }

    #[cfg(target_os = "windows")]
    fn log_to_system(&self, logger: &SystemLogger, level: LogLevel, msg: &str) {
        logger.report(level.to_winlog_level(), msg);
    }
}

// ===== Макросы =====

#[macro_export]
macro_rules! log {
    ($logger:expr, $($arg:tt)*) => {{
        $logger.log(std::format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! trace {
    ($logger:expr, $($arg:tt)*) => {{
        $logger.write_to_file($crate::LogLevel::Trace, &format!($($arg)*));
    }};
}
#[macro_export]
macro_rules! debug {
    ($logger:expr, $($arg:tt)*) => {{
        $logger.write_to_file($crate::LogLevel::Debug, &format!($($arg)*));
    }};
}
#[macro_export]
macro_rules! info {
    ($logger:expr, $($arg:tt)*) => {{
        $logger.write_to_file($crate::LogLevel::Info, &format!($($arg)*));
    }};
}
#[macro_export]
macro_rules! warning {
    ($logger:expr, $($arg:tt)*) => {{
        $logger.write_to_file($crate::LogLevel::Warning, &format!($($arg)*));
    }};
}
#[macro_export]
macro_rules! error {
    ($logger:expr, $($arg:tt)*) => {{
        $logger.write_to_file($crate::LogLevel::Error, &format!($($arg)*));
    }};
}
#[macro_export]
macro_rules! fatal {
    ($logger:expr, $($arg:tt)*) => {{
        $logger.write_to_file($crate::LogLevel::Fatal, &format!($($arg)*));
    }};
}

// ===== Глобальные макросы =====

#[macro_export]
macro_rules! glog {
    ($($arg:tt)*) => {{
        if let Some(ref logger) = *$crate::GLOBAL_LOGGER.lock().unwrap() {
            logger.log(std::format_args!($($arg)*));
        }
    }};
}
#[macro_export]
macro_rules! gtrace {
    ($($arg:tt)*) => {{
        if let Some(ref logger) = *$crate::GLOBAL_LOGGER.lock().unwrap() {
            logger.write_to_file($crate::LogLevel::Trace, &format!($($arg)*));
        }
    }};
}
#[macro_export]
macro_rules! gdebug {
    ($($arg:tt)*) => {{
        if let Some(ref logger) = *$crate::GLOBAL_LOGGER.lock().unwrap() {
            logger.write_to_file($crate::LogLevel::Debug, &format!($($arg)*));
        }
    }};
}
#[macro_export]
macro_rules! ginfo {
    ($($arg:tt)*) => {{
        if let Some(ref logger) = *$crate::GLOBAL_LOGGER.lock().unwrap() {
            logger.write_to_file($crate::LogLevel::Info, &format!($($arg)*));
        }
    }};
}
#[macro_export]
macro_rules! gwarning {
    ($($arg:tt)*) => {{
        if let Some(ref logger) = *$crate::GLOBAL_LOGGER.lock().unwrap() {
            logger.write_to_file($crate::LogLevel::Warning, &format!($($arg)*));
        }
    }};
}
#[macro_export]
macro_rules! gerror {
    ($($arg:tt)*) => {{
        if let Some(ref logger) = *$crate::GLOBAL_LOGGER.lock().unwrap() {
            logger.write_to_file($crate::LogLevel::Error, &format!($($arg)*));
        }
    }};
}
#[macro_export]
macro_rules! gfatal {
    ($($arg:tt)*) => {{
        if let Some(ref logger) = *$crate::GLOBAL_LOGGER.lock().unwrap() {
            logger.write_to_file($crate::LogLevel::Fatal, &format!($($arg)*));
        }
    }};
}

// ===== Глобальный логгер =====

static GLOBAL_LOGGER: Lazy<std::sync::Mutex<Option<Logger>>> =
    Lazy::new(|| std::sync::Mutex::new(None));

pub fn init_global_logger_file_only(
    directory: &str,
    filename: &str,
    max_size_bytes: u64,
    max_files: usize,
) -> std::io::Result<()> {
    let logger = Logger::file_only(directory, filename, max_size_bytes, max_files)?;
    *GLOBAL_LOGGER.lock().unwrap() = Some(logger);
    Ok(())
}

pub fn init_global_logger_system_only(app_name: &str) -> std::io::Result<()> {
    let logger = Logger::system_only(app_name)?;
    *GLOBAL_LOGGER.lock().unwrap() = Some(logger);
    Ok(())
}

pub fn init_global_logger_file_and_system(
    app_name: &str,
    directory: &str,
    filename: &str,
    max_size_bytes: u64,
    max_files: usize,
) -> std::io::Result<()> {
    let logger = Logger::file_and_system(app_name, directory, filename, max_size_bytes, max_files)?;
    *GLOBAL_LOGGER.lock().unwrap() = Some(logger);
    Ok(())
}
