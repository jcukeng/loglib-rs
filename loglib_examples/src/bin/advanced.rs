// example_advanced — более сложный пример: потоки, форматирование

use loglib::{Logger, debug, warning, error, fatal, LogLevel};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const APP_NAME: &str = "example_advanced";
const APP_VERSION: &str = "1.1.0";

fn main() {
    // 1. Преамбула
    let logger = match Logger::system_only(APP_NAME) {
        Ok(l) => l,
        Err(_) => {
            eprintln!("[FATAL] Cannot initialize system logger.");
            std::process::exit(1);
        }
    };

    // ✅ Заменяем Info → Debug
    let _ = logger.platform_log(LogLevel::Debug, &format!("Starting {} v{}", APP_NAME, APP_VERSION));

    // 2. Инициализация
    let file_logger = match Logger::file_only("logs", "advanced.log", 1024 * 1024, 5) {
        Ok(l) => l,
        Err(e) => {
            let _ = logger.platform_log(LogLevel::Error, &format!("Failed to create log file: {}", e));
            std::process::exit(1);
        }
    };

    // Оборачиваем в Arc
    let file_logger = Arc::new(file_logger);

    // Пример фатальной ошибки на этапе инициализации
    let config_ok = true;
    if !config_ok {
        fatal!(file_logger, "Configuration validation failed");
        let _ = logger.platform_log(LogLevel::Error, "Application failed to initialize");
        std::process::exit(1);
    }

    debug!(file_logger, "Configuration loaded");
    debug!(file_logger, "Starting worker threads");

    // 3. Основной код — многопоточность
    let handles: Vec<_> = (0..3)
        .map(|i| {
            let logger_clone = Arc::clone(&file_logger);
            thread::spawn(move || {
                debug!(logger_clone, "Worker thread {} started", i);
                thread::sleep(Duration::from_millis(100 * (i + 1)));

                if i == 1 {
                    warning!(logger_clone, "Worker {} detected high latency", i);
                }

                debug!(logger_clone, "Worker {} finished", i);
            })
        })
        .collect();

    for h in handles {
        let _ = h.join();
    }

    error!(file_logger, "Finalizing with one last error log");

    // 4. Финальная часть
    let _ = logger.platform_log(LogLevel::Debug, "Application finished successfully");
}