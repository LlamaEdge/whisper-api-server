use crate::error::LlamaCoreError;
use serde::{Deserialize, Serialize};
use wasmedge_wasi_nn::{Graph as WasiNnGraph, GraphExecutionContext, TensorType};

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogLevel {
    /// Describes messages about the values of variables and the flow of
    /// control within a program.
    Trace,

    /// Describes messages likely to be of interest to someone debugging a
    /// program.
    Debug,

    /// Describes messages likely to be of interest to someone monitoring a
    /// program.
    Info,

    /// Describes messages indicating hazardous situations.
    Warn,

    /// Describes messages indicating serious errors.
    Error,

    /// Describes messages indicating fatal errors.
    Critical,
}
impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => log::LevelFilter::Trace,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Critical => log::LevelFilter::Error,
        }
    }
}
impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
            LogLevel::Critical => write!(f, "critical"),
        }
    }
}
impl std::str::FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            "critical" => Ok(LogLevel::Critical),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

/// Wrapper of the `wasmedge_wasi_nn::Graph` struct
#[derive(Debug)]
pub(crate) struct Graph {
    _graph: WasiNnGraph,
    context: GraphExecutionContext,
}
impl Graph {
    /// Create a new computation graph from the given metadata.
    pub(crate) fn new(metadata: &Metadata) -> Result<Self, LlamaCoreError> {
        // load the model
        let graph = wasmedge_wasi_nn::GraphBuilder::new(
            wasmedge_wasi_nn::GraphEncoding::Ggml,
            wasmedge_wasi_nn::ExecutionTarget::AUTO,
        )
        .build_from_cache(&metadata.model_alias)
        .map_err(|e| {
            let err_msg = e.to_string();

            error!(target: "api-server", "{}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })?;

        // initialize the execution context
        let context = graph.init_execution_context().map_err(|e| {
            let err_msg = e.to_string();

            error!(target: "api-server", "{}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })?;

        Ok(Self {
            _graph: graph,
            context,
        })
    }

    /// Set input uses the data, not only [u8](https://doc.rust-lang.org/nightly/std/primitive.u8.html), but also [f32](https://doc.rust-lang.org/nightly/std/primitive.f32.html), [i32](https://doc.rust-lang.org/nightly/std/primitive.i32.html), etc.
    pub(crate) fn set_input<T: Sized>(
        &mut self,
        index: usize,
        tensor_type: TensorType,
        dimensions: &[usize],
        data: impl AsRef<[T]>,
    ) -> Result<(), LlamaCoreError> {
        self.context
            .set_input(index, tensor_type, dimensions, data)
            .map_err(|e| {
                let err_msg = e.to_string();

                error!(target: "api-server", "{}", &err_msg);

                LlamaCoreError::Operation(err_msg)
            })
    }

    /// Compute the inference on the given inputs.
    pub(crate) fn compute(&mut self) -> Result<(), LlamaCoreError> {
        self.context.compute().map_err(|e| {
            let err_msg = e.to_string();

            error!(target: "api-server", "{}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })
    }

    /// Copy output tensor to out_buffer, return the output’s **size in bytes**.
    pub(crate) fn get_output<T: Sized>(
        &self,
        index: usize,
        out_buffer: &mut [T],
    ) -> Result<usize, LlamaCoreError> {
        self.context.get_output(index, out_buffer).map_err(|e| {
            let err_msg = e.to_string();

            error!(target: "api-server", "{}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })
    }
}

/// Model metadata
#[derive(Debug, Clone)]
pub(crate) struct Metadata {
    pub model_alias: String,
}
