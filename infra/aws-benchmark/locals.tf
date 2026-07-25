locals {
  common_tags = {
    Project     = var.project_name
    Owner       = var.owner
    ManagedBy   = "Terraform"
    Environment = "benchmark"
    ExpiresAt   = var.expires_at
    AutoDelete  = "true"
  }

  interface_endpoint_services = toset([
    "ecs",
    "ecs-agent",
    "ecs-telemetry",
    "ecr.api",
    "ecr.dkr",
    "logs",
  ])

  runner_image_tag = coalesce(var.runner_image_tag, substr(var.source_revision, 0, 12))
  benchmark_runner_image = coalesce(
    var.benchmark_runner_image,
    "${aws_ecr_repository.runner.repository_url}:${local.runner_image_tag}",
  )
  kafka_image = coalesce(
    var.kafka_image,
    local.benchmark_runner_image,
  )
}
