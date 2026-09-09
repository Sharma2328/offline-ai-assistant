//! System inspection aggregate (FR-ONB-001, FR-SYS-001) — the `system.inspect` payload.

use std::path::Path;

use inference::{detect_capabilities, RuntimeCapabilities};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::hardware::{detect_hardware, HardwareInfo};

/// Combined hardware + runtime snapshot returned by `system.inspect` (contract §9.1).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemInspection {
    pub hardware: HardwareInfo,
    pub capabilities: RuntimeCapabilities,
}

/// Run a full local inspection. `disk_probe_path` selects the volume for free-space
/// reporting (normally the app-data directory). Performs no network access.
pub fn inspect(disk_probe_path: Option<&Path>) -> SystemInspection {
    SystemInspection {
        hardware: detect_hardware(disk_probe_path),
        capabilities: detect_capabilities(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_populates_hardware_and_capabilities() {
        let snapshot = inspect(None);
        assert!(!snapshot.hardware.os.is_empty());
        // Runtime is not integrated until Phase 5.
        assert!(!snapshot.capabilities.is_available());
    }
}
