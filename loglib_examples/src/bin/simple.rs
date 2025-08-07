// example_simple — простой пример: инициализация, логи в файл, завершение

use loglib::{Logger, debug, warning, error, fatal, LogLevel};

const APP_NAME: &str = "example_simple";
const APP_VERSION: &str = "1.0.0";

fn main() {
    // 1. Преамбула: пишем в системный лог
    let logger = match Logger::system_only(APP_NAME) {
        Ok(l) => l,
        Err(_) => {
            eprintln!("[FATAL] Cannot initialize system logger. Exiting.");
            std::process::exit(1);
        }
    };

    let _ = logger.platform_log(LogLevel::Info, &format!("Starting {} v{}", APP_NAME, APP_VERSION));

    // 2. Инициализация: открываем файловый лог
    let file_logger = match Logger::file_only("logs", "simple.log", 1024 * 1024, 3) {
        Ok(l) => l,
        Err(e) => {
            let _ = logger.platform_log(LogLevel::Error, &format!("Failed to open log file: {}", e));
            std::process::exit(1);
        }
    };

    // Фатальные ошибки инициализации пишем в файл (но файл уже открыт)
    if false {
        fatal!(file_logger, "Simulated fatal during init");
        let _ = logger.platform_log(LogLevel::Error, "Application failed to initialize");
        std::process::exit(1);
    }

    // 3. Основной код — только в файл
    debug!(file_logger, "Application initialized successfully");
    debug!(file_logger, "Processing data block #1");
    warning!(file_logger, "Non-critical issue detected");
    error!(file_logger, "An error occurred, but we continue");
    debug!(file_logger, "Processing data block #2");

    // 4. Финальная часть
    let _ = logger.platform_log(LogLevel::Info, "Application finished successfully");
}