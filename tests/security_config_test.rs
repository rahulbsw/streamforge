use streamforge::config::{MirrorMakerConfig, SecurityProtocol, SaslMechanism};

#[test]
fn test_ssl_config_parsing() {
    let yaml = r#"
appid: test
bootstrap: kafka:9093
input: test-topic
output: output-topic
security:
  protocol: SSL
  ssl:
    ca_location: /path/to/ca.pem
    certificate_location: /path/to/cert.pem
    key_location: /path/to/key.pem
    key_password: password123
    endpoint_identification_algorithm: https
"#;

    let config: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(config.security.is_some());
    let security = config.security.unwrap();

    assert!(matches!(security.protocol, SecurityProtocol::Ssl));
    assert!(security.ssl.is_some());

    let ssl = security.ssl.unwrap();
    assert_eq!(ssl.ca_location, Some("/path/to/ca.pem".to_string()));
    assert_eq!(ssl.certificate_location, Some("/path/to/cert.pem".to_string()));
    assert_eq!(ssl.key_location, Some("/path/to/key.pem".to_string()));
    assert_eq!(ssl.key_password, Some("password123".to_string()));
    assert_eq!(ssl.endpoint_identification_algorithm, Some("https".to_string()));
}

#[test]
fn test_sasl_plain_config_parsing() {
    let yaml = r#"
appid: test
bootstrap: kafka:9093
input: test-topic
output: output-topic
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca.pem
  sasl:
    mechanism: PLAIN
    username: myuser
    password: mypass
"#;

    let config: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(config.security.is_some());
    let security = config.security.unwrap();

    assert!(matches!(security.protocol, SecurityProtocol::SaslSsl));
    assert!(security.sasl.is_some());

    let sasl = security.sasl.unwrap();
    assert!(matches!(sasl.mechanism, SaslMechanism::Plain));
    assert_eq!(sasl.username, Some("myuser".to_string()));
    assert_eq!(sasl.password, Some("mypass".to_string()));
}

#[test]
fn test_sasl_scram_config_parsing() {
    let yaml = r#"
appid: test
bootstrap: kafka:9093
input: test-topic
output: output-topic
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca.pem
  sasl:
    mechanism: SCRAM-SHA-256
    username: myuser
    password: mypass
"#;

    let config: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(config.security.is_some());
    let security = config.security.unwrap();

    let sasl = security.sasl.unwrap();
    assert!(matches!(sasl.mechanism, SaslMechanism::ScramSha256));
}

#[test]
fn test_kerberos_config_parsing() {
    let yaml = r#"
appid: test
bootstrap: kafka:9093
input: test-topic
output: output-topic
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca.pem
  sasl:
    mechanism: GSSAPI
    kerberos_service_name: kafka
    kerberos_principal: client@REALM
    kerberos_keytab: /path/to/keytab
"#;

    let config: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(config.security.is_some());
    let security = config.security.unwrap();

    let sasl = security.sasl.unwrap();
    assert!(matches!(sasl.mechanism, SaslMechanism::Gssapi));
    assert_eq!(sasl.kerberos_service_name, Some("kafka".to_string()));
    assert_eq!(sasl.kerberos_principal, Some("client@REALM".to_string()));
    assert_eq!(sasl.kerberos_keytab, Some("/path/to/keytab".to_string()));
}

#[test]
fn test_json_security_config() {
    let json = r#"
{
  "appid": "test",
  "bootstrap": "kafka:9093",
  "input": "test-topic",
  "output": "output-topic",
  "security": {
    "protocol": "SASL_SSL",
    "ssl": {
      "ca_location": "/path/to/ca.pem"
    },
    "sasl": {
      "mechanism": "SCRAM-SHA-512",
      "username": "user",
      "password": "pass"
    }
  }
}
"#;

    let config: MirrorMakerConfig = serde_json::from_str(json).unwrap();

    assert!(config.security.is_some());
    let security = config.security.unwrap();

    let sasl = security.sasl.unwrap();
    assert!(matches!(sasl.mechanism, SaslMechanism::ScramSha512));
}

#[test]
fn test_no_security_config() {
    let yaml = r#"
appid: test
bootstrap: kafka:9092
input: test-topic
output: output-topic
"#;

    let config: MirrorMakerConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.security.is_none());
}
