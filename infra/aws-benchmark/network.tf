resource "aws_vpc" "benchmark" {
  cidr_block           = var.vpc_cidr
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = {
    Name = var.project_name
  }
}

resource "aws_subnet" "private" {
  vpc_id                  = aws_vpc.benchmark.id
  cidr_block              = var.private_subnet_cidr
  availability_zone       = var.availability_zone
  map_public_ip_on_launch = false

  tags = {
    Name = "${var.project_name}-private"
    Tier = "private"
  }
}

resource "aws_route_table" "private" {
  vpc_id = aws_vpc.benchmark.id

  tags = {
    Name = "${var.project_name}-private"
  }
}

resource "aws_route_table_association" "private" {
  subnet_id      = aws_subnet.private.id
  route_table_id = aws_route_table.private.id
}

resource "aws_security_group" "instance" {
  name                   = "${var.project_name}-instance"
  description            = "No-ingress ECS container-instance security group"
  vpc_id                 = aws_vpc.benchmark.id
  revoke_rules_on_delete = true

  tags = {
    Name = "${var.project_name}-instance"
  }
}

resource "aws_security_group" "task" {
  name                   = "${var.project_name}-task"
  description            = "No-ingress benchmark task security group"
  vpc_id                 = aws_vpc.benchmark.id
  revoke_rules_on_delete = true

  tags = {
    Name = "${var.project_name}-task"
  }
}

resource "aws_security_group" "endpoints" {
  name                   = "${var.project_name}-endpoints"
  description            = "PrivateLink HTTPS only from benchmark workloads"
  vpc_id                 = aws_vpc.benchmark.id
  revoke_rules_on_delete = true

  tags = {
    Name = "${var.project_name}-endpoints"
  }
}

resource "aws_vpc_security_group_ingress_rule" "endpoint_from_instance" {
  security_group_id            = aws_security_group.endpoints.id
  referenced_security_group_id = aws_security_group.instance.id
  from_port                    = 443
  to_port                      = 443
  ip_protocol                  = "tcp"
  description                  = "HTTPS from the ECS container instance"
}

resource "aws_vpc_security_group_ingress_rule" "endpoint_from_task" {
  security_group_id            = aws_security_group.endpoints.id
  referenced_security_group_id = aws_security_group.task.id
  from_port                    = 443
  to_port                      = 443
  ip_protocol                  = "tcp"
  description                  = "HTTPS from the benchmark task"
}

resource "aws_vpc_security_group_egress_rule" "instance_to_endpoints" {
  security_group_id            = aws_security_group.instance.id
  referenced_security_group_id = aws_security_group.endpoints.id
  from_port                    = 443
  to_port                      = 443
  ip_protocol                  = "tcp"
  description                  = "HTTPS to private AWS interface endpoints"
}

resource "aws_vpc_security_group_egress_rule" "task_to_endpoints" {
  security_group_id            = aws_security_group.task.id
  referenced_security_group_id = aws_security_group.endpoints.id
  from_port                    = 443
  to_port                      = 443
  ip_protocol                  = "tcp"
  description                  = "HTTPS to private AWS interface endpoints"
}

data "aws_prefix_list" "s3" {
  name = "com.amazonaws.${var.aws_region}.s3"
}

resource "aws_vpc_security_group_egress_rule" "instance_to_s3" {
  security_group_id = aws_security_group.instance.id
  prefix_list_id    = data.aws_prefix_list.s3.id
  from_port         = 443
  to_port           = 443
  ip_protocol       = "tcp"
  description       = "HTTPS to S3 through the gateway endpoint"
}

resource "aws_vpc_security_group_egress_rule" "task_to_s3" {
  security_group_id = aws_security_group.task.id
  prefix_list_id    = data.aws_prefix_list.s3.id
  from_port         = 443
  to_port           = 443
  ip_protocol       = "tcp"
  description       = "HTTPS to S3 through the gateway endpoint"
}

resource "aws_vpc_endpoint" "interface" {
  for_each = var.provision_runtime ? local.interface_endpoint_services : toset([])

  vpc_id              = aws_vpc.benchmark.id
  service_name        = "com.amazonaws.${var.aws_region}.${each.value}"
  vpc_endpoint_type   = "Interface"
  subnet_ids          = [aws_subnet.private.id]
  security_group_ids  = [aws_security_group.endpoints.id]
  private_dns_enabled = true

  tags = {
    Name = "${var.project_name}-${replace(each.value, ".", "-")}"
  }
}

resource "aws_vpc_endpoint" "s3" {
  vpc_id            = aws_vpc.benchmark.id
  service_name      = "com.amazonaws.${var.aws_region}.s3"
  vpc_endpoint_type = "Gateway"
  route_table_ids   = [aws_route_table.private.id]

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid       = "BenchmarkArtifacts"
        Effect    = "Allow"
        Principal = "*"
        Action = [
          "s3:AbortMultipartUpload",
          "s3:DeleteObject",
          "s3:GetBucketLocation",
          "s3:GetObject",
          "s3:ListBucket",
          "s3:ListBucketMultipartUploads",
          "s3:ListMultipartUploadParts",
          "s3:PutObject",
        ]
        Resource = [
          aws_s3_bucket.artifacts.arn,
          "${aws_s3_bucket.artifacts.arn}/*",
        ]
      },
      {
        Sid       = "EcrLayerDownloads"
        Effect    = "Allow"
        Principal = "*"
        Action    = ["s3:GetObject"]
        Resource  = "arn:${data.aws_partition.current.partition}:s3:::prod-${var.aws_region}-starport-layer-bucket/*"
      },
    ]
  })

  tags = {
    Name = "${var.project_name}-s3"
  }
}
