use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorMakerConfig {
    /// Application ID
    pub appid: String,

    /// Source Kafka bootstrap servers
    pub bootstrap: String,

    /// Input topic(s) - comma-separated
    pub input: String,

    /// Output topic (for single destination)
    pub output: Option<String>,

    /// Target broker (for cross-cluster mirroring)
    #[serde(default)]
    pub target_broker: Option<String>,

    /// Consumer offset reset strategy
    #[serde(default = "default_offset")]
    pub offset: String,

    /// Number of processing threads
    #[serde(default = "default_threads")]
    pub threads: usize,

    /// Compression configuration
    #[serde(default)]
    pub compression: CompressionConfig,

    /// Multi-destination routing configuration
    pub routing: Option<RoutingConfig>,

    /// Consumer properties
    #[serde(default)]
    pub consumer_properties: HashMap<String, String>,

    /// Producer properties
    #[serde(default)]
    pub producer_properties: HashMap<String, String>,

    /// Security configuration
    #[serde(default)]
    pub security: Option<SecurityConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Security protocol: PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL
    pub protocol: SecurityProtocol,

    /// SSL/TLS configuration
    #[serde(default)]
    pub ssl: Option<SslConfig>,

    /// SASL authentication configuration
    #[serde(default)]
    pub sasl: Option<SaslConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecurityProtocol {
    Plaintext,
    Ssl,
    SaslPlaintext,
    SaslSsl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    /// Path to CA certificate file for verifying broker's certificate
    pub ca_location: Option<String>,

    /// Path to client's certificate file (for mutual TLS)
    pub certificate_location: Option<String>,

    /// Path to client's private key file (for mutual TLS)
    pub key_location: Option<String>,

    /// Password for the private key file
    pub key_password: Option<String>,

    /// Endpoint identification algorithm (default: https)
    pub endpoint_identification_algorithm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaslConfig {
    /// SASL mechanism: PLAIN, SCRAM-SHA-256, SCRAM-SHA-512, GSSAPI, OAUTHBEARER
    pub mechanism: SaslMechanism,

    /// Username (for PLAIN and SCRAM mechanisms)
    pub username: Option<String>,

    /// Password (for PLAIN and SCRAM mechanisms)
    pub password: Option<String>,

    /// Kerberos service name (for GSSAPI)
    pub kerberos_service_name: Option<String>,

    /// Kerberos principal (for GSSAPI)
    pub kerberos_principal: Option<String>,

    /// Path to Kerberos keytab (for GSSAPI)
    pub kerberos_keytab: Option<String>,

    /// OAuth bearer token (for OAUTHBEARER)
    pub oauthbearer_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SaslMechanism {
    #[serde(rename = "PLAIN")]
    Plain,
    #[serde(rename = "SCRAM-SHA-256")]
    ScramSha256,
    #[serde(rename = "SCRAM-SHA-512")]
    ScramSha512,
    #[serde(rename = "GSSAPI")]
    Gssapi,
    #[serde(rename = "OAUTHBEARER")]
    Oauthbearer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    #[serde(default)]
    pub compression_type: CompressionType,

    #[serde(default)]
    pub compression_algo: CompressionAlgo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    #[default]
    None,
    /// Native Kafka compression (recommended)
    Raw,
    /// Enveloped compression (custom format)
    Enveloped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgo {
    #[default]
    Gzip,
    Snappy,
    Zstd,
    Lz4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Routing type: content, filter, or hybrid
    pub routing_type: String,

    /// JSON path for content-based routing
    pub path: Option<String>,

    /// Destination configurations
    pub destinations: Vec<DestinationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationConfig {
    /// Destination topic name
    pub output: String,

    /// Match value for content-based routing
    pub match_value: Option<String>,

    /// Filter expression (simple or composite)
    /// Simple: "path,operator,value" e.g., "/message/siteId,>,10000"
    /// Composite JSON for AND/OR/NOT (parsed separately)
    pub filter: Option<String>,

    /// Transform expression
    /// Simple path: "/message" or "/message/confId"
    /// Object construction JSON (parsed separately)
    pub transform: Option<String>,

    /// Partition field JSON path
    pub partition: Option<String>,

    /// Broadcast flag for hybrid routing
    #[serde(default)]
    pub broadcast: bool,

    /// Description
    pub description: Option<String>,
}

fn default_offset() -> String {
    "latest".to_string()
}

fn default_threads() -> usize {
    4
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            compression_type: CompressionType::None,
            compression_algo: CompressionAlgo::Gzip,
        }
    }
}

impl MirrorMakerConfig {
    /// Load configuration from file.
    ///
    /// Automatically detects format based on file extension:
    /// - .json → JSON format
    /// - .yaml, .yml → YAML format
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use streamforge::config::MirrorMakerConfig;
    /// let config = MirrorMakerConfig::from_file("config.json").unwrap();
    /// let config = MirrorMakerConfig::from_file("config.yaml").unwrap();
    /// ```
    pub fn from_file(path: &str) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Detect format based on file extension
        let config = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)
                .map_err(|e| crate::error::MirrorMakerError::Config(format!("YAML parse error: {}", e)))?
        } else {
            // Default to JSON for backward compatibility
            serde_json::from_str(&content)
                .map_err(|e| crate::error::MirrorMakerError::Config(format!("JSON parse error: {}", e)))?
        };

        Ok(config)
    }

    pub fn get_target_broker(&self) -> String {
        self.target_broker
            .as_ref()
            .unwrap_or(&self.bootstrap)
            .clone()
    }

    /// Apply security configuration to a Kafka ClientConfig
    pub fn apply_security(&self, client_config: &mut rdkafka::ClientConfig) {
        if let Some(security) = &self.security {
            // Set security protocol
            let protocol = match security.protocol {
                SecurityProtocol::Plaintext => "plaintext",
                SecurityProtocol::Ssl => "ssl",
                SecurityProtocol::SaslPlaintext => "sasl_plaintext",
                SecurityProtocol::SaslSsl => "sasl_ssl",
            };
            client_config.set("security.protocol", protocol);

            // Apply SSL configuration
            if let Some(ssl) = &security.ssl {
                if let Some(ca_location) = &ssl.ca_location {
                    client_config.set("ssl.ca.location", ca_location);
                }
                if let Some(cert_location) = &ssl.certificate_location {
                    client_config.set("ssl.certificate.location", cert_location);
                }
                if let Some(key_location) = &ssl.key_location {
                    client_config.set("ssl.key.location", key_location);
                }
                if let Some(key_password) = &ssl.key_password {
                    client_config.set("ssl.key.password", key_password);
                }
                if let Some(endpoint_id) = &ssl.endpoint_identification_algorithm {
                    client_config.set("ssl.endpoint.identification.algorithm", endpoint_id);
                }
            }

            // Apply SASL configuration
            if let Some(sasl) = &security.sasl {
                let mechanism = match sasl.mechanism {
                    SaslMechanism::Plain => "PLAIN",
                    SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
                    SaslMechanism::ScramSha512 => "SCRAM-SHA-512",
                    SaslMechanism::Gssapi => "GSSAPI",
                    SaslMechanism::Oauthbearer => "OAUTHBEARER",
                };
                client_config.set("sasl.mechanism", mechanism);

                if let Some(username) = &sasl.username {
                    client_config.set("sasl.username", username);
                }
                if let Some(password) = &sasl.password {
                    client_config.set("sasl.password", password);
                }
                if let Some(service_name) = &sasl.kerberos_service_name {
                    client_config.set("sasl.kerberos.service.name", service_name);
                }
                if let Some(principal) = &sasl.kerberos_principal {
                    client_config.set("sasl.kerberos.principal", principal);
                }
                if let Some(keytab) = &sasl.kerberos_keytab {
                    client_config.set("sasl.kerberos.keytab", keytab);
                }
                if let Some(token) = &sasl.oauthbearer_token {
                    client_config.set("sasl.oauthbearer.token", token);
                }
            }
        }
    }
}
