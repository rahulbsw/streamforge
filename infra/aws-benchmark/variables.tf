variable "aws_region" {
  description = "AWS region for the benchmark environment."
  type        = string
  default     = "us-west-2"

  validation {
    condition     = var.aws_region == "us-west-2"
    error_message = "This benchmark is intentionally pinned to us-west-2."
  }
}

variable "availability_zone" {
  description = "Single availability zone used by the private benchmark subnet."
  type        = string
  default     = "us-west-2a"

  validation {
    condition     = var.availability_zone == "us-west-2a"
    error_message = "This cost-bounded benchmark is intentionally pinned to us-west-2a."
  }
}

variable "project_name" {
  description = "Lowercase name prefix for benchmark resources."
  type        = string
  default     = "streamforge-aws-benchmark"

  validation {
    condition     = can(regex("^[a-z][a-z0-9-]{2,31}$", var.project_name))
    error_message = "project_name must be 3-32 lowercase letters, numbers, or hyphens."
  }
}

variable "owner" {
  description = "Owner tag used for inventory and cost attribution."
  type        = string

  validation {
    condition     = length(trimspace(var.owner)) >= 3
    error_message = "owner must identify the person or team responsible for cleanup."
  }
}

variable "expires_at" {
  description = "Hard UTC expiry in RFC3339 form (YYYY-MM-DDTHH:MM:SSZ). The ASG scales to zero at this time."
  type        = string

  validation {
    condition = (
      can(regex("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$", var.expires_at)) &&
      can(formatdate("YYYY-MM-DD'T'hh:mm:ss'Z'", var.expires_at))
    )
    error_message = "expires_at must be a valid UTC timestamp such as 2026-07-26T03:00:00Z."
  }
}

variable "vpc_cidr" {
  description = "RFC1918 CIDR for the isolated benchmark VPC."
  type        = string
  default     = "10.79.0.0/24"
}

variable "private_subnet_cidr" {
  description = "CIDR for the only, private benchmark subnet."
  type        = string
  default     = "10.79.0.0/24"
}

variable "instance_type" {
  description = "Dedicated On-Demand benchmark instance type."
  type        = string
  default     = "c7i.2xlarge"

  validation {
    condition     = var.instance_type == "c7i.2xlarge"
    error_message = "The matched benchmark contract requires c7i.2xlarge."
  }
}

variable "provision_runtime" {
  description = "Create billable PrivateLink endpoints and one EC2 host only after the runner image has been built."
  type        = bool
  default     = false
}

variable "source_repository_url" {
  description = "GitHub repository cloned by CodeBuild."
  type        = string
  default     = "https://github.com/rahulbsw/streamforge.git"

  validation {
    condition     = startswith(var.source_repository_url, "https://github.com/")
    error_message = "source_repository_url must be an HTTPS GitHub repository URL."
  }
}

variable "source_revision" {
  description = "Exact 40-character Git commit built into the benchmark runner."
  type        = string

  validation {
    condition     = can(regex("^[0-9a-f]{40}$", var.source_revision))
    error_message = "source_revision must be a lowercase 40-character Git commit SHA."
  }
}

variable "benchmark_run_id" {
  description = "Unique run identifier and S3 artifact prefix."
  type        = string

  validation {
    condition     = can(regex("^[a-zA-Z0-9][a-zA-Z0-9._-]{2,63}$", var.benchmark_run_id))
    error_message = "benchmark_run_id must be 3-64 safe path characters."
  }
}

variable "runner_image_tag" {
  description = "Optional immutable runner tag. Defaults to the first 12 characters of source_revision."
  type        = string
  default     = null
  nullable    = true

  validation {
    condition = (
      var.runner_image_tag == null ||
      can(regex("^[a-zA-Z0-9_][a-zA-Z0-9._-]{0,127}$", var.runner_image_tag))
    )
    error_message = "runner_image_tag must be a valid ECR tag."
  }
}

variable "benchmark_runner_image" {
  description = "Optional immutable private-ECR runner image URI. Defaults to the created runner repository's benchmark tag."
  type        = string
  default     = null
  nullable    = true
}

variable "kafka_image" {
  description = "Optional immutable private-ECR Kafka image URI. Defaults to the runner image, which is based on the pinned Kafka image."
  type        = string
  default     = null
  nullable    = true
}

variable "benchmark_runner_command" {
  description = "Optional arguments passed to the runner image entry point."
  type        = list(string)
  default     = []
}

variable "artifact_retention_days" {
  description = "Days before temporary benchmark artifacts expire."
  type        = number
  default     = 2

  validation {
    condition     = var.artifact_retention_days >= 1 && var.artifact_retention_days <= 7
    error_message = "artifact_retention_days must be between 1 and 7."
  }
}

variable "log_retention_days" {
  description = "CloudWatch Logs retention for benchmark containers."
  type        = number
  default     = 3

  validation {
    condition     = contains([1, 3, 5, 7], var.log_retention_days)
    error_message = "log_retention_days must be one of 1, 3, 5, or 7."
  }
}
