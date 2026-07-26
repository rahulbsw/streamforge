use anyhow::Result;
use k8s_openapi::api::{
    apps::v1::{Deployment, DeploymentSpec},
    core::v1::{
        Capabilities, ConfigMap, ConfigMapVolumeSource, Container, EnvVar, KeyToPath,
        PersistentVolumeClaimVolumeSource, PodSpec, PodTemplateSpec,
        ResourceRequirements as K8sResourceRequirements, SeccompProfile, SecretVolumeSource,
        SecurityContext, Volume, VolumeMount,
    },
};
use k8s_openapi::apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector};
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams},
    runtime::controller::Action,
    Client, ResourceExt,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

use crate::crd::{StreamforgePipeline, UdfArtifactRef};
use crate::render::{self, UDF_MODULE_ROOT};

const UDF_DIGEST_ANNOTATION: &str = "streamforge.io/udf-digests";
const CONFIG_DIGEST_ANNOTATION: &str = "streamforge.io/config-sha256";

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

        // Generate YAML config
        let config_yaml = render::generate_config_yaml(pipeline)?;

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
        let deployment = Self::build_deployment(pipeline, namespace, name, labels)?;

        let deploy_api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
        let patch_params = PatchParams::apply("streamforge-operator");
        deploy_api
            .patch(name, &patch_params, &Patch::Apply(&deployment))
            .await?;

        debug!("Deployment reconciled: {}", name);
        Ok(())
    }

    pub(crate) fn build_deployment(
        pipeline: &StreamforgePipeline,
        namespace: &str,
        name: &str,
        labels: &BTreeMap<String, String>,
    ) -> Result<Deployment, Error> {
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

        // Volume mounts - Start with config
        let mut volume_mounts = vec![VolumeMount {
            name: "config".to_string(),
            mount_path: "/etc/streamforge".to_string(),
            read_only: Some(true),
            ..Default::default()
        }];

        // Volumes - Start with config
        let mut volumes = vec![Volume {
            name: "config".to_string(),
            config_map: Some(k8s_openapi::api::core::v1::ConfigMapVolumeSource {
                default_mode: None,
                items: None,
                name: config_name.to_string(),
                optional: None,
            }),
            ..Default::default()
        }];

        // Add secret volumes and mounts
        Self::add_secret_volumes(pipeline, &mut volumes, &mut volume_mounts);
        Self::add_udf_volumes(pipeline, &mut volumes, &mut volume_mounts)?;

        // Container
        let container = Container {
            name: "streamforge".to_string(),
            image: Some(image),
            image_pull_policy: Some(spec.image.pull_policy.clone()),
            env: Some(env_vars),
            volume_mounts: Some(volume_mounts),
            resources: Some(Self::get_resources(&spec.resources)),
            security_context: Some(SecurityContext {
                allow_privilege_escalation: Some(false),
                capabilities: Some(Capabilities {
                    add: None,
                    drop: Some(vec!["ALL".to_string()]),
                }),
                privileged: Some(false),
                read_only_root_filesystem: Some(true),
                run_as_non_root: Some(true),
                seccomp_profile: Some(SeccompProfile {
                    localhost_profile: None,
                    type_: "RuntimeDefault".to_string(),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let annotations = Some(Self::rollout_annotations(pipeline)?);

        // Pod template
        let pod_template = PodTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: Some(labels.clone()),
                annotations,
                ..Default::default()
            }),
            spec: Some(PodSpec {
                containers: vec![container],
                volumes: Some(volumes),
                automount_service_account_token: Some(false),
                service_account_name: spec.service_account.clone(),
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

        Ok(deployment)
    }

    fn get_resources(resources: &crate::crd::ResourceRequirements) -> K8sResourceRequirements {
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

    /// Add secret volumes and mounts for SSL/SASL credentials
    /// Secrets are mounted with source/destination prefixes to avoid conflicts
    fn add_secret_volumes(
        pipeline: &StreamforgePipeline,
        volumes: &mut Vec<Volume>,
        volume_mounts: &mut Vec<VolumeMount>,
    ) {
        let mut secret_names = BTreeSet::new();

        // Collect secrets from source security config
        if let Some(security) = &pipeline.spec.source.security {
            if let Some(ssl) = &security.ssl {
                Self::collect_ssl_secrets(ssl, &mut secret_names, "source");
            }
            if let Some(sasl) = &security.sasl {
                Self::collect_sasl_secrets(sasl, &mut secret_names, "source");
            }
        }

        // Collect secrets from destination security configs
        for (idx, dest) in pipeline.spec.destinations.iter().enumerate() {
            let prefix = format!("destination-{}", idx);
            if let Some(security) = &dest.security {
                if let Some(ssl) = &security.ssl {
                    Self::collect_ssl_secrets(ssl, &mut secret_names, &prefix);
                }
                if let Some(sasl) = &security.sasl {
                    Self::collect_sasl_secrets(sasl, &mut secret_names, &prefix);
                }
            }
        }

        // Create volumes and mounts for each unique secret with role prefix
        for (idx, (role, secret_name)) in secret_names.iter().enumerate() {
            let volume_name = format!("secret-{}", idx);
            let mount_path = format!("/etc/streamforge/secrets/{}/{}", role, secret_name);

            volumes.push(Volume {
                name: volume_name.clone(),
                secret: Some(SecretVolumeSource {
                    secret_name: Some(secret_name.clone()),
                    default_mode: Some(0o400), // Read-only for owner
                    optional: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            });

            volume_mounts.push(VolumeMount {
                name: volume_name,
                mount_path,
                read_only: Some(true),
                ..Default::default()
            });
        }
    }

    fn collect_ssl_secrets(
        ssl: &crate::crd::SslConfig,
        secrets: &mut BTreeSet<(String, String)>,
        role: &str,
    ) {
        if let Some(ref secret_ref) = ssl.ca_secret {
            secrets.insert((role.to_string(), secret_ref.name.clone()));
        }
        if let Some(ref secret_ref) = ssl.certificate_secret {
            secrets.insert((role.to_string(), secret_ref.name.clone()));
        }
        if let Some(ref secret_ref) = ssl.key_secret {
            secrets.insert((role.to_string(), secret_ref.name.clone()));
        }
        if let Some(ref secret_ref) = ssl.key_password_secret {
            secrets.insert((role.to_string(), secret_ref.name.clone()));
        }
    }

    fn collect_sasl_secrets(
        sasl: &crate::crd::SaslConfig,
        secrets: &mut BTreeSet<(String, String)>,
        role: &str,
    ) {
        if let Some(ref secret_ref) = sasl.username_secret {
            secrets.insert((role.to_string(), secret_ref.name.clone()));
        }
        if let Some(ref secret_ref) = sasl.password_secret {
            secrets.insert((role.to_string(), secret_ref.name.clone()));
        }
        if let Some(ref secret_ref) = sasl.keytab_secret {
            secrets.insert((role.to_string(), secret_ref.name.clone()));
        }
    }

    fn add_udf_volumes(
        pipeline: &StreamforgePipeline,
        volumes: &mut Vec<Volume>,
        volume_mounts: &mut Vec<VolumeMount>,
    ) -> Result<(), Error> {
        let Some(udfs) = &pipeline.spec.udfs else {
            return Ok(());
        };

        let mut modules = udfs.modules.iter().collect::<Vec<_>>();
        modules.sort_by(|left, right| left.name.cmp(&right.name));

        for module in modules {
            let volume_name = format!("udf-{}", module.name);
            let mount_path = format!("{UDF_MODULE_ROOT}/{}", module.name);
            let volume = match &module.artifact {
                UdfArtifactRef::ConfigMap { name, key } => Volume {
                    name: volume_name.clone(),
                    config_map: Some(ConfigMapVolumeSource {
                        default_mode: Some(0o444),
                        items: Some(vec![KeyToPath {
                            key: key.clone(),
                            mode: Some(0o444),
                            path: "module.wasm".to_string(),
                        }]),
                        name: name.clone(),
                        optional: Some(false),
                    }),
                    ..Default::default()
                },
                UdfArtifactRef::PersistentVolumeClaim { claim_name, .. } => Volume {
                    name: volume_name.clone(),
                    persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                        claim_name: claim_name.clone(),
                        read_only: Some(true),
                    }),
                    ..Default::default()
                },
            };

            volumes.push(volume);
            volume_mounts.push(VolumeMount {
                name: volume_name,
                mount_path,
                read_only: Some(true),
                ..Default::default()
            });
        }

        Ok(())
    }

    fn udf_digest_annotation(pipeline: &StreamforgePipeline) -> Option<BTreeMap<String, String>> {
        let udfs = pipeline.spec.udfs.as_ref()?;
        let mut modules = udfs.modules.iter().collect::<Vec<_>>();
        modules.sort_by(|left, right| left.name.cmp(&right.name));
        let digest_set = modules
            .iter()
            .map(|module| format!("{}={}", module.name, module.sha256))
            .collect::<Vec<_>>()
            .join(",");
        let mut annotations = BTreeMap::new();
        annotations.insert(UDF_DIGEST_ANNOTATION.to_string(), digest_set);
        Some(annotations)
    }

    fn rollout_annotations(
        pipeline: &StreamforgePipeline,
    ) -> Result<BTreeMap<String, String>, Error> {
        let mut annotations = Self::udf_digest_annotation(pipeline).unwrap_or_default();
        let config = render::generate_config_yaml(pipeline)?;
        annotations.insert(
            CONFIG_DIGEST_ANNOTATION.to_string(),
            format!("{:x}", Sha256::digest(config.as_bytes())),
        );
        Ok(annotations)
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
