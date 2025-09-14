mod logger;
pub use logger::{init, LoggerOptions};
pub use tracing::{debug, error, info, trace, warn,  span, info_span,  event, Level};
pub use tracing_shared::{setup_shared_logger_ref as setup_shared_logger, SharedLogger};
