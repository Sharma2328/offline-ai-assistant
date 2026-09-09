//! Hardware inspection (FR-ONB-001).
//!
//! Detects OS/architecture, CPU, memory, disk, and (where inferable) GPU backend — all
//! **locally, with no network** (FR-ONB-001 acceptance (a)/(c)). Any field that cannot be
//! read is recorded in [`HardwareInfo::undetected`] so the UI can explain the gap rather
//! than showing a misleading zero (acceptance (b)).

use std::path::Path;

use serde::{Deserialize, Serialize};
use specta::Type;
use sysinfo::{Disks, System};

/// GPU / compute backend family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum GpuBackend {
    Metal,
    Cuda,
    Vulkan,
    Cpu,
}

/// A detected GPU (or accelerator). `vram_bytes` is `None` on unified-memory systems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    pub backend: GpuBackend,
    pub vram_bytes: Option<u64>,
}

/// Local hardware snapshot (contract §8.1). Numeric fields are `None` when undetectable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub os: String,
    pub arch: String,
    pub cpu_model: Option<String>,
    pub logical_cores: Option<u32>,
    pub total_memory_bytes: Option<u64>,
    pub available_memory_bytes: Option<u64>,
    pub gpus: Vec<Gpu>,
    pub available_disk_bytes: Option<u64>,
    /// Names of fields that could not be detected on this system (FR-ONB-001).
    pub undetected: Vec<String>,
}

/// Detect the local hardware. `disk_probe_path`, when supplied, selects the volume whose
/// free space is reported (normally the app-data directory's volume).
pub fn detect_hardware(disk_probe_path: Option<&Path>) -> HardwareInfo {
    let mut undetected = Vec::new();

    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let cpu_model = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty());
    if cpu_model.is_none() {
        undetected.push("cpuModel".to_string());
    }

    let logical_cores = u32::try_from(sys.cpus().len()).ok().filter(|&n| n > 0);
    if logical_cores.is_none() {
        undetected.push("logicalCores".to_string());
    }

    let total_memory_bytes = non_zero(sys.total_memory());
    if total_memory_bytes.is_none() {
        undetected.push("totalMemoryBytes".to_string());
    }

    let available_memory_bytes = non_zero(sys.available_memory());
    if available_memory_bytes.is_none() {
        undetected.push("availableMemoryBytes".to_string());
    }

    let available_disk_bytes = detect_available_disk(disk_probe_path);
    if available_disk_bytes.is_none() {
        undetected.push("availableDiskBytes".to_string());
    }

    let gpus = detect_gpus(&os, &arch, cpu_model.as_deref());
    if gpus.is_empty() {
        // We can't enumerate discrete GPUs without platform-specific probing yet.
        undetected.push("gpus".to_string());
    }

    HardwareInfo {
        os,
        arch,
        cpu_model,
        logical_cores,
        total_memory_bytes,
        available_memory_bytes,
        gpus,
        available_disk_bytes,
        undetected,
    }
}

/// Treat a reported `0` as "unknown" — sysinfo returns 0 when a value is unavailable.
fn non_zero(value: u64) -> Option<u64> {
    (value != 0).then_some(value)
}

/// Free space on the volume containing `probe`, else the largest free space across volumes.
fn detect_available_disk(probe: Option<&Path>) -> Option<u64> {
    let disks = Disks::new_with_refreshed_list();
    if disks.is_empty() {
        return None;
    }

    if let Some(path) = probe {
        // Longest mount-point prefix wins (handles nested mounts).
        let best = disks
            .iter()
            .filter(|disk| path.starts_with(disk.mount_point()))
            .max_by_key(|disk| disk.mount_point().as_os_str().len());
        if let Some(disk) = best {
            return non_zero(disk.available_space());
        }
    }

    disks
        .iter()
        .map(sysinfo::Disk::available_space)
        .max()
        .and_then(non_zero)
}

/// Best-effort GPU inference. On Apple Silicon the integrated GPU uses Metal over unified
/// memory (no separate VRAM). Discrete-GPU enumeration on other platforms is deferred.
fn detect_gpus(os: &str, arch: &str, cpu_model: Option<&str>) -> Vec<Gpu> {
    if os == "macos" && arch == "aarch64" {
        let name = cpu_model
            .filter(|m| m.contains("Apple"))
            .map(str::to_string)
            .unwrap_or_else(|| "Apple GPU".to_string());
        return vec![Gpu {
            name,
            backend: GpuBackend::Metal,
            vram_bytes: None,
        }];
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_os_and_arch_offline() {
        let info = detect_hardware(None);
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        // os/arch are compile-time constants and must never be reported as undetected.
        assert!(!info.undetected.contains(&"os".to_string()));
    }

    #[test]
    fn undetected_lists_gpus_on_non_apple_silicon() {
        // We can't assert real hardware, but the GPU detector is a pure function.
        assert!(detect_gpus("linux", "x86_64", Some("Some CPU")).is_empty());
    }

    #[test]
    fn apple_silicon_reports_a_metal_gpu_with_unified_memory() {
        let gpus = detect_gpus("macos", "aarch64", Some("Apple M2 Pro"));
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].backend, GpuBackend::Metal);
        assert_eq!(gpus[0].name, "Apple M2 Pro");
        assert!(gpus[0].vram_bytes.is_none());
    }

    #[test]
    fn non_zero_maps_zero_to_none() {
        assert_eq!(non_zero(0), None);
        assert_eq!(non_zero(42), Some(42));
    }

    #[test]
    fn gpu_backend_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&GpuBackend::Metal).unwrap(),
            "\"metal\""
        );
    }
}
