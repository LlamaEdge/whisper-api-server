use crate::error::LlamaCoreError;
use wasmedge_wasi_nn::{Graph as WasiNnGraph, GraphExecutionContext, TensorType};

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

            println!("[ERROR] {}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })?;

        // initialize the execution context
        let context = graph.init_execution_context().map_err(|e| {
            let err_msg = e.to_string();

            println!("ERROR: {}", &err_msg);

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

                println!("[ERROR] {}", &err_msg);

                LlamaCoreError::Operation(err_msg)
            })
    }

    /// Compute the inference on the given inputs.
    pub(crate) fn compute(&mut self) -> Result<(), LlamaCoreError> {
        self.context.compute().map_err(|e| {
            let err_msg = e.to_string();

            println!("[ERROR] {}", &err_msg);

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

            println!("[ERROR] {}", &err_msg);

            LlamaCoreError::Operation(err_msg)
        })
    }
}

/// Model metadata
#[derive(Debug, Clone)]
pub(crate) struct Metadata {
    pub model_alias: String,
}
