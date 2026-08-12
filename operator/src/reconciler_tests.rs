use std::collections::BTreeMap;

use crate::crd::StreamforgePipeline;
use crate::render::generate_config_yaml;
use crate::resources::{build_configmap, build_deployment};

fn pipeline_with_udfs() -> StreamforgePipeline {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "streamforge.io/v1alpha1",
        "kind": "StreamforgePipeline",
        "metadata": {"name": "orders"},
        "spec": {
            "source": {"brokers": "source:9092", "topic": "orders"},
            "destinations": [{"brokers": "target:9092", "topic": "accepted"}],
            "udfs": {
                "modules": [
                    {
                        "name": "pvc-module",
                        "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                        "world": "value_transform",
                        "artifact": {
                            "persistentVolumeClaim": {
                                "claimName": "udf-artifacts",
                                "path": "release/transform.wasm"
                            }
                        }
                    },
                    {
                        "name": "config-module",
                        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "world": "filter",
                        "artifact": {
                            "configMap": {"name": "order-udfs", "key": "filter.wasm"}
                        }
                    }
                ]
            }
        }
    }))
    .unwrap()
}

fn secure_pipeline() -> StreamforgePipeline {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "streamforge.io/v1alpha1",
        "kind": "StreamforgePipeline",
        "metadata": {"name": "secure-orders"},
        "spec": {
            "source": {
                "brokers": "source:9093",
                "topic": "orders",
                "security": {
                    "protocol": "SASL_SSL",
                    "ssl": {
                        "caSecret": {"name": "source-ca", "key": "ca.crt"}
                    },
                    "sasl": {
                        "mechanism": "SCRAM-SHA-512",
                        "usernameSecret": {"name": "source-auth", "key": "username"},
                        "passwordSecret": {"name": "source-auth", "key": "password"}
                    }
                }
            },
            "destinations": [{
                "brokers": "target:9093",
                "topic": "accepted",
                "security": {
                    "protocol": "SSL",
                    "ssl": {
                        "caSecret": {"name": "target-tls", "key": "ca.crt"},
                        "certificateSecret": {"name": "target-tls", "key": "tls.crt"},
                        "keySecret": {"name": "target-tls", "key": "tls.key"}
                    }
                }
            }]
        }
    }))
    .unwrap()
}

#[test]
fn secret_mounts_match_file_backed_engine_security_configuration() {
    let pipeline = secure_pipeline();
    let rendered: serde_json::Value =
        serde_yaml::from_str(&generate_config_yaml(&pipeline).unwrap()).unwrap();
    assert_eq!(
        rendered["security"]["sasl"]["username_file"],
        "/etc/streamforge/secrets/source/source-auth/username"
    );
    assert_eq!(
        rendered["target_security"]["ssl"]["certificate_location"],
        "/etc/streamforge/secrets/destination-0/target-tls/tls.crt"
    );

    let deployment =
        build_deployment(&pipeline, "default", "secure-orders", &BTreeMap::new()).unwrap();
    let pod = deployment.spec.unwrap().template.spec.unwrap();
    let mounts = pod.containers[0].volume_mounts.as_ref().unwrap();
    let mount_paths = mounts
        .iter()
        .map(|mount| mount.mount_path.as_str())
        .collect::<Vec<_>>();
    assert!(mount_paths.contains(&"/etc/streamforge/secrets/source/source-ca"));
    assert!(mount_paths.contains(&"/etc/streamforge/secrets/source/source-auth"));
    assert!(mount_paths.contains(&"/etc/streamforge/secrets/destination-0/target-tls"));
    assert_eq!(
        mount_paths
            .iter()
            .filter(|path| path.contains("destination-"))
            .count(),
        1
    );
}

#[test]
fn deployment_mounts_udfs_deterministically_and_hardens_container() {
    let deployment =
        build_deployment(&pipeline_with_udfs(), "default", "orders", &BTreeMap::new()).unwrap();
    let template = deployment.spec.unwrap().template;
    let annotations = template.metadata.unwrap().annotations.unwrap();
    assert_eq!(
        annotations["streamforge.io/udf-digests"],
        concat!(
            "config-module=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,",
            "pvc-module=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        )
    );
    assert_eq!(annotations["streamforge.io/config-sha256"].len(), 64);

    let pod = template.spec.unwrap();
    assert_eq!(pod.automount_service_account_token, Some(false));
    let pod_security = pod.security_context.as_ref().unwrap();
    assert_eq!(pod_security.run_as_non_root, Some(true));
    assert_eq!(pod_security.run_as_user, Some(65_532));
    assert_eq!(pod_security.run_as_group, Some(65_532));
    assert_eq!(pod_security.fs_group, Some(65_532));
    let volumes = pod.volumes.unwrap();
    assert_eq!(
        volumes
            .iter()
            .map(|volume| volume.name.as_str())
            .collect::<Vec<_>>(),
        vec!["config", "udf-config-module", "udf-pvc-module"]
    );
    let config_map = volumes[1].config_map.as_ref().unwrap();
    assert_eq!(config_map.name, "order-udfs");
    assert_eq!(config_map.default_mode, Some(0o444));
    assert_eq!(config_map.items.as_ref().unwrap()[0].path, "module.wasm");
    let pvc = volumes[2].persistent_volume_claim.as_ref().unwrap();
    assert_eq!(pvc.claim_name, "udf-artifacts");
    assert_eq!(pvc.read_only, Some(true));

    let container = &pod.containers[0];
    let mounts = container.volume_mounts.as_ref().unwrap();
    assert_eq!(
        mounts
            .iter()
            .map(|mount| (
                mount.name.as_str(),
                mount.mount_path.as_str(),
                mount.read_only
            ))
            .collect::<Vec<_>>(),
        vec![
            ("config", "/etc/streamforge", Some(true)),
            (
                "udf-config-module",
                "/var/run/streamforge/udfs/config-module",
                Some(true)
            ),
            (
                "udf-pvc-module",
                "/var/run/streamforge/udfs/pvc-module",
                Some(true)
            )
        ]
    );
    let security = container.security_context.as_ref().unwrap();
    assert_eq!(security.allow_privilege_escalation, Some(false));
    assert_eq!(security.privileged, Some(false));
    assert_eq!(security.read_only_root_filesystem, Some(true));
    assert_eq!(security.run_as_non_root, Some(true));
    assert_eq!(security.run_as_user, Some(65_532));
    assert_eq!(security.run_as_group, Some(65_532));
    assert_eq!(
        security.capabilities.as_ref().unwrap().drop.as_deref(),
        Some(["ALL".to_string()].as_slice())
    );
    assert_eq!(
        security.seccomp_profile.as_ref().unwrap().type_,
        "RuntimeDefault"
    );
}

#[test]
fn configuration_changes_trigger_a_pod_template_rollout() {
    let pipeline = pipeline_with_udfs();
    let before = build_deployment(&pipeline, "default", "orders", &BTreeMap::new()).unwrap();
    let before_digest = before
        .spec
        .unwrap()
        .template
        .metadata
        .unwrap()
        .annotations
        .unwrap()["streamforge.io/config-sha256"]
        .clone();

    let mut changed = pipeline;
    changed.spec.udfs.as_mut().unwrap().runtime.max_execution_ms = Some(11);
    let after = build_deployment(&changed, "default", "orders", &BTreeMap::new()).unwrap();
    let after_digest = &after
        .spec
        .unwrap()
        .template
        .metadata
        .unwrap()
        .annotations
        .unwrap()["streamforge.io/config-sha256"];

    assert_ne!(&before_digest, after_digest);
}

#[test]
fn generated_resources_are_controlled_by_the_pipeline() {
    let mut pipeline = pipeline_with_udfs();
    pipeline.metadata.uid = Some("pipeline-uid".to_string());

    let configmap = build_configmap(&pipeline, "default", "orders", &BTreeMap::new()).unwrap();
    let deployment = build_deployment(&pipeline, "default", "orders", &BTreeMap::new()).unwrap();

    for metadata in [&configmap.metadata, &deployment.metadata] {
        let owners = metadata.owner_references.as_ref().unwrap();
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].api_version, "streamforge.io/v1alpha1");
        assert_eq!(owners[0].kind, "StreamforgePipeline");
        assert_eq!(owners[0].name, "orders");
        assert_eq!(owners[0].uid, "pipeline-uid");
        assert_eq!(owners[0].controller, Some(true));
    }
}
