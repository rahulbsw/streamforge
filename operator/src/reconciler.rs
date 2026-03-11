use anyhow::Result;
use k8s_openapi::api::{
    apps::v1::{Deployment, DeploymentSpec},
    core::v1::{
        ConfigMap, Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec,
        ResourceRequirements as K8sResourceRequirements, Volume, VolumeMount,
    },
};
use k8s_openapi::apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector};
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams, PostParams},
    runtime::controller::Action,
    Client, ResourceExt,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::crd::StreamforgePipeline;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Kubernetes error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

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
        labels.insert("app.kubernetes.io/name".to_string(), "streamforge".to_string());
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

        // Generate YAML config
        let config_yaml = self.generate_config_yaml(pipeline)?;

        let mut data = BTreeMap::new();
        data.insert("config.yaml".to_string(), config_yaml);

        let configmap = ConfigMap {
            metadata: ObjectMeta {
                name: Some(config_name.clone()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        };

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
        let spec = &pipeline.spec;
        let image = format!("{}:{}", spec.image.repository, spec.image.tag);
        let config_name = format!("{}-config", name);

        // Environment variables
        let env_vars = vec![
            EnvVar {
                name: "CONFIG_FILE".to_string(),
                value: Some("/etc/streamforge/config.yaml".to_string()),
                ..Default::default()
            },
            EnvVar {
                name: "RUST_LOG".to_string(),
                value: Some(spec.log_level.clone()),
                ..Default::default()
            },
        ];

        // Volume mounts
        let volume_mounts = vec![VolumeMount {
            name: "config".to_string(),
            mount_path: "/etc/streamforge".to_string(),
            read_only: Some(true),
            ..Default::default()
        }];

        // Volumes
        let volumes = vec![Volume {
            name: "config".to_string(),
            config_map: Some(k8s_openapi::api::core::v1::ConfigMapVolumeSource {
                name: Some(config_name),
                ..Default::default()
            }),
            ..Default::default()
        }];

        // Container
        let container = Container {
            name: "streamforge".to_string(),
            image: Some(image),
            image_pull_policy: Some(spec.image.pull_policy.clone()),
            env: Some(env_vars),
            volume_mounts: Some(volume_mounts),
            resources: Some(self.get_resources(&spec.resources)),
            ..Default::default()
        };

        // Pod template
        let pod_template = PodTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: Some(labels.clone()),
                ..Default::default()
            }),
            spec: Some(PodSpec {
                containers: vec![container],
                volumes: Some(volumes),
                service_account: spec.service_account.clone(),
                node_selector: if spec.node_selector.is_empty() {
                    None
                } else {
                    Some(spec.node_selector.clone())
                },
                ..Default::default()
            }),
        };

        // Deployment
        let deployment = Deployment {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(spec.replicas),
                selector: LabelSelector {
                    match_labels: Some(labels.clone()),
                    ..Default::default()
                },
                template: pod_template,
                ..Default::default()
            }),
            ..Default::default()
        };

        let deploy_api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
        let patch_params = PatchParams::apply("streamforge-operator");
        deploy_api
            .patch(name, &patch_params, &Patch::Apply(&deployment))
            .await?;

        debug!("Deployment reconciled: {}", name);
        Ok(())
    }

    fn get_resources(
        &self,
        resources: &crate::crd::ResourceRequirements,
    ) -> K8sResourceRequirements {
        K8sResourceRequirements {
            requests: resources.requests.as_ref().map(|r| {
                r.iter()
                    .map(|(k, v)| (k.clone(), Quantity(v.clone())))
                    .collect()
            }),
            limits: resources.limits.as_ref().map(|l| {
                l.iter()
                    .map(|(k, v)| (k.clone(), Quantity(v.clone())))
                    .collect()
            }),
            ..Default::default()
        }
    }

    fn generate_config_yaml(&self, pipeline: &StreamforgePipeline) -> Result<String, Error> {
        let spec = &pipeline.spec;

        // Build config structure matching streamforge's config format
        let config = serde_json::json!({
            "appid": spec.appid.clone().unwrap_or_else(|| pipeline.name_any()),
            "source": {
                "brokers": spec.source.brokers,
                "topic": spec.source.topic,
                "group_id": spec.source.group_id.clone().unwrap_or_else(|| {
                    format!("streamforge-{}", pipeline.name_any())
                }),
                "offset": spec.source.offset,
                "security": spec.source.security.as_ref().map(|s| serde_json::json!({
                    "protocol": s.protocol,
                    "ssl": s.ssl,
                    "sasl": s.sasl,
                })),
            },
            "destinations": spec.destinations.iter().map(|d| {
                serde_json::json!({
                    "brokers": d.brokers,
                    "topic": d.topic,
                    "filter": d.filter,
                    "transform": d.transform,
                    "partitioner": d.partitioner,
                    "partitioner_field": d.partitioner_field,
                    "compression": d.compression,
                    "security": d.security.as_ref().map(|s| serde_json::json!({
                        "protocol": s.protocol,
                        "ssl": s.ssl,
                        "sasl": s.sasl,
                    })),
                })
            }).collect::<Vec<_>>(),
            "threads": spec.threads,
        });

        serde_yaml::to_string(&config).map_err(|e| {
            Error::InvalidSpec(format!("Failed to serialize config: {}", e))
        })
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
        let status = deployment.status.unwrap_or_default();

        let ready_replicas = status.ready_replicas.unwrap_or(0);
        let phase = if ready_replicas == pipeline.spec.replicas {
            "Running"
        } else if ready_replicas > 0 {
            "Pending"
        } else {
            "Failed"
        };

        // Update CRD status
        let pipeline_api: Api<StreamforgePipeline> =
            Api::namespaced(self.client.clone(), namespace);

        let status_patch = serde_json::json!({
            "status": {
                "phase": phase,
                "replicas": ready_replicas,
                "lastUpdated": chrono::Utc::now().to_rfc3339(),
            }
        });

        let patch_params = PatchParams::default();
        pipeline_api
            .patch_status(&name, &patch_params, &Patch::Merge(&status_patch))
            .await?;

        Ok(())
    }
}
