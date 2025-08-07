// example_error_init — пример с ошибкой инициализации

use loglib::{Logger, fatal, debug, LogLevel};

const APP_NAME: &str = "example_error_init";
const APP_VERSION: &str = "1.0.0";

fn main() {
    // 1. Преамбула
    let logger = match Logger::system_only(APP_NAME) {
        Ok(l) => l,
        Err(_) => {
            eprintln!("[FATAL] Cannot initialize system logger.");
            std::process::exit(1);
        }
    };

    let _ = logger.platform_log(LogLevel::Info, &format!("Starting {} v{}", APP_NAME, APP_VERSION));

    // 2. Инициализация — имитируем ошибку
    let log_dir = "/root/forbidden"; // Недоступная директория
    let file_logger = match Logger::file_only(log_dir, "error.log", 1024, 1) {
        Ok(l) => l,
        Err(e) => {
            let _ = logger.platform_log(LogLevel::Error, &format!("Failed to open log file in {}: {}", log_dir, e));
            std::process::exit(1);
        }
    };

    // Дополнительная фатальная ошибка (например, не хватило памяти)
    let enough_memory = false;
    if !enough_memory {
        fatal!(file_logger, "Insufficient memory to continue");
        let _ = logger.platform_log(LogLevel::Error, "Application failed to initialize");
        std::process::exit(1);
    }

    // 3. Основной код (не достигается)
    debug!(file_logger, "This will not be logged");

    // 4. Финальная часть
    let _ = logger.platform_log(LogLevel::Info, "Application finished successfully");
}