use chrono_tz::Asia::Shanghai;
use owo_colors::OwoColorize;
use std::fmt;
use std::sync::OnceLock;
use tracing::Subscriber;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::reload::{self, Handle};
use tracing_subscriber::{
    Layer, Registry,
    filter::LevelFilter,
    fmt::{FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
};

static RELOAD_HANDLE: OnceLock<Handle<LevelFilter, Registry>> = OnceLock::new();
static INIT_LOGGER: OnceLock<()> = OnceLock::new();
pub struct LoggerOptions {
    /// 日志等级
    pub level: String,
    /// 是否启用文件日志记录
    pub enable_file_logging: bool,
    /// 自定义前缀
    pub prefix: Option<String>,
    /// 日志文件保存路径
    pub log_directory: Option<String>,
    /// 日志文件保留天数
    pub retention_days: Option<u8>,
}

impl LoggerOptions {
    /// 创建新的日志配置选项
    pub fn new(level: &str) -> Self {
        Self {
            level: level.to_string(),
            enable_file_logging: false,
            prefix: None,
            log_directory: None,
            retention_days: None,
        }
    }
    /// 设置是否启用文件日志记录
    pub fn with_file_logging(mut self, enable: bool) -> Self {
        self.enable_file_logging = enable;
        self
    }

    /// 设置自定义前缀
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// 设置日志文件保存目录
    pub fn with_log_directory(mut self, directory: String) -> Self {
        self.log_directory = Some(directory);
        self
    }
    /// 设置日志文件保留天数
    pub fn with_retention_days(mut self, days: u8) -> Self {
        self.retention_days = Some(days);
        self
    }
}

struct Formatter {
    prefix: String,
    color: bool,
}

impl<S, N> FormatEvent<S, N> for Formatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        let prefix = if self.color {
            format!("[{}]", self.prefix.magenta())
        } else {
            format!("[{}]", self.prefix)
        };
        write!(writer, "{} ", prefix)?;

        let local_time = chrono::Local::now();
        let shanghai_time = local_time.with_timezone(&Shanghai);
        let formatted_time = shanghai_time.format("%H:%M:%S%.3f");
        write!(writer, "[{}] ", formatted_time)?;

        let logger_level = event.metadata().level();
        if self.color {
            let colored_level = match *logger_level {
                tracing::Level::ERROR => logger_level.red().to_string(),
                tracing::Level::WARN => logger_level.yellow().to_string(),
                tracing::Level::INFO => logger_level.green().to_string(),
                tracing::Level::DEBUG => logger_level.blue().to_string(),
                tracing::Level::TRACE => logger_level.magenta().to_string(),
            };
            write!(writer, "[{: <17}] ", colored_level)?;
        } else {
            write!(writer, "[{: <7}] ", logger_level)?;
        }

        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

pub fn log_init(options: Option<LoggerOptions>) {

    if INIT_LOGGER.get().is_some() {
        return;
    }

    let options = options.unwrap_or_else(|| LoggerOptions::new("info"));

    let logger_level = parse_log_level(&options.level);
    let prefix = options.prefix.unwrap_or_else(|| "PuniYu".to_string());

    let (filter, reload_handle) = reload::Layer::new(logger_level);
    if RELOAD_HANDLE.set(reload_handle).is_err() {
        return;
    }
    INIT_LOGGER.set(()).unwrap();

    let console_subscriber = tracing_subscriber::fmt::layer()
        .event_format(Formatter { prefix: prefix.clone(), color: true })
        .with_filter(filter);

    let mut layers = vec![console_subscriber.boxed()];

    if options.enable_file_logging {
        let log_dir = options.log_directory.unwrap_or_else(|| "logs".to_string());
        let _ = std::fs::create_dir_all(&log_dir);
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("puniyu")
            .filename_suffix("log")
            .max_log_files(options.retention_days.unwrap_or(7) as usize)
            .build(&log_dir)
            .unwrap();

        let file_subscriber = tracing_subscriber::fmt::layer()
            .event_format(Formatter { prefix, color: false })
            .with_writer(file_appender)
            .with_ansi(false)
            .with_filter(logger_level);

        layers.push(file_subscriber.boxed());
    }
    let subscriber = tracing_subscriber::registry().with(layers);

    tracing::subscriber::set_global_default(subscriber)
        .unwrap();
}

pub fn set_log_level(level: &str) {
    let logger_level = parse_log_level(level);
    if let Some(handle) = RELOAD_HANDLE.get() {
        handle.modify(|filter| *filter = logger_level).unwrap();
    }
}

fn parse_log_level(level: &str) -> LevelFilter {
    match level.to_lowercase().as_str() {
        "trace" => LevelFilter::TRACE,
        "debug" => LevelFilter::DEBUG,
        "info" => LevelFilter::INFO,
        "warn" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        _ => LevelFilter::INFO,
    }
}
