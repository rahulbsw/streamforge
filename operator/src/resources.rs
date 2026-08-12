use std::collections::{BTreeMap, BTreeSet};

use k8s_openapi::api::{
    apps::v1::{Deployment, DeploymentSpec},
    core::v1::{
        Capabilities, ConfigMap, ConfigMapVolumeSource, Container, ContainerPort, EnvVar,
        HTTPGetAction, KeyToPath, PersistentVolumeClaimVolumeSource, PodSecurityContext, PodSpec,
        PodTemplateSpec, Probe, ResourceRequirements as K8sResourceRequirements, SeccompProfile,
        SecretVolumeSource, SecurityContext, Volume, VolumeMount,
    },
};
use k8s_openapi::apimachinery::pkg::{
    api::resource::Quantity,
    apis::meta::v1::{LabelSelector, OwnerReference},
    util::intstr::IntOrString,
};
use kube::{api::ObjectMeta, Resource};
use sha2::{Digest, Sha256};

use crate::crd::{StreamforgePipeline, UdfArtifactRef};
use crate::reconciler::Error;
use crate::render::{self, UDF_MODULE_ROOT};

const UDF_DIGEST_ANNOTATION: &str = "streamforge.io/udf-digests";
const CONFIG_DIGEST_ANNOTATION: &str = "streamforge.io/config-sha256";

pub(crate) fn build_configmap(
    pipeline: &StreamforgePipeline,
    namespace: &str,
    name: &str,
    labels: &BTreeMap<String, String>,
) -> Result<ConfigMap, Error> {
    let mut data = BTreeMap::new();
    data.insert(
        "config.yaml".to_string(),
        render::generate_config_yaml(pipeline)?,
    );

    Ok(ConfigMap {
        metadata: ObjectMeta {
            name: Some(format!("{}-config", name)),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            owner_references: controller_owner_references(pipeline),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    })
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
        EnvVar {
            name: "STREAMFORGE_LOG_FORMAT".to_string(),
            value: Some("json".to_string()),
            ..Default::default()
        },
    ];

    let mut volume_mounts = vec![VolumeMount {
        name: "config".to_string(),
        mount_path: "/etc/streamforge".to_string(),
        read_only: Some(true),
        ..Default::default()
    }];
    let mut volumes = vec![Volume {
        name: "config".to_string(),
        config_map: Some(ConfigMapVolumeSource {
            name: config_name,
            ..Default::default()
        }),
        ..Default::default()
    }];

    add_secret_volumes(pipeline, &mut volumes, &mut volume_mounts);
    add_udf_volumes(pipeline, &mut volumes, &mut volume_mounts)?;

    let container = Container {
        name: "streamforge".to_string(),
        image: Some(image),
        image_pull_policy: Some(spec.image.pull_policy.clone()),
        env: Some(env_vars),
        ports: Some(vec![ContainerPort {
            name: Some("metrics".to_string()),
            container_port: 9090,
            protocol: Some("TCP".to_string()),
            ..Default::default()
        }]),
        readiness_probe: Some(http_probe("/ready")),
        liveness_probe: Some(http_probe("/health")),
        volume_mounts: Some(volume_mounts),
        resources: Some(get_resources(&spec.resources)),
        security_context: Some(SecurityContext {
            allow_privilege_escalation: Some(false),
            capabilities: Some(Capabilities {
                add: None,
                drop: Some(vec!["ALL".to_string()]),
            }),
            privileged: Some(false),
            read_only_root_filesystem: Some(true),
            run_as_non_root: Some(true),
            run_as_user: Some(65_532),
            run_as_group: Some(65_532),
            seccomp_profile: Some(SeccompProfile {
                localhost_profile: None,
                type_: "RuntimeDefault".to_string(),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let pod_template = PodTemplateSpec {
        metadata: Some(ObjectMeta {
            labels: Some(labels.clone()),
            annotations: Some(rollout_annotations(pipeline)?),
            ..Default::default()
        }),
        spec: Some(PodSpec {
            containers: vec![container],
            volumes: Some(volumes),
            automount_service_account_token: Some(false),
            security_context: Some(PodSecurityContext {
                fs_group: Some(65_532),
                run_as_non_root: Some(true),
                run_as_user: Some(65_532),
                run_as_group: Some(65_532),
                ..Default::default()
            }),
            service_account_name: spec.service_account.clone(),
            node_selector: if spec.node_selector.is_empty() {
                None
            } else {
                Some(spec.node_selector.clone())
            },
            ..Default::default()
        }),
    };

    Ok(Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            owner_references: controller_owner_references(pipeline),
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
    })
}

fn controller_owner_references(pipeline: &StreamforgePipeline) -> Option<Vec<OwnerReference>> {
    pipeline.controller_owner_ref(&()).map(|owner| vec![owner])
}

fn get_resources(resources: &crate::crd::ResourceRequirements) -> K8sResourceRequirements {
    K8sResourceRequirements {
        requests: resources.requests.as_ref().map(|requests| {
            requests
                .iter()
                .map(|(key, value)| (key.clone(), Quantity(value.clone())))
                .collect()
        }),
        limits: resources.limits.as_ref().map(|limits| {
            limits
                .iter()
                .map(|(key, value)| (key.clone(), Quantity(value.clone())))
                .collect()
        }),
        ..Default::default()
    }
}

fn add_secret_volumes(
    pipeline: &StreamforgePipeline,
    volumes: &mut Vec<Volume>,
    volume_mounts: &mut Vec<VolumeMount>,
) {
    let mut secret_names = BTreeSet::new();

    if let Some(security) = &pipeline.spec.source.security {
        if let Some(ssl) = &security.ssl {
            collect_ssl_secrets(ssl, &mut secret_names, "source");
        }
        if let Some(sasl) = &security.sasl {
            collect_sasl_secrets(sasl, &mut secret_names, "source");
        }
    }

    if let Some(dest) = pipeline.spec.destinations.first() {
        if let Some(security) = &dest.security {
            if let Some(ssl) = &security.ssl {
                collect_ssl_secrets(ssl, &mut secret_names, "destination-0");
            }
            if let Some(sasl) = &security.sasl {
                collect_sasl_secrets(sasl, &mut secret_names, "destination-0");
            }
        }
    }

    for (index, (role, secret_name)) in secret_names.iter().enumerate() {
        let volume_name = format!("secret-{}", index);
        let mount_path = format!("/etc/streamforge/secrets/{}/{}", role, secret_name);

        volumes.push(Volume {
            name: volume_name.clone(),
            secret: Some(SecretVolumeSource {
                secret_name: Some(secret_name.clone()),
                default_mode: Some(0o440),
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
    for secret in [
        &ssl.ca_secret,
        &ssl.certificate_secret,
        &ssl.key_secret,
        &ssl.key_password_secret,
    ]
    .into_iter()
    .flatten()
    {
        secrets.insert((role.to_string(), secret.name.clone()));
    }
}

fn collect_sasl_secrets(
    sasl: &crate::crd::SaslConfig,
    secrets: &mut BTreeSet<(String, String)>,
    role: &str,
) {
    for secret in [
        &sasl.username_secret,
        &sasl.password_secret,
        &sasl.keytab_secret,
    ]
    .into_iter()
    .flatten()
    {
        secrets.insert((role.to_string(), secret.name.clone()));
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

fn rollout_annotations(pipeline: &StreamforgePipeline) -> Result<BTreeMap<String, String>, Error> {
    let mut annotations = udf_digest_annotation(pipeline).unwrap_or_default();
    let config = render::generate_config_yaml(pipeline)?;
    annotations.insert(
        CONFIG_DIGEST_ANNOTATION.to_string(),
        format!("{:x}", Sha256::digest(config.as_bytes())),
    );
    Ok(annotations)
}

fn http_probe(path: &str) -> Probe {
    Probe {
        http_get: Some(HTTPGetAction {
            path: Some(path.to_string()),
            port: IntOrString::Int(9090),
            scheme: Some("HTTP".to_string()),
            ..Default::default()
        }),
        initial_delay_seconds: Some(5),
        period_seconds: Some(10),
        timeout_seconds: Some(2),
        failure_threshold: Some(3),
        ..Default::default()
    }
}
