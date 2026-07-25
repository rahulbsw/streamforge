resource "aws_ecs_task_definition" "benchmark" {
  family                   = var.project_name
  requires_compatibilities = ["EC2"]
  network_mode             = "awsvpc"
  cpu                      = "7168"
  memory                   = "13312"
  execution_role_arn       = aws_iam_role.task_execution.arn
  task_role_arn            = aws_iam_role.benchmark_task.arn

  runtime_platform {
    operating_system_family = "LINUX"
    cpu_architecture        = "X86_64"
  }

  volume {
    name = "benchmark-control"
  }

  volume {
    name = "kafka-data"
  }

  container_definitions = jsonencode([
    {
      name                   = "kafka"
      image                  = local.kafka_image
      essential              = true
      cpu                    = 2048
      memory                 = 4096
      memoryReservation      = 3072
      readonlyRootFilesystem = false
      privileged             = false
      entryPoint             = ["/etc/confluent/docker/run"]
      environment = [
        { name = "CLUSTER_ID", value = "MkU3OEVBNTcwNTJENDM2Qk" },
        { name = "KAFKA_NODE_ID", value = "1" },
        { name = "KAFKA_PROCESS_ROLES", value = "broker,controller" },
        { name = "KAFKA_CONTROLLER_QUORUM_VOTERS", value = "1@127.0.0.1:9093" },
        { name = "KAFKA_LISTENERS", value = "PLAINTEXT://0.0.0.0:9092,CONTROLLER://0.0.0.0:9093" },
        { name = "KAFKA_ADVERTISED_LISTENERS", value = "PLAINTEXT://127.0.0.1:9092" },
        { name = "KAFKA_CONTROLLER_LISTENER_NAMES", value = "CONTROLLER" },
        { name = "KAFKA_LISTENER_SECURITY_PROTOCOL_MAP", value = "CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT" },
        { name = "KAFKA_INTER_BROKER_LISTENER_NAME", value = "PLAINTEXT" },
        { name = "KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR", value = "1" },
        { name = "KAFKA_TRANSACTION_STATE_LOG_REPLICATION_FACTOR", value = "1" },
        { name = "KAFKA_TRANSACTION_STATE_LOG_MIN_ISR", value = "1" },
        { name = "KAFKA_AUTO_CREATE_TOPICS_ENABLE", value = "false" },
        { name = "KAFKA_DELETE_TOPIC_ENABLE", value = "true" },
        { name = "KAFKA_NUM_PARTITIONS", value = "8" },
      ]
      mountPoints = [
        {
          sourceVolume  = "kafka-data"
          containerPath = "/var/lib/kafka/data"
          readOnly      = false
        },
      ]
      healthCheck = {
        command     = ["CMD-SHELL", "kafka-broker-api-versions --bootstrap-server 127.0.0.1:9092 >/dev/null 2>&1 || exit 1"]
        interval    = 10
        timeout     = 5
        retries     = 10
        startPeriod = 30
      }
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          awslogs-group         = aws_cloudwatch_log_group.benchmark.name
          awslogs-region        = var.aws_region
          awslogs-stream-prefix = "kafka"
        }
      }
    },
    {
      name                   = "benchmark-runner"
      image                  = local.benchmark_runner_image
      essential              = true
      cpu                    = 5120
      memory                 = 8192
      memoryReservation      = 6144
      readonlyRootFilesystem = false
      privileged             = false
      command                = var.benchmark_runner_command
      dependsOn = [{
        containerName = "kafka"
        condition     = "HEALTHY"
      }]
      environment = [
        { name = "AWS_REGION", value = var.aws_region },
        { name = "AWS_DEFAULT_REGION", value = var.aws_region },
        { name = "BENCHMARK_ARTIFACT_BUCKET", value = aws_s3_bucket.artifacts.id },
        { name = "BENCHMARK_RUN_ID", value = var.benchmark_run_id },
        { name = "BENCHMARK_GIT_SHA", value = var.source_revision },
        { name = "BENCHMARK_EXPIRES_AT", value = var.expires_at },
        { name = "BENCHMARK_EXECUTION_MODE", value = "direct" },
        { name = "BENCHMARK_KAFKA_BOOTSTRAP", value = "127.0.0.1:9092" },
        { name = "BENCHMARK_KAFKA_DATA_DIR", value = "/var/lib/kafka/data" },
      ]
      mountPoints = [
        {
          sourceVolume  = "benchmark-control"
          containerPath = "/results"
          readOnly      = false
        },
        {
          sourceVolume  = "kafka-data"
          containerPath = "/var/lib/kafka/data"
          readOnly      = true
        },
      ]
      linuxParameters = {
        initProcessEnabled = true
        capabilities = {
          drop = ["ALL"]
        }
      }
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          awslogs-group         = aws_cloudwatch_log_group.benchmark.name
          awslogs-region        = var.aws_region
          awslogs-stream-prefix = "runner"
        }
      }
    },
  ])

  tags = {
    Name = var.project_name
  }

  lifecycle {
    precondition {
      condition = (
        !var.provision_runtime ||
        (
          var.benchmark_runner_image != null &&
          can(regex("@sha256:[0-9a-f]{64}$", var.benchmark_runner_image))
        )
      )
      error_message = "provision_runtime=true requires benchmark_runner_image pinned by ECR sha256 digest."
    }
  }
}
