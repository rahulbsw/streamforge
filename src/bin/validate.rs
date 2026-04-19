//! Config validation CLI
//!
//! Usage: streamforge-validate <config.yaml>
//!
//! Validates a StreamForge configuration file by parsing all filter and
//! transform expressions without executing them. Reports syntax errors,
//! invalid paths, and deprecation warnings.

use std::fs;
use std::path::PathBuf;
use streamforge::config::MirrorMakerConfig;
use streamforge::filter_parser::{parse_filter, parse_transform_with_cache};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(
    name = "streamforge-validate",
    about = "Validate StreamForge configuration files"
)]
struct Opt {
    /// Config file to validate
    #[structopt(parse(from_os_str))]
    config: PathBuf,

    /// Show detailed validation output
    #[structopt(short, long)]
    verbose: bool,

    /// Fail on warnings (exit with non-zero)
    #[structopt(short = "W", long)]
    fail_on_warnings: bool,
}

fn main() {
    let opt = Opt::from_args();

    // Initialize tracing for error messages
    let log_level = if opt.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.parse().unwrap()),
        )
        .init();

    // Read config file
    let config_content = match fs::read_to_string(&opt.config) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("❌ Error: Failed to read config file: {}", e);
            std::process::exit(1);
        }
    };

    // Parse YAML
    let config: MirrorMakerConfig = match serde_yaml::from_str(&config_content) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("❌ Error: Invalid YAML syntax:");
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    println!("✅ YAML syntax valid");
    println!("📋 Validating StreamForge configuration...\n");

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut expr_count = 0;

    // Validate single-destination mode
    if config.routing.is_none() {
        if let Some(ref filter_expr) = config.routing.as_ref().and_then(|r| {
            r.destinations.first().and_then(|d| d.filter.as_ref())
        }) {
            expr_count += 1;
            if opt.verbose {
                println!("  Validating filter: {}", filter_expr);
            }
            if let Err(e) = parse_filter(filter_expr) {
                errors.push(format!("Filter expression: {}", e));
            }
        }

        if let Some(ref transform_expr) = config.transform {
            expr_count += 1;
            if opt.verbose {
                println!("  Validating transform: {}", transform_expr);
            }
            if let Err(e) = parse_transform_with_cache(transform_expr, None) {
                errors.push(format!("Transform expression: {}", e));
            }
        }
    }

    // Validate multi-destination routing
    if let Some(ref routing) = config.routing {
        println!("📍 Routing mode: {}", routing.routing_type);
        println!("📦 Destinations: {}", routing.destinations.len());

        for (idx, dest) in routing.destinations.iter().enumerate() {
            let dest_name = format!("Destination #{} ({})", idx + 1, dest.output);

            if opt.verbose {
                println!("\n  {} {}", if idx == 0 { "┌─" } else { "├─" }, dest_name);
            }

            // Validate filter
            if let Some(ref filter_expr) = dest.filter {
                expr_count += 1;
                if opt.verbose {
                    println!("  │  Filter: {}", filter_expr);
                }
                if let Err(e) = parse_filter(filter_expr) {
                    errors.push(format!("{} - Filter: {}", dest_name, e));
                }
            }

            // Validate transform
            if let Some(ref transform_expr) = dest.transform {
                expr_count += 1;
                if opt.verbose {
                    println!("  │  Transform: {}", transform_expr);
                }
                if let Err(e) = parse_transform_with_cache(transform_expr, None) {
                    errors.push(format!("{} - Transform: {}", dest_name, e));
                }
            }

            // Validate key transform
            if let Some(ref key_expr) = dest.key_transform {
                expr_count += 1;
                if opt.verbose {
                    println!("  │  Key transform: {}", key_expr);
                }
                // Key transforms use a simpler syntax, validate as transform
                if key_expr.starts_with('/') || key_expr.starts_with("HASH:") || key_expr.starts_with("CONSTRUCT:") {
                    if let Err(e) = parse_transform_with_cache(key_expr, None) {
                        errors.push(format!("{} - Key transform: {}", dest_name, e));
                    }
                } else if !key_expr.starts_with("CONSTANT:") && !key_expr.contains('{') {
                    warnings.push(format!(
                        "{} - Key transform '{}' may be invalid. Expected: '/path', 'CONSTANT:...', 'HASH:...', or template",
                        dest_name, key_expr
                    ));
                }
            }

            // Check for deprecated KEY_SUFFIX and KEY_CONTAINS
            if let Some(ref filter_expr) = dest.filter {
                if filter_expr.contains("KEY_SUFFIX:") {
                    warnings.push(format!(
                        "{} - KEY_SUFFIX is deprecated. Use KEY_MATCHES with regex instead. \
                        Example: KEY_SUFFIX:-prod → KEY_MATCHES:.*-prod$",
                        dest_name
                    ));
                }
                if filter_expr.contains("KEY_CONTAINS:") {
                    warnings.push(format!(
                        "{} - KEY_CONTAINS is deprecated. Use KEY_MATCHES with regex instead. \
                        Example: KEY_CONTAINS:test → KEY_MATCHES:.*test.*",
                        dest_name
                    ));
                }
            }
        }

        if opt.verbose && !routing.destinations.is_empty() {
            println!("  └─ End of destinations\n");
        }
    }

    // Summary
    println!("\n═══════════════════════════════════════════════════");
    println!("📊 Validation Summary");
    println!("═══════════════════════════════════════════════════");
    println!("   Expressions validated: {}", expr_count);
    println!("   Errors: {}", errors.len());
    println!("   Warnings: {}", warnings.len());
    println!("═══════════════════════════════════════════════════\n");

    // Report errors
    if !errors.is_empty() {
        println!("❌ Errors found:\n");
        for (i, err) in errors.iter().enumerate() {
            println!("{}. {}", i + 1, err);
        }
        println!();
    }

    // Report warnings
    if !warnings.is_empty() {
        println!("⚠️  Warnings:\n");
        for (i, warn) in warnings.iter().enumerate() {
            println!("{}. {}", i + 1, warn);
        }
        println!();
    }

    // Exit status
    let exit_code = if !errors.is_empty() {
        println!("❌ Validation FAILED - Fix errors before deploying");
        1
    } else if !warnings.is_empty() {
        if opt.fail_on_warnings {
            println!("⚠️  Validation FAILED - Warnings present and --fail-on-warnings set");
            1
        } else {
            println!("⚠️  Validation PASSED with warnings - Review warnings before deploying");
            0
        }
    } else {
        println!("✅ Validation PASSED - Config is ready for deployment");
        0
    };

    std::process::exit(exit_code);
}
