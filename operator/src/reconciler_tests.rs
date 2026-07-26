use std::collections::BTreeMap;

use crate::crd::StreamforgePipeline;
use crate::reconciler::PipelineReconciler;

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

#[test]
fn deployment_mounts_udfs_deterministically_and_hardens_container() {
    let deployment = PipelineReconciler::build_deployment(
        &pipeline_with_udfs(),
        "default",
        "orders",
        &BTreeMap::new(),
    )
    .unwrap();
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
    let before =
        PipelineReconciler::build_deployment(&pipeline, "default", "orders", &BTreeMap::new())
            .unwrap();
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
    let after =
        PipelineReconciler::build_deployment(&changed, "default", "orders", &BTreeMap::new())
            .unwrap();
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
