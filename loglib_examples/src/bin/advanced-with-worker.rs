// example_advanced — многопоточный пример с "классом" Worker
// Каждый поток — экземпляр структуры Worker, которой передаётся клон логгера

use loglib::{Logger, debug, warning, error};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const APP_NAME: &str = "example_advanced";
const APP_VERSION: &str = "1.1.0";

// Структура, моделирующая "класс" потока
pub struct Worker {
    id: u32,
    log: Arc<Logger>, // Потокобезопасная обёртка над логгером
}

impl Worker {
    pub fn new(id: u32, log: Arc<Logger>) -> Self {
        Self { id, log }
    }

    pub fn run(&self) {
        debug!(self.log, "Worker {} started execution", self.id);

        // Имитация работы
        thread::sleep(Duration::from_millis(50 + (self.id as u64) * 100));

        if self.id % 2 == 1 {
            warning!(self.log, "Worker {} detected odd workload", self.id);
        }

        // Имитация ошибки у одного из воркеров
        if self.id == 2 {
            error!(self.log, "Worker {} encountered a transient error", self.id);
        }

        debug!(self.log, "Worker {} finished", self.id);
    }
}

fn main() {
    // 1. Преамбула: запись в системный лог
    let system_logger = match Logger::system_only(APP_NAME) {
        Ok(l) => l,
        Err(_) => {
            eprintln!("[FATAL] Cannot initialize system logger. Exiting.");
            std::process::exit(1);
        }
    };

    let _ = system_logger.platform_log(
        loglib::LogLevel::Info,
        &format!("Starting {} v{}", APP_NAME, APP_VERSION),
    );

    // 2. Инициализация: открываем файловый лог
    let file_logger = match Logger::file_only("logs", "advanced.log", 1024 * 1024, 5) {
        Ok(l) => l,
        Err(e) => {
            let _ = system_logger.platform_log(
                loglib::LogLevel::Error,
                &format!("Failed to create log file: {}", e),
            );
            std::process::exit(1);
        }
    };

    // Оборачиваем логгер в Arc, чтобы безопасно клонировать между потоками
    let shared_logger = Arc::new(file_logger);

    debug!(shared_logger, "Main thread initialized, spawning workers...");

    // 3. Основной код: создание потоков с объектами Worker
    let mut handles = vec![];

    for i in 0..4 {
        let logger_clone = Arc::clone(&shared_logger); // Клонируем Arc

        let handle = thread::spawn(move || {
            let worker = Worker::new(i, logger_clone);
            worker.run();
        });

        handles.push(handle);
    }

    // Ожидание завершения всех потоков
    for h in handles {
        let _ = h.join();
    }

    debug!(shared_logger, "All workers have finished");

    // 4. Финальная часть: завершение
    let _ = system_logger.platform_log(
        loglib::LogLevel::Info,
        "Application finished successfully",
    );
}