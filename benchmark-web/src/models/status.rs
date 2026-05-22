//! RunStatus enum - mirrors Java RunStatus.java

use serde::{Deserialize, Serialize};
use std::fmt;

/// Status of a benchmark run / queue item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    PENDING,
    RUNNING,
    COMPLETED,
    FAILED,
    CANCELLED,
}

impl RunStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::PENDING | Self::RUNNING)
    }
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PENDING => write!(f, "PENDING"),
            Self::RUNNING => write!(f, "RUNNING"),
            Self::COMPLETED => write!(f, "COMPLETED"),
            Self::FAILED => write!(f, "FAILED"),
            Self::CANCELLED => write!(f, "CANCELLED"),
        }
    }
}
