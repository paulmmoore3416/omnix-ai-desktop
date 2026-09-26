//! Host inspection and control: metrics, GPUs, history, processes,
//! services, containers, cleanup and optimisation.
pub mod advisor;
pub mod cleanup;
pub mod docker;
pub mod gpu;
pub mod history;
pub mod metrics;
pub mod probe;
pub mod processes;
pub mod services;
pub mod snapshot;
