//! Validate engine configuration or a Kubernetes StreamforgePipeline resource.

use clap::Parser;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use streamforge::config::MirrorMakerConfig;
use streamforge::filter_parser::{parse_filter, parse_key_transform, parse_transform_with_cache};
use streamforge::kubernetes::config_from_pipeline_value;
use streamforge::WasmRegistry;

#[derive(Parser, Debug)]
#[command(
    name = "streamforge-validate",
    about = "Validate StreamForge configuration and StreamforgePipeline files"
)]
struct Opt {
    /// File to validate
    config: PathBuf,

    /// Input document type: config or pipeline-crd
    #[arg(long, default_value = "config", value_parser = ["config", "pipeline-crd"])]
    input_format: String,

    /// Output format: text or json
    #[arg(long, default_value = "text", value_parser = ["text", "json"])]
    output: String,

    /// Show detailed validation output
    #[arg(short, long)]
    verbose: bool,

    /// Fail on warnings
    #[arg(short = 'W', long)]
    fail_on_warnings: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostic {
    severity: &'static str,
    code: &'static str,
    field: String,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationReport {
    valid: bool,
    input_format: String,
    expressions_validated: usize,
    wasm_modules_verified: usize,
    diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    fn new(input_format: &str) -> Self {
        Self {
            valid: true,
            input_format: input_format.to_string(),
            expressions_validated: 0,
            wasm_modules_verified: 0,
            diagnostics: Vec::new(),
        }
    }

    fn error(&mut self, code: &'static str, field: impl Into<String>, message: impl Into<String>) {
        self.valid = false;
        self.diagnostics.push(Diagnostic {
            severity: "error",
            code,
            field: field.into(),
            message: message.into(),
        });
    }

    fn warning(
        &mut self,
        code: &'static str,
        field: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic {
            severity: "warning",
            code,
            field: field.into(),
            message: message.into(),
        });
    }
}

fn parse_config(
    content: &str,
    input_format: &str,
    report: &mut ValidationReport,
) -> Option<MirrorMakerConfig> {
    if input_format == "pipeline-crd" {
        let value: serde_json::Value = match serde_yaml::from_str(content) {
            Ok(value) => value,
            Err(error) => {
                report.error("invalid_yaml", "$", error.to_string());
                return None;
            }
        };
        match config_from_pipeline_value(value) {
            Ok(config) => Some(config),
            Err(error) => {
                report.error("invalid_pipeline", error.field, error.message);
                None
            }
        }
    } else {
        match serde_yaml::from_str(content) {
            Ok(config) => Some(config),
            Err(error) => {
                report.error("invalid_yaml", "$", error.to_string());
                None
            }
        }
    }
}

fn validate_expression(
    expression: &str,
    field: String,
    kind: &'static str,
    report: &mut ValidationReport,
) {
    report.expressions_validated += 1;
    let result = match kind {
        "filter" => parse_filter(expression).map(|_| ()),
        "transform" => parse_transform_with_cache(expression, None).map(|_| ()),
        "key_transform" => parse_key_transform(expression).map(|_| ()),
        _ => unreachable!("validation expression kind is internal"),
    };
    if let Err(error) = result {
        report.error("invalid_dsl", field, error.to_string());
    }
}

fn validate_config(
    config: &MirrorMakerConfig,
    load_wasm_artifacts: bool,
    report: &mut ValidationReport,
) {
    if let Err(error) = config.validate() {
        report.error("invalid_config", "spec", error.to_string());
    }

    if load_wasm_artifacts {
        if let Some(wasm) = &config.wasm {
            match WasmRegistry::load(wasm) {
                Ok(registry) => {
                    report.wasm_modules_verified = registry.module_names().count();
                }
                Err(error) => {
                    report.error("invalid_wasm_artifact", "wasm.modules", error.to_string());
                }
            }
        }
    }

    if let Some(expression) = &config.transform {
        validate_expression(expression, "transform".to_string(), "transform", report);
    }

    if let Some(routing) = &config.routing {
        for (index, destination) in routing.destinations.iter().enumerate() {
            if let Some(expression) = &destination.filter {
                validate_expression(
                    expression,
                    format!("routing.destinations[{index}].filter"),
                    "filter",
                    report,
                );
                if expression.contains("KEY_SUFFIX:") {
                    report.warning(
                        "deprecated_dsl",
                        format!("routing.destinations[{index}].filter"),
                        "KEY_SUFFIX is deprecated; use KEY_MATCHES with a suffix regex",
                    );
                }
                if expression.contains("KEY_CONTAINS:") {
                    report.warning(
                        "deprecated_dsl",
                        format!("routing.destinations[{index}].filter"),
                        "KEY_CONTAINS is deprecated; use KEY_MATCHES",
                    );
                }
            }
            if let Some(expression) = &destination.transform {
                validate_expression(
                    expression,
                    format!("routing.destinations[{index}].transform"),
                    "transform",
                    report,
                );
            }
            if let Some(expression) = &destination.key_transform {
                validate_expression(
                    expression,
                    format!("routing.destinations[{index}].keyTransform"),
                    "key_transform",
                    report,
                );
            }
        }
    }
}

fn print_text(report: &ValidationReport, verbose: bool) {
    if verbose {
        println!(
            "Validated {} expression(s), verified {} WebAssembly module(s) from {} input",
            report.expressions_validated, report.wasm_modules_verified, report.input_format
        );
    }
    for diagnostic in &report.diagnostics {
        println!(
            "{} [{}] {}: {}",
            diagnostic.severity.to_uppercase(),
            diagnostic.code,
            diagnostic.field,
            diagnostic.message
        );
    }
    if report.valid {
        println!("Validation passed");
    } else {
        println!("Validation failed");
    }
}

fn main() {
    let opt = Opt::parse();
    let mut report = ValidationReport::new(&opt.input_format);

    match fs::read_to_string(&opt.config) {
        Ok(content) => {
            if let Some(config) = parse_config(&content, &opt.input_format, &mut report) {
                validate_config(&config, opt.input_format == "config", &mut report);
            }
        }
        Err(error) => report.error("read_failed", "$", error.to_string()),
    }

    let has_warnings = report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == "warning");
    if opt.output == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("validation report is serializable")
        );
    } else {
        print_text(&report, opt.verbose);
    }

    std::process::exit(if !report.valid || (opt.fail_on_warnings && has_warnings) {
        1
    } else {
        0
    });
}
