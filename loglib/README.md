# loglib — Кроссплатформенная библиотека логгирования

`loglib` — это потокобезопасная библиотека для кроссплатформенного логгирования в файл и в системный лог (syslog / Windows Event Log). Поддерживает ротацию по размеру, фильтрацию по уровню и работает без прав администратора.

---

## 📦 Возможности

- ✅ Логгирование в **файл** с **ротацией по размеру**
- ✅ Логгирование в **системный лог ОС** (Linux: `syslog`, Windows: `Event Log`)
- ✅ Поддержка уровней: `TRACE`, `DEBUG`, `INFO`, `WARNING`, `ERROR`, `FATAL`
- ✅ Автоматическая **ротация файлов** (например, `app.log` → `app.log.1`)
- ✅ При ротации — в новый файл добавляется строка:  
  `[ROTATION] Logger restarted — MyApp v1.0.0`
- ✅ **Фильтрация по уровню**: можно установить порог (например, `WARNING`), и более слабые сообщения не будут записываться
- ✅ Потокобезопасность: можно использовать из нескольких потоков
- ✅ Нет зависимости от `stderr`: если запись невозможна — сообщение теряется

---

## 🧩 Основные понятия

### 1. `Logger` — экземпляр логгера

Вы можете создать один или несколько экземпляров `Logger`, каждый из которых может:
- Писать в свой файл
- Использовать свой источник в системном логе

```rust
let logger = Logger::file_and_system(
    "MyApp",           // имя приложения (для системного лога)
    "logs",            // директория
    "app.log",         // имя файла
    1024 * 1024,       // максимальный размер: 1 МБ
    3,                 // максимум 3 файла (включая текущий)
)?;
```
### 2. 2. Макросы: debug!, warning!, error! и др.
Эти макросы принимают экземпляр Logger и пишут сообщение в файл:
```rust
debug!(logger, "User {} logged in", user_id);
error!(logger, "Failed to connect to database: {}", error);
```
⚠️ Эти макросы пишут только в файл, не в системный лог. 
### 3. Глобальные макросы: gdebug!, gwarning!, gerror! и др.
Эти макросы не требуют передачи Logger — они используют глобальный логгер, который нужно инициализировать один раз:
```rust
// Инициализация (один раз)
init_global_logger_file_and_system("MyApp", "logs", "app.log", 1e6 as u64, 3)?;

// Где угодно в коде
gdebug!("Application started");
gerror!("Something went wrong");
```
### 4. platform_log — запись в системный лог
Если нужно записать в системный лог ОС (не в файл), используйте:
```rust
let _ = logger.platform_log(LogLevel::Warning, "High memory usage detected");
```
### 6. Уровни логгирования
```rust
LogLevel::Trace,   // Детальные отладочные сообщения
LogLevel::Debug,   // Отладочная информация
LogLevel::Info,    // Обычные информационные сообщения
LogLevel::Warning, // Предупреждения
LogLevel::Error,   // Ошибки
LogLevel::Fatal,   // Фатальные ошибки (программа завершится)
```

### 7. Фильтрация по уровню
Можно установить минимальный уровень, ниже которого сообщения не будут записываться:
```rust
// Глобально
set_global_log_level(LogLevel::Warning);

// Или через экземпляр
logger.set_log_level(LogLevel::Info);
```

### 8. Ротация логов
Когда файл достигает max_size_bytes — он переименовывается в app.log.1
Старые файлы сдвигаются: .1 → .2, .2 → .3
Хранится до max_files файлов
При ротации в новый файл автоматически добавляется строка:

```log
[2025-04-05
14:30:22.123] DEBUG PID:12345 TID:{1} [ROTATION] Logger restarted — MyApp v1.0.0
```
⚠️ Минимальный max_size — 256 байт (чтобы вместить заголовок). 
### 🧪 Пример использования^
```rust
use loglib::{Logger, debug, warning, error, set_global_log_level};

fn main() {
    // 1. Создаём логгер
    let logger = Logger::file_and_system(
        "example_app",
        "logs",
        "app.log",
        1024 * 1024,
        3,
    ).expect("Failed to create logger");

    // 2. Устанавливаем уровень
    set_global_log_level(loglib::LogLevel::Debug);

    // 3. Пишем в файл
    debug!(logger, "Service started");
    warning!(logger, "Configuration uses default values");

    // 4. Пишем в системный лог
    let _ = logger.platform_log(loglib::LogLevel::Info, "Startup completed");

    // 5. Глобальные макросы (если инициализирован глобальный логгер)
    // init_global_logger_file_only("logs", "global.log", 1e6 as u64, 3)?;
    // gdebug!("This goes to global logger");
}
```
См. также примеры в директории loglib_examples
