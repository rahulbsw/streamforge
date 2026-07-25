resource "aws_codebuild_project" "runner" {
  name          = "${var.project_name}-runner"
  description   = "Build the exact StreamForge benchmark revision into private ECR"
  service_role  = aws_iam_role.codebuild.arn
  build_timeout = 60

  source {
    type                = "GITHUB"
    location            = var.source_repository_url
    git_clone_depth     = 1
    buildspec           = "scripts/benchmarks/aws/buildspec.yml"
    report_build_status = false
  }

  source_version = var.source_revision

  artifacts {
    type = "NO_ARTIFACTS"
  }

  cache {
    type  = "LOCAL"
    modes = ["LOCAL_DOCKER_LAYER_CACHE"]
  }

  environment {
    compute_type                = "BUILD_GENERAL1_LARGE"
    image                       = "aws/codebuild/standard:7.0"
    type                        = "LINUX_CONTAINER"
    image_pull_credentials_type = "CODEBUILD"
    privileged_mode             = true

    environment_variable {
      name  = "AWS_ACCOUNT_ID"
      value = data.aws_caller_identity.current.account_id
    }

    environment_variable {
      name  = "ECR_REPOSITORY_URI"
      value = aws_ecr_repository.runner.repository_url
    }

    environment_variable {
      name  = "IMAGE_TAG"
      value = local.runner_image_tag
    }
  }

  logs_config {
    cloudwatch_logs {
      group_name  = aws_cloudwatch_log_group.codebuild.name
      stream_name = "build"
      status      = "ENABLED"
    }
  }

  tags = {
    Name = "${var.project_name}-runner-build"
  }
}
