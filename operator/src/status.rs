use k8s_openapi::api::apps::v1::Deployment;

use crate::crd::{PipelineCondition, PipelineStatus, StreamforgePipeline};

pub(crate) fn desired_status(
    pipeline: &StreamforgePipeline,
    deployment: &Deployment,
    now: String,
) -> PipelineStatus {
    let deployment_status = deployment.status.as_ref().cloned().unwrap_or_default();
    let ready_replicas = deployment_status.ready_replicas.unwrap_or(0);
    let desired_replicas = pipeline.spec.replicas;
    let deployment_generation = deployment.metadata.generation.unwrap_or(0);
    let generation_observed =
        deployment_status.observed_generation.unwrap_or(-1) >= deployment_generation;
    let updated = deployment_status.updated_replicas.unwrap_or(0) == desired_replicas;
    let available = deployment_status.available_replicas.unwrap_or(0) == desired_replicas;
    let replica_failure = deployment_status
        .conditions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|condition| condition.type_ == "ReplicaFailure" && condition.status == "True");

    let (phase, ready_status, reason, message) = if let Some(failure) = replica_failure {
        (
            "Failed",
            "False",
            "ReplicaFailure",
            failure
                .message
                .clone()
                .unwrap_or_else(|| "The Deployment reported a replica failure".to_string()),
        )
    } else if generation_observed && updated && available && ready_replicas == desired_replicas {
        (
            "Running",
            "True",
            "MinimumReplicasAvailable",
            format!("All {desired_replicas} desired replicas are ready"),
        )
    } else if !generation_observed {
        (
            "Pending",
            "False",
            "Progressing",
            "Waiting for the Deployment controller to observe the latest generation".to_string(),
        )
    } else {
        (
            "Pending",
            "False",
            "ReplicasNotReady",
            format!("{ready_replicas} of {desired_replicas} desired replicas are ready"),
        )
    };

    let previous_ready = pipeline.status.as_ref().and_then(|status| {
        status
            .conditions
            .iter()
            .find(|condition| condition.r#type == "Ready")
    });
    let last_transition_time = previous_ready
        .filter(|condition| condition.status == ready_status)
        .and_then(|condition| condition.last_transition_time.clone())
        .or_else(|| Some(now.clone()));

    let mut desired = PipelineStatus {
        phase: phase.to_string(),
        replicas: ready_replicas,
        conditions: vec![PipelineCondition {
            r#type: "Ready".to_string(),
            status: ready_status.to_string(),
            last_transition_time,
            reason: Some(reason.to_string()),
            message: Some(message),
        }],
        last_updated: pipeline
            .status
            .as_ref()
            .and_then(|status| status.last_updated.clone()),
    };

    if pipeline
        .status
        .as_ref()
        .is_some_and(|current| current == &desired)
    {
        return pipeline.status.clone().expect("status checked above");
    }

    desired.last_updated = Some(now);
    desired
}

#[cfg(test)]
mod tests {
    use k8s_openapi::api::apps::v1::{Deployment, DeploymentStatus};
    use kube::api::ObjectMeta;

    use super::desired_status;
    use crate::crd::StreamforgePipeline;

    #[test]
    fn status_is_idempotent_and_ready_transition_time_changes_only_on_transition() {
        let mut pipeline: StreamforgePipeline = serde_json::from_value(serde_json::json!({
            "apiVersion": "streamforge.io/v1alpha1",
            "kind": "StreamforgePipeline",
            "metadata": {"name": "orders"},
            "spec": {
                "source": {"brokers": "source:9092", "topic": "orders"},
                "destinations": [{"brokers": "target:9092", "topic": "accepted"}],
                "replicas": 2
            }
        }))
        .unwrap();

        let progressing = deployment_with_status(2, 1, 1, 1);
        let first = desired_status(&pipeline, &progressing, "2026-07-26T01:00:00Z".into());
        assert_eq!(first.phase, "Pending");
        assert_eq!(first.replicas, 1);
        assert_eq!(first.conditions[0].r#type, "Ready");
        assert_eq!(first.conditions[0].status, "False");
        assert_eq!(
            first.conditions[0].last_transition_time.as_deref(),
            Some("2026-07-26T01:00:00Z")
        );

        pipeline.status = Some(first.clone());
        let unchanged = desired_status(&pipeline, &progressing, "2026-07-26T01:05:00Z".into());
        assert_eq!(unchanged, first);

        let ready = deployment_with_status(2, 2, 2, 2);
        let transitioned = desired_status(&pipeline, &ready, "2026-07-26T01:10:00Z".into());
        assert_eq!(transitioned.phase, "Running");
        assert_eq!(transitioned.replicas, 2);
        assert_eq!(transitioned.conditions[0].status, "True");
        assert_eq!(
            transitioned.conditions[0].last_transition_time.as_deref(),
            Some("2026-07-26T01:10:00Z")
        );
        assert_eq!(
            transitioned.last_updated.as_deref(),
            Some("2026-07-26T01:10:00Z")
        );
        let serialized = serde_json::to_value(&transitioned).unwrap();
        assert_eq!(serialized["lastUpdated"], "2026-07-26T01:10:00Z");
        assert!(serialized.get("last_updated").is_none());
    }

    fn deployment_with_status(
        desired: i32,
        ready: i32,
        updated: i32,
        available: i32,
    ) -> Deployment {
        Deployment {
            metadata: ObjectMeta {
                generation: Some(3),
                ..Default::default()
            },
            status: Some(DeploymentStatus {
                replicas: Some(desired),
                ready_replicas: Some(ready),
                updated_replicas: Some(updated),
                available_replicas: Some(available),
                observed_generation: Some(3),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}
