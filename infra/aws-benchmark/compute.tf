data "aws_ssm_parameter" "ecs_optimized_ami" {
  name = "/aws/service/ecs/optimized-ami/amazon-linux-2023/recommended/image_id"
}

resource "aws_ecs_cluster" "benchmark" {
  name = var.project_name

  setting {
    name  = "containerInsights"
    value = "disabled"
  }

  tags = {
    Name = var.project_name
  }
}

resource "aws_launch_template" "benchmark" {
  name_prefix   = "${var.project_name}-"
  image_id      = data.aws_ssm_parameter.ecs_optimized_ami.value
  instance_type = var.instance_type

  iam_instance_profile {
    arn = aws_iam_instance_profile.ecs.arn
  }

  network_interfaces {
    associate_public_ip_address = false
    delete_on_termination       = true
    device_index                = 0
    security_groups             = [aws_security_group.instance.id]
  }

  block_device_mappings {
    device_name = "/dev/xvda"

    ebs {
      delete_on_termination = true
      encrypted             = true
      volume_size           = 40
      volume_type           = "gp3"
      iops                  = 3000
      throughput            = 125
    }
  }

  metadata_options {
    http_endpoint               = "enabled"
    http_protocol_ipv6          = "disabled"
    http_put_response_hop_limit = 2
    http_tokens                 = "required"
    instance_metadata_tags      = "enabled"
  }

  monitoring {
    enabled = false
  }

  user_data = base64encode(templatefile(
    "${path.module}/templates/ecs-user-data.sh.tftpl",
    { cluster_name = aws_ecs_cluster.benchmark.name },
  ))

  update_default_version = true

  tag_specifications {
    resource_type = "instance"
    tags = merge(local.common_tags, {
      Name = "${var.project_name}-ecs"
    })
  }

  tag_specifications {
    resource_type = "volume"
    tags = merge(local.common_tags, {
      Name = "${var.project_name}-root"
    })
  }

  tags = {
    Name = "${var.project_name}-launch-template"
  }
}

resource "aws_autoscaling_group" "benchmark" {
  name_prefix         = "${var.project_name}-"
  min_size            = var.provision_runtime ? 1 : 0
  max_size            = var.provision_runtime ? 1 : 0
  desired_capacity    = var.provision_runtime ? 1 : 0
  vpc_zone_identifier = [aws_subnet.private.id]

  health_check_type         = "EC2"
  health_check_grace_period = 300
  default_cooldown          = 30
  force_delete              = true
  wait_for_capacity_timeout = "10m"

  launch_template {
    id      = aws_launch_template.benchmark.id
    version = "$Latest"
  }

  dynamic "tag" {
    for_each = merge(local.common_tags, {
      Name = "${var.project_name}-ecs"
    })
    content {
      key                 = tag.key
      value               = tag.value
      propagate_at_launch = true
    }
  }

}

resource "aws_autoscaling_schedule" "hard_expiry" {
  count = var.provision_runtime ? 1 : 0

  scheduled_action_name  = "${var.project_name}-hard-expiry"
  autoscaling_group_name = aws_autoscaling_group.benchmark.name
  start_time             = var.expires_at
  min_size               = 0
  max_size               = 0
  desired_capacity       = 0
}
