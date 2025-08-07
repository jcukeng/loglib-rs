// example_tokio — асинхронный пример с tokio и shared Logger
// Показывает, как использовать loglib в асинхронном контексте

use loglib::{Logger, debug, warning, error};
use std::sync::Arc;
use tokio::task;
use tokio::time::{sleep, Duration};

const APP_NAME: &str = "example_tokio";
const APP_VERSION: &str = "1.0.0";

// Асинхронный "воркер"
pub struct Worker {
    id: u32,
    log: Arc<Logger>,
}

impl Worker {
    pub fn new(id: u32, log: Arc<Logger>) -> Self {
        Self { id, log }
    }

    pub async fn run(&self) {
        debug!(self.log, "Worker {} started (async)", self.id);

        // Имитация асинхронной работы
        sleep(Duration::from_millis(50 + (self.id as u64) * 100)).await;

        if self.id % 3 == 0 {
            warning!(self.log, "Worker {} has high priority task", self.id);
        }

        if self.id == 2 {
            error!(self.log, "Worker {} failed to process data", self.id);
        }

        debug!(self.log, "Worker {} completed", self.id);
    }
}

#[tokio::main]
async fn main() {
    // 1. Преамбула: системный лог
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

    // 2. Инициализация: файловый лог
    let file_logger = match Logger::file_only("logs", "tokio.log", 1024 * 1024, 5) {
        Ok(l) => l,
        Err(e) => {
            let _ = system_logger.platform_log(
                loglib::LogLevel::Error,
                &format!("Failed to open log file: {}", e),
            );
            std::process::exit(1);
        }
    };

    // Оборачиваем в Arc для шаринга между задачами
    let shared_logger = Arc::new(file_logger);

    debug!(shared_logger, "Tokio runtime initialized, spawning async tasks...");

    // 3. Основной код: запуск нескольких асинхронных задач
    let mut handles = vec![];

    for i in 0..5 {
        let logger_clone = Arc::clone(&shared_logger);
        let handle = task::spawn(async move {
            let worker = Worker::new(i, logger_clone);
            worker.run().await;
        });
        handles.push(handle);
    }

    // Ждём завершения всех задач
    for h in handles {
        let _ = h.await;
    }

    debug!(shared_logger, "All async tasks completed");

    // 4. Финальная часть
    let _ = system_logger.platform_log(
        loglib::LogLevel::Info,
        "Application finished successfully",
    );
}