output "network" {
  description = "Private network identifiers. No internet gateway or public subnet is created."
  value = {
    vpc_id                     = aws_vpc.benchmark.id
    private_subnet_id          = aws_subnet.private.id
    instance_security_group_id = aws_security_group.instance.id
    task_security_group_id     = aws_security_group.task.id
    endpoint_security_group_id = aws_security_group.endpoints.id
    interface_endpoint_ids = {
      for service, endpoint in aws_vpc_endpoint.interface : service => endpoint.id
    }
    s3_gateway_endpoint_id = aws_vpc_endpoint.s3.id
  }
}

output "ecs_cluster" {
  description = "ECS cluster name and ARN."
  value = {
    name = aws_ecs_cluster.benchmark.name
    arn  = aws_ecs_cluster.benchmark.arn
  }
}

output "autoscaling_group_name" {
  description = "Single-instance On-Demand ECS capacity group."
  value       = aws_autoscaling_group.benchmark.name
}

output "task_definition_arn" {
  description = "One-shot Kafka plus benchmark-runner task definition."
  value       = aws_ecs_task_definition.benchmark.arn
}

output "image_repositories" {
  description = "Private ECR repositories that must be populated before running the task."
  value = {
    runner = aws_ecr_repository.runner.repository_url
  }
}

output "resolved_images" {
  description = "Image references encoded in the current task definition."
  value = {
    runner = local.benchmark_runner_image
    kafka  = local.kafka_image
  }
}

output "artifact_bucket" {
  description = "Private encrypted S3 bucket for result manifests and logs."
  value       = aws_s3_bucket.artifacts.id
}

output "cloudwatch_log_group" {
  description = "Container log group."
  value       = aws_cloudwatch_log_group.benchmark.name
}

output "hard_expiry" {
  description = "UTC time at which the ASG is forced to zero capacity."
  value       = var.expires_at
}

output "codebuild_project_name" {
  description = "Build this project before enabling provision_runtime."
  value       = aws_codebuild_project.runner.name
}

output "start_build_command" {
  description = "Build and push the immutable runner image into private ECR."
  value       = "aws codebuild start-build --region ${var.aws_region} --project-name ${aws_codebuild_project.runner.name}"
}

output "run_task_command" {
  description = "Run after both immutable images are present in ECR and the ECS instance is ACTIVE."
  value = join(" ", [
    "aws ecs run-task",
    "--region ${var.aws_region}",
    "--cluster ${aws_ecs_cluster.benchmark.name}",
    "--launch-type EC2",
    "--count 1",
    "--task-definition ${aws_ecs_task_definition.benchmark.arn}",
    "--network-configuration",
    "'awsvpcConfiguration={subnets=[${aws_subnet.private.id}],securityGroups=[${aws_security_group.task.id}],assignPublicIp=DISABLED}'",
  ])
}

output "destroy_command" {
  description = "Run immediately after artifacts are downloaded."
  value       = "terraform destroy"
}
