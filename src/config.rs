use std::{str::FromStr, net::{Ipv4Addr, SocketAddrV4}, time::Duration};

use thiserror::Error;

const PING_INTERVAL_MIN_MS: u64 = 1;
const PING_INTERVAL_MAX_MS: u64 = 1000;

const PING_COUNT_MIN: u16 = 1;
const PING_COUNT_MAX: u16 = 10;

pub const ICMP_ECHO_TIMEOUT_DURATION_SECONDS: Duration = Duration::from_secs(5);

#[derive(Error, Debug, PartialEq)]
pub enum ConfigError {
    #[error("Number is out of range")]
    OutOfRange,

    #[error("Ping interval is out of range: {0}")]
    PingIntervalOutOfRange(u64),
    
    #[error("Ping count is out of range: {0}")]
    PingCountOutOfRange(u16),

    #[error("Parse config error: {0}")]
    ParseError(String),

    #[error("Invalid IPv4 syntax")]
    InvalidIPv4Syntax,

    #[error("Invalid ping_interval syntax")]
    InvalidPingIntervalSyntax,

    #[error("Invalid ping_count syntax")]
    InvalidPingCountSyntax,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub target_addr_ipv4: SocketAddrV4,
    pub ping_count: u16,
    pub ping_interval: u64,
}

impl Config {
    /// Constructs a new Config instance
    pub fn new(target_addr_ipv4: SocketAddrV4, ping_count: u16, ping_interval: u64) -> Result<Config, ConfigError> {
        let ping_count = Config::check_range(ping_count, PING_COUNT_MIN, PING_COUNT_MAX)
            .map_err(|_| ConfigError::PingCountOutOfRange(ping_count))?;
        let ping_interval = Config::check_range(ping_interval, PING_INTERVAL_MIN_MS, PING_INTERVAL_MAX_MS)
            .map_err(|_| ConfigError::PingIntervalOutOfRange(ping_interval))?;
        Ok(Config {
            target_addr_ipv4,  ping_count, ping_interval
        })
    }

    /// Validates that the number is in the given range
    fn check_range<T>(number: T, lower: T, upper: T) -> Result<T, ConfigError> 
    where 
        T: PartialOrd + Copy
    {
        if number >= lower && number <= upper {
            Ok(number)
        } else {
            Err(ConfigError::OutOfRange)
        }
    }
}

impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}:{}:{}", self.target_addr_ipv4.ip(), self.target_addr_ipv4.port(), self.ping_count, self.ping_interval)
    }
}

impl FromStr for Config {
    type Err = ConfigError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.splitn(3, ',');
        let ipv4_s = iter
            .next()
            .ok_or(ConfigError::ParseError("Column Not Found".to_string()))?;
        let ip = ipv4_s.parse::<Ipv4Addr>().map_err(|_| ConfigError::InvalidIPv4Syntax)?;
        let target_addr_ipv4 = SocketAddrV4::new(ip, 8080);
        
        let ping_count = iter
            .next()
            .ok_or(ConfigError::ParseError("Column Not Found".to_string()))?
            .parse()
            .map_err(|_| ConfigError::InvalidPingCountSyntax)?;
        let ping_count = Config::check_range(ping_count, PING_COUNT_MIN, PING_COUNT_MAX)
            .map_err(|_| ConfigError::PingCountOutOfRange(ping_count))?;
        let ping_interval = iter
            .next()
            .ok_or(ConfigError::ParseError("Column Not Found".to_string()))?
            .parse()
            .map_err(|_| ConfigError::InvalidPingIntervalSyntax)?;
        let ping_interval = Config::check_range(ping_interval, PING_INTERVAL_MIN_MS, PING_INTERVAL_MAX_MS)
            .map_err(|_| ConfigError::PingIntervalOutOfRange(ping_interval))?;
        Ok(Config {
            target_addr_ipv4,
            ping_count,
            ping_interval,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let c_left = "0.0.0.0,5,1000".parse::<Config>().unwrap();
        let c_right = Config::new({
            let ipv4 = Ipv4Addr::new(0, 0, 0, 0);
            SocketAddrV4::new(ipv4, 8080)
        }, 5, 1000).expect("Config expected to be valid in the test_parse_config unit test");
        assert_eq!(c_left, c_right);
    }

    #[test]
    fn test_check_range() {
        // List of ping count assertions
        assert_eq!(Config::check_range(1, PING_COUNT_MIN, PING_COUNT_MAX), Ok(1));
        assert_eq!(Config::check_range(10, PING_COUNT_MIN, PING_COUNT_MAX), Ok(10));
        assert_eq!(Config::check_range(0, PING_COUNT_MIN, PING_COUNT_MAX), Err(ConfigError::OutOfRange));
        assert_eq!(Config::check_range(11, PING_COUNT_MIN, PING_COUNT_MAX), Err(ConfigError::OutOfRange));

        // List of ping interval assertions
        assert_eq!(Config::check_range(500, PING_INTERVAL_MIN_MS, PING_INTERVAL_MAX_MS), Ok(500));
        assert_eq!(Config::check_range(PING_INTERVAL_MIN_MS, PING_INTERVAL_MIN_MS, PING_INTERVAL_MAX_MS), Ok(PING_INTERVAL_MIN_MS));
        assert_eq!(Config::check_range(PING_INTERVAL_MAX_MS, PING_INTERVAL_MIN_MS, PING_INTERVAL_MAX_MS), Ok(PING_INTERVAL_MAX_MS));
        assert_eq!(Config::check_range(0, PING_INTERVAL_MIN_MS, PING_INTERVAL_MAX_MS), Err(ConfigError::OutOfRange));
        assert_eq!(Config::check_range(1001, PING_INTERVAL_MIN_MS, PING_INTERVAL_MAX_MS), Err(ConfigError::OutOfRange));
    }

    #[test]
    fn test_parse_config_error() {
        // Bad IPv4 address
        assert_eq!("0.0.0.A,5,1000".parse::<Config>(), Err(ConfigError::InvalidIPv4Syntax));

        // Bad ping count number
        assert_eq!("0.0.0.0,B,1000".parse::<Config>(), Err(ConfigError::InvalidPingCountSyntax));

        // Bad ping interval number
        assert_eq!("0.0.0.0,5,C".parse::<Config>(), Err(ConfigError::InvalidPingIntervalSyntax));

        // Ping count is out of range
        assert_eq!("0.0.0.0,0,1000".parse::<Config>(), Err(ConfigError::PingCountOutOfRange(0)));

        // Ping interval is out of range
        assert_eq!("0.0.0.0,1,0".parse::<Config>(), Err(ConfigError::PingIntervalOutOfRange(0)));
    }
}
