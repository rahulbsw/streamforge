use anyhow::Result;
use k8s_openapi::api::{apps::v1::Deployment, core::v1::ConfigMap};
use kube::{
    api::{Api, Patch, PatchParams},
    runtime::controller::Action,
    Client, ResourceExt,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

use crate::crd::StreamforgePipeline;
use crate::{render, resources, status};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Kubernetes error: {0}")]
    Kube(#[from] kube::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid spec: {0}")]
    InvalidSpec(String),
}

pub struct PipelineReconciler {
    client: Client,
}

impl PipelineReconciler {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn reconcile(&self, pipeline: Arc<StreamforgePipeline>) -> Result<Action, Error> {
        let namespace = pipeline.namespace().unwrap();
        let name = pipeline.name_any();

        info!("Reconciling pipeline: {}/{}", namespace, name);

        render::validate_pipeline_spec(&pipeline)?;

        // Generate labels
        let labels = self.get_labels(&pipeline);

        // Create or update ConfigMap
        self.reconcile_configmap(&pipeline, &namespace, &name, &labels)
            .await?;

        // Create or update Deployment
        self.reconcile_deployment(&pipeline, &namespace, &name, &labels)
            .await?;

        // Update status
        self.update_status(&pipeline, &namespace).await?;

        info!("Successfully reconciled pipeline: {}/{}", namespace, name);
        Ok(Action::requeue(Duration::from_secs(300)))
    }

    fn get_labels(&self, pipeline: &StreamforgePipeline) -> BTreeMap<String, String> {
        let mut labels = BTreeMap::new();
        labels.insert(
            "app.kubernetes.io/name".to_string(),
            "streamforge".to_string(),
        );
        labels.insert(
            "app.kubernetes.io/instance".to_string(),
            pipeline.name_any(),
        );
        labels.insert(
            "app.kubernetes.io/component".to_string(),
            "pipeline".to_string(),
        );
        labels.insert(
            "app.kubernetes.io/managed-by".to_string(),
            "streamforge-operator".to_string(),
        );
        labels.insert("streamforge.io/pipeline".to_string(), pipeline.name_any());
        labels
    }

    async fn reconcile_configmap(
        &self,
        pipeline: &StreamforgePipeline,
        namespace: &str,
        name: &str,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        let config_name = format!("{}-config", name);
        let configmap = resources::build_configmap(pipeline, namespace, name, labels)?;

        let cm_api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
        let patch_params = PatchParams::apply("streamforge-operator");
        cm_api
            .patch(&config_name, &patch_params, &Patch::Apply(&configmap))
            .await?;

        debug!("ConfigMap reconciled: {}", config_name);
        Ok(())
    }

    async fn reconcile_deployment(
        &self,
        pipeline: &StreamforgePipeline,
        namespace: &str,
        name: &str,
        labels: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        let deployment = resources::build_deployment(pipeline, namespace, name, labels)?;

        let deploy_api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
        let patch_params = PatchParams::apply("streamforge-operator");
        deploy_api
            .patch(name, &patch_params, &Patch::Apply(&deployment))
            .await?;

        debug!("Deployment reconciled: {}", name);
        Ok(())
    }

    async fn update_status(
        &self,
        pipeline: &StreamforgePipeline,
        namespace: &str,
    ) -> Result<(), Error> {
        let name = pipeline.name_any();
        let deploy_api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);

        // Get deployment status
        let deployment = deploy_api.get(&name).await?;
        let desired_status =
            status::desired_status(pipeline, &deployment, chrono::Utc::now().to_rfc3339());

        if pipeline.status.as_ref() == Some(&desired_status) {
            debug!("Pipeline status unchanged: {}", name);
            return Ok(());
        }

        // Update CRD status
        let pipeline_api: Api<StreamforgePipeline> =
            Api::namespaced(self.client.clone(), namespace);

        let status_patch = serde_json::json!({
            "status": desired_status
        });

        let patch_params = PatchParams::default();
        pipeline_api
            .patch_status(&name, &patch_params, &Patch::Merge(&status_patch))
            .await?;

        Ok(())
    }
}
