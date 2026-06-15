use std::{
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::Local;
use tracing_subscriber::{
    filter::LevelFilter, fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt,
};

pub fn init(log_dir: &Path) -> Result<(), LoggingError> {
    fs::create_dir_all(log_dir).map_err(|source| LoggingError::Io {
        path: log_dir.to_path_buf(),
        source,
    })?;

    let writer = DailyLogWriter {
        log_dir: Arc::new(log_dir.to_path_buf()),
    };
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(writer);

    tracing_subscriber::registry()
        .with(LevelFilter::INFO)
        .with(file_layer)
        .try_init()
        .map_err(LoggingError::Subscriber)?;

    install_panic_hook();

    Ok(())
}

// Routes panics from every thread through the daily log file via tracing. The
// previous hook is chained so the default stderr output is preserved.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let thread = std::thread::current();
        tracing::error!(
            target: "panic",
            thread = thread.name().unwrap_or("<unnamed>"),
            message = %info,
            backtrace = %backtrace,
            "process panic captured",
        );
        default_hook(info);
    }));
}

#[derive(Debug)]
pub enum LoggingError {
    Io { path: PathBuf, source: io::Error },
    Subscriber(tracing_subscriber::util::TryInitError),
}

impl fmt::Display for LoggingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to prepare log directory {}: {}",
                    path.display(),
                    source
                )
            }
            Self::Subscriber(source) => write!(formatter, "failed to initialize tracing: {source}"),
        }
    }
}

impl std::error::Error for LoggingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Subscriber(source) => Some(source),
        }
    }
}

#[derive(Clone)]
struct DailyLogWriter {
    log_dir: Arc<PathBuf>,
}

impl<'writer> MakeWriter<'writer> for DailyLogWriter {
    type Writer = DailyLogFile;

    fn make_writer(&'writer self) -> Self::Writer {
        DailyLogFile {
            file: None,
            log_dir: Arc::clone(&self.log_dir),
        }
    }
}

struct DailyLogFile {
    file: Option<File>,
    log_dir: Arc<PathBuf>,
}

impl DailyLogFile {
    fn file(&mut self) -> io::Result<&mut File> {
        if self.file.is_none() {
            let path = current_log_file(&self.log_dir);
            self.file = Some(OpenOptions::new().append(true).create(true).open(path)?);
        }

        match self.file.as_mut() {
            Some(file) => Ok(file),
            None => unreachable!("log file is opened before returning"),
        }
    }
}

impl Write for DailyLogFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.file()?.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file()?.flush()
    }
}

fn current_log_file(log_dir: &Path) -> PathBuf {
    log_dir.join(format!("foco-{}.log", Local::now().format("%Y-%m-%d")))
}
