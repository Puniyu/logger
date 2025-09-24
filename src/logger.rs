use chrono_tz::Asia::Shanghai;
use owo_colors::OwoColorize;
use std::fmt;
use tracing::{field::{Field, Visit}, Subscriber};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    Layer,
    filter::LevelFilter,
    fmt::{FormatEvent, FormatFields},
    layer::SubscriberExt,
    registry::LookupSpan,
};
use convert_case::{Case, Casing};

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
    pub fn new() -> Self {
        Self {
            level: "info".to_string(),
            enable_file_logging: false,
            prefix: None,
            log_directory: None,
            retention_days: None,
        }
    }
    /// 设置日志等级
    pub fn with_level(mut self, level: &str) -> Self {
        self.level = level.to_string();
        self
    }
    /// 设置是否启用文件日志记录
    pub fn with_file_logging(mut self, enable: bool) -> Self {
        self.enable_file_logging = enable;
        self
    }

    /// 设置自定义前缀
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = Some(prefix.to_string());
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

struct MessageVisitor<'a, W> {
    writer: &'a mut W,
    strip_ansi: bool,
}

impl<'a, W> MessageVisitor<'a, W> {
    fn new(writer: &'a mut W, strip_ansi: bool) -> Self {
        Self { writer, strip_ansi }
    }
}

impl<'a, W> Visit for MessageVisitor<'a, W>
where
    W: fmt::Write,
{
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            if self.strip_ansi {
                // 移除 ANSI 转义序列
                let stripped = strip_ansi_escapes::strip(value);
                let clean_value = String::from_utf8_lossy(&stripped);
                write!(self.writer, "{}", clean_value).unwrap();
            } else {
                write!(self.writer, "{}", value).unwrap();
            }
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            if self.strip_ansi {
                let debug_str = format!("{:?}", value);
                let stripped = strip_ansi_escapes::strip(debug_str);
                let clean_value = String::from_utf8_lossy(&stripped);
                write!(self.writer, "{}", clean_value).unwrap();
            } else {
                write!(self.writer, "{:?}", value).unwrap();
            }
        }
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
        _: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
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

        let mut writer_ref = writer.by_ref();
        let mut visitor = MessageVisitor::new(&mut writer_ref, !self.color);
        event.record(&mut visitor);

        writeln!(writer)
    }
}

pub fn init(options: Option<LoggerOptions>) {

    let options = options.unwrap_or_else(|| LoggerOptions::new());

    let logger_level = parse_log_level(&options.level);
    let prefix = options.prefix.as_deref().unwrap_or("puniyu");
    let prefix_str = prefix.to_case(Case::Pascal);
    let console_prefix = prefix.to_case(Case::Snake);

    let console_subscriber = tracing_subscriber::fmt::layer()
        .event_format(Formatter { prefix: prefix_str.to_string(), color: true })
        .with_filter(logger_level);

    let mut layers = vec![console_subscriber.boxed()];

    if options.enable_file_logging {
        let log_dir = options.log_directory.unwrap_or_else(|| "logs".to_string());
        let _ = std::fs::create_dir_all(&log_dir);
        let file_appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(console_prefix)
            .filename_suffix("log")
            .max_log_files(options.retention_days.unwrap_or(7) as usize)
            .build(&log_dir)
            .unwrap();

        let file_subscriber = tracing_subscriber::fmt::layer()
            .event_format(Formatter { prefix: prefix_str.to_string(), color: false })
            .with_writer(file_appender)
            .with_ansi(false)
            .with_filter(logger_level);

        layers.push(file_subscriber.boxed());
    }
    let subscriber = tracing_subscriber::registry().with(layers);

    tracing::subscriber::set_global_default(subscriber).ok();
    tracing_log::LogTracer::init().ok();
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
