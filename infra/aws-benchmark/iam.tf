data "aws_iam_policy_document" "ec2_assume" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ec2.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "ecs_instance" {
  name_prefix        = "${var.project_name}-instance-"
  assume_role_policy = data.aws_iam_policy_document.ec2_assume.json
}

data "aws_iam_policy_document" "ecs_instance" {
  statement {
    sid = "EcsContainerInstance"
    actions = [
      "ecs:CreateCluster",
      "ecs:DeregisterContainerInstance",
      "ecs:DiscoverPollEndpoint",
      "ecs:Poll",
      "ecs:RegisterContainerInstance",
      "ecs:StartTelemetrySession",
      "ecs:SubmitAttachmentStateChanges",
      "ecs:SubmitContainerStateChange",
      "ecs:SubmitTaskStateChange",
      "ecs:TagResource",
      "ecs:UpdateContainerInstancesState",
    ]
    resources = ["*"]
  }
}

resource "aws_iam_role_policy" "ecs_instance" {
  name_prefix = "ecs-agent-"
  role        = aws_iam_role.ecs_instance.id
  policy      = data.aws_iam_policy_document.ecs_instance.json
}

resource "aws_iam_instance_profile" "ecs" {
  name_prefix = "${var.project_name}-"
  role        = aws_iam_role.ecs_instance.name
}

data "aws_iam_policy_document" "ecs_task_assume" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "task_execution" {
  name_prefix        = "${var.project_name}-execution-"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume.json
}

data "aws_iam_policy_document" "task_execution" {
  statement {
    sid       = "EcrAuthorization"
    actions   = ["ecr:GetAuthorizationToken"]
    resources = ["*"]
  }

  statement {
    sid = "PullBenchmarkImages"
    actions = [
      "ecr:BatchCheckLayerAvailability",
      "ecr:BatchGetImage",
      "ecr:GetDownloadUrlForLayer",
    ]
    resources = [
      aws_ecr_repository.runner.arn,
    ]
  }

  statement {
    sid = "WriteContainerLogs"
    actions = [
      "logs:CreateLogStream",
      "logs:PutLogEvents",
    ]
    resources = ["${aws_cloudwatch_log_group.benchmark.arn}:*"]
  }
}

resource "aws_iam_role_policy" "task_execution" {
  name_prefix = "pull-and-logs-"
  role        = aws_iam_role.task_execution.id
  policy      = data.aws_iam_policy_document.task_execution.json
}

resource "aws_iam_role" "benchmark_task" {
  name_prefix        = "${var.project_name}-task-"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume.json
}

data "aws_iam_policy_document" "benchmark_task" {
  statement {
    sid = "ListArtifactBucket"
    actions = [
      "s3:GetBucketLocation",
      "s3:ListBucket",
      "s3:ListBucketMultipartUploads",
    ]
    resources = [aws_s3_bucket.artifacts.arn]

    condition {
      test     = "StringLike"
      variable = "s3:prefix"
      values = [
        var.benchmark_run_id,
        "${var.benchmark_run_id}/*",
      ]
    }
  }

  statement {
    sid = "ManageBenchmarkArtifacts"
    actions = [
      "s3:AbortMultipartUpload",
      "s3:DeleteObject",
      "s3:GetObject",
      "s3:ListMultipartUploadParts",
      "s3:PutObject",
    ]
    resources = ["${aws_s3_bucket.artifacts.arn}/${var.benchmark_run_id}/*"]
  }
}

resource "aws_iam_role_policy" "benchmark_task" {
  name_prefix = "artifacts-"
  role        = aws_iam_role.benchmark_task.id
  policy      = data.aws_iam_policy_document.benchmark_task.json
}

resource "aws_iam_role" "codebuild" {
  name_prefix        = "${var.project_name}-build-"
  assume_role_policy = data.aws_iam_policy_document.codebuild_assume.json
}

data "aws_iam_policy_document" "codebuild_assume" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["codebuild.amazonaws.com"]
    }
  }
}

data "aws_iam_policy_document" "codebuild" {
  statement {
    sid       = "EcrAuthorization"
    actions   = ["ecr:GetAuthorizationToken"]
    resources = ["*"]
  }

  statement {
    sid = "PushRunnerImage"
    actions = [
      "ecr:BatchCheckLayerAvailability",
      "ecr:BatchGetImage",
      "ecr:CompleteLayerUpload",
      "ecr:GetDownloadUrlForLayer",
      "ecr:InitiateLayerUpload",
      "ecr:PutImage",
      "ecr:UploadLayerPart",
    ]
    resources = [aws_ecr_repository.runner.arn]
  }

  statement {
    sid = "WriteBuildLogs"
    actions = [
      "logs:CreateLogStream",
      "logs:PutLogEvents",
    ]
    resources = ["${aws_cloudwatch_log_group.codebuild.arn}:*"]
  }
}

resource "aws_iam_role_policy" "codebuild" {
  name_prefix = "build-runner-"
  role        = aws_iam_role.codebuild.id
  policy      = data.aws_iam_policy_document.codebuild.json
}
