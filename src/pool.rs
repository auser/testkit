use std::time::Duration;

/// Configuration for a connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    pub max_size: usize,
    /// Minimum number of idle connections to maintain
    pub min_idle: Option<usize>,
    /// Maximum lifetime of a connection
    pub max_lifetime: Option<Duration>,
    /// Maximum time to wait for a connection
    pub connection_timeout: Duration,
    /// Maximum time a connection can be idle
    pub idle_timeout: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            min_idle: None,
            max_lifetime: Some(Duration::from_secs(30 * 60)), // 30 minutes
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(10 * 60)), // 10 minutes
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration with the given maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            ..Default::default()
        }
    }

    /// Set the minimum number of idle connections
    pub fn min_idle(mut self, min_idle: usize) -> Self {
        self.min_idle = Some(min_idle);
        self
    }

    /// Set the maximum lifetime of a connection
    pub fn max_lifetime(mut self, max_lifetime: Duration) -> Self {
        self.max_lifetime = Some(max_lifetime);
        self
    }

    /// Set the connection timeout
    pub fn connection_timeout(mut self, connection_timeout: Duration) -> Self {
        self.connection_timeout = connection_timeout;
        self
    }

    /// Set the idle timeout
    pub fn idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = Some(idle_timeout);
        self
    }
}

impl PoolConfig {
    /// Create a new builder with the given maximum size
    pub fn builder(max_size: usize) -> PoolConfigBuilder {
        PoolConfigBuilder {
            config: Self::new(max_size),
        }
    }
}

/// Builder for pool configuration
pub struct PoolConfigBuilder {
    config: PoolConfig,
}

impl PoolConfigBuilder {
    /// Set the minimum number of idle connections
    pub fn min_idle(mut self, min_idle: usize) -> Self {
        self.config.min_idle = Some(min_idle);
        self
    }

    /// Set the maximum lifetime of a connection
    pub fn max_lifetime(mut self, max_lifetime: Duration) -> Self {
        self.config.max_lifetime = Some(max_lifetime);
        self
    }

    /// Set the connection timeout
    pub fn connection_timeout(mut self, connection_timeout: Duration) -> Self {
        self.config.connection_timeout = connection_timeout;
        self
    }

    /// Set the idle timeout
    pub fn idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.config.idle_timeout = Some(idle_timeout);
        self
    }

    /// Build the pool configuration
    pub fn build(self) -> PoolConfig {
        self.config
    }
}
