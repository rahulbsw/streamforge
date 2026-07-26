use crate::config::ErrorPolicy;
use crate::error::{
    DestinationFailure, DestinationFailureDisposition, DestinationStage, MirrorMakerError, Result,
};
use tracing::{error, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransformErrorAction {
    SkipDestination,
    SendOriginal,
}

pub(crate) fn handle_filter_error(
    policy: ErrorPolicy,
    destination: &str,
    source: MirrorMakerError,
) -> Result<bool> {
    match policy {
        ErrorPolicy::Fail => {
            error!(
                destination,
                operation = %DestinationStage::Filter,
                error = %source,
                "Pipeline halted due to error (error_policy: fail)"
            );
            Err(destination_failure(
                destination,
                DestinationStage::Filter,
                DestinationFailureDisposition::FailFast,
                source,
            ))
        }
        ErrorPolicy::Dlq => {
            warn!(
                destination,
                operation = %DestinationStage::Filter,
                error = %source,
                "Error will be sent to DLQ (error_policy: dlq)"
            );
            Err(destination_failure(
                destination,
                DestinationStage::Filter,
                DestinationFailureDisposition::DeadLetter,
                source,
            ))
        }
        ErrorPolicy::SkipAndLog => {
            warn!(
                destination,
                operation = %DestinationStage::Filter,
                error = %source,
                "Skipping message due to error (error_policy: skip_and_log)"
            );
            Ok(false)
        }
        ErrorPolicy::Continue => {
            warn!(
                destination,
                operation = %DestinationStage::Filter,
                error = %source,
                "Treating failed filter as pass (error_policy: continue)"
            );
            Ok(true)
        }
    }
}

pub(crate) fn handle_transform_error(
    policy: ErrorPolicy,
    destination: &str,
    stage: DestinationStage,
    source: MirrorMakerError,
) -> Result<TransformErrorAction> {
    match policy {
        ErrorPolicy::Fail => {
            error!(
                destination,
                operation = %stage,
                error = %source,
                "Pipeline halted due to error (error_policy: fail)"
            );
            Err(destination_failure(
                destination,
                stage,
                DestinationFailureDisposition::FailFast,
                source,
            ))
        }
        ErrorPolicy::Dlq => {
            warn!(
                destination,
                operation = %stage,
                error = %source,
                "Error will be sent to DLQ (error_policy: dlq)"
            );
            Err(destination_failure(
                destination,
                stage,
                DestinationFailureDisposition::DeadLetter,
                source,
            ))
        }
        ErrorPolicy::SkipAndLog => {
            warn!(
                destination,
                operation = %stage,
                error = %source,
                "Skipping message due to error (error_policy: skip_and_log)"
            );
            Ok(TransformErrorAction::SkipDestination)
        }
        ErrorPolicy::Continue => {
            warn!(
                destination,
                operation = %stage,
                error = %source,
                "Sending the original unchanged envelope (error_policy: continue)"
            );
            Ok(TransformErrorAction::SendOriginal)
        }
    }
}

pub(crate) fn destination_failure(
    destination: &str,
    stage: DestinationStage,
    disposition: DestinationFailureDisposition,
    source: MirrorMakerError,
) -> MirrorMakerError {
    MirrorMakerError::DestinationFailure {
        failure: Box::new(DestinationFailure::new(
            destination,
            stage,
            disposition,
            source,
        )),
    }
}

pub(crate) fn collect_destination_failures(
    failures: &mut Vec<DestinationFailure>,
    destination: String,
    fallback_stage: DestinationStage,
    error: MirrorMakerError,
) {
    match error {
        MirrorMakerError::DestinationFailure { failure } => failures.push(*failure),
        MirrorMakerError::DestinationFailures { failures: nested } => failures.extend(nested),
        source => failures.push(DestinationFailure::new(
            destination,
            fallback_stage,
            // Retrying the entire multi-destination processor would re-run
            // destinations that already succeeded.
            DestinationFailureDisposition::FailFast,
            source,
        )),
    }
}
