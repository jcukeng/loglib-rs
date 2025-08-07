//! example_rotation — демонстрация ротации логов по размеру

use loglib::{Logger, debug, warning, error};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const APP_NAME: &str = "example_rotation";
const APP_VERSION: &str = "1.0.0";

// Маленький максимальный размер — чтобы ротация сработала быстро
const MAX_LOG_SIZE: u64 = 4096; // 4 КБ
const MAX_LOG_FILES: usize = 15;

fn main() {
    // 1. Преамбула: пишем в системный лог
    let system_logger = match Logger::system_only(APP_NAME) {
        Ok(l) => l,
        Err(_) => {
            eprintln!("[FATAL] Cannot initialize system logger. Exiting.");
            std::process::exit(1);
        }
    };

    let _ = system_logger.platform_log(
        loglib::LogLevel::Debug,
        &format!("Starting {} v{}", APP_NAME, APP_VERSION),
    );

    // 2. Инициализация: открываем файловый лог с маленьким размером
    let file_logger = match Logger::file_only("logs", "rotation.log", MAX_LOG_SIZE, MAX_LOG_FILES) {
        Ok(l) => l,
        Err(e) => {
            let _ = system_logger.platform_log(
                loglib::LogLevel::Error,
                &format!("Failed to create log file: {}", e),
            );
            std::process::exit(1);
        }
    };

    let file_logger = Arc::new(file_logger);

    debug!(file_logger, "Logger initialized with max_size={} bytes, max_files={}", MAX_LOG_SIZE, MAX_LOG_FILES);
    debug!(file_logger, "This example will generate a lot of log messages to trigger rotation");

    // 3. Основной код: генерируем много логов
    for i in 0..100 {
        debug!(file_logger, "This is a debug message number {}", i);
        if i % 30 == 0 {
            warning!(file_logger, "Warning message at iteration {}", i);
        }
        if i % 35 == 0 {
            error!(file_logger, "Error message at iteration {}", i);
        }

        // Делаем паузу, чтобы видеть процесс
        thread::sleep(Duration::from_millis(10));
    }

    debug!(file_logger, "Log generation completed. Check 'logs/' directory for rotated files.");

    // Дополнительная проверка: запускаем ещё раз, чтобы увидеть, как старые файлы сдвигаются
    debug!(file_logger, "Restarting log generation to test file shifting...");
    for i in 0..100 {
        debug!(file_logger, "Post-check message #{}", i);
        thread::sleep(Duration::from_millis(5));
    }

    // 4. Финальная часть
    let _ = system_logger.platform_log(
        loglib::LogLevel::Debug,
        "Application finished successfully",
    );
}