use anyhow::Result;
use clap::Parser;
use futures::StreamExt;
use kube::{
    runtime::{controller::Action, watcher, Controller},
    Api, Client, ResourceExt,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

mod crd;
mod reconciler;

use crd::StreamforgePipeline;
use reconciler::PipelineReconciler;

#[derive(Parser, Debug)]
#[clap(author, version, about = "Streamforge Kubernetes Operator")]
struct Args {
    /// Namespace to watch (empty = all namespaces)
    #[clap(short, long, env = "OPERATOR_NAMESPACE", default_value = "")]
    namespace: String,

    /// Reconcile interval in seconds
    #[clap(short, long, env = "RECONCILE_INTERVAL", default_value = "30")]
    interval: u64,

    /// Log level
    #[clap(short, long, env = "RUST_LOG", default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(&args.log_level)
        .json()
        .init();

    info!(
        "Starting Streamforge Operator v{}",
        env!("CARGO_PKG_VERSION")
    );
    info!(
        "Watching namespace: {}",
        if args.namespace.is_empty() {
            "all"
        } else {
            &args.namespace
        }
    );

    // Create Kubernetes client
    let client = Client::try_default().await?;
    info!("Connected to Kubernetes API");

    // Create API client for StreamforgePipeline CRD
    let pipelines = if args.namespace.is_empty() {
        Api::<StreamforgePipeline>::all(client.clone())
    } else {
        Api::<StreamforgePipeline>::namespaced(client.clone(), &args.namespace)
    };

    // Create reconciler
    let reconciler = Arc::new(PipelineReconciler::new(client.clone()));

    info!("Starting controller");

    // Start controller
    Controller::new(pipelines.clone(), watcher::Config::default())
        .shutdown_on_signal()
        .run(
            move |pipeline, ctx| {
                let reconciler = ctx.clone();
                async move { reconciler.reconcile(pipeline).await }
            },
            error_policy,
            reconciler,
        )
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("Reconciled {:?}", o),
                Err(e) => warn!("Reconcile error: {:?}", e),
            }
        })
        .await;

    Ok(())
}

fn error_policy(
    pipeline: Arc<StreamforgePipeline>,
    error: &reconciler::Error,
    _ctx: Arc<PipelineReconciler>,
) -> Action {
    error!(
        "Reconciliation error for pipeline {}: {:?}",
        pipeline.name_any(),
        error
    );
    Action::requeue(Duration::from_secs(60))
}
