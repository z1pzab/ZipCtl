use std::collections::HashSet;
use std::time::Instant;
use sysinfo::{
    Components, CpuRefreshKind, DiskKind, Disks, MemoryRefreshKind, Networks, Pid,
    ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System,
};

use crate::platform;

#[derive(Debug, Clone)]
pub struct DiskStatus {
    pub name: String,
    pub mount_point: String,
    pub disk_kind: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub fs_type: String,
    pub low_space: bool,
}

impl DiskStatus {
    pub fn ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessSort {
    Cpu,
    Memory,
}

impl ProcessSort {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cpu => "ЦП %",
            Self::Memory => "ОЗУ",
        }
    }
}

pub struct SysMonitor {
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    low_space_bytes: u64,
    last_refresh: Instant,

    pub cpu_usage: f32,
    pub per_core_usage: Vec<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub cpu_temp: Option<f32>,
    pub gpu_temp: Option<f32>,
    pub rx_rate: f64,
    pub tx_rate: f64,
    pub rx_total: u64,
    pub tx_total: u64,
    pub disks_status: Vec<DiskStatus>,
    pub processes: Vec<ProcessInfo>,
    pub sort_mode: ProcessSort,
}

impl SysMonitor {
    pub fn new(low_space_gb: u64) -> Self {
        let specifics = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything())
            .with_processes(ProcessRefreshKind::nothing().with_cpu().with_memory());

        let system = System::new_with_specifics(specifics);
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();

        let mut monitor = Self {
            system,
            disks,
            networks,
            components,
            low_space_bytes: low_space_gb.saturating_mul(1024 * 1024 * 1024),
            last_refresh: Instant::now(),
            cpu_usage: 0.0,
            per_core_usage: Vec::new(),
            mem_used: 0,
            mem_total: 0,
            swap_used: 0,
            swap_total: 0,
            cpu_temp: None,
            gpu_temp: None,
            rx_rate: 0.0,
            tx_rate: 0.0,
            rx_total: 0,
            tx_total: 0,
            disks_status: Vec::new(),
            processes: Vec::new(),
            sort_mode: ProcessSort::Cpu,
        };
        monitor.refresh();
        monitor
    }

    pub fn low_space_gb(&self) -> u64 {
        self.low_space_bytes / (1024 * 1024 * 1024)
    }

    pub fn set_low_space_gb(&mut self, gb: u64) {
        self.low_space_bytes = gb.saturating_mul(1024 * 1024 * 1024);
        self.refresh_disks();
    }

    pub fn refresh(&mut self) {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(0.1);
        self.last_refresh = Instant::now();

        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        self.disks.refresh(true);
        self.networks.refresh(true);
        self.components.refresh(true);

        self.cpu_usage = self.system.global_cpu_usage();
        self.per_core_usage = self.system.cpus().iter().map(|c| c.cpu_usage()).collect();

        self.mem_used = self.system.used_memory();
        self.mem_total = self.system.total_memory();
        self.swap_used = self.system.used_swap();
        self.swap_total = self.system.total_swap();

        self.refresh_temperatures();
        self.refresh_network(elapsed);
        self.refresh_disks();
        self.refresh_processes();
    }

    fn refresh_temperatures(&mut self) {
        let mut cpu_t: Option<f32> = None;
        let mut gpu_t: Option<f32> = None;

        for comp in &self.components {
            let label = comp.label().to_lowercase();
            if let Some(temp) = comp.temperature() {
                if temp <= 0.0 || temp > 130.0 {
                    continue;
                }
                if gpu_t.is_none()
                    && (label.contains("gpu")
                        || label.contains("amdgpu")
                        || label.contains("nvidia")
                        || label.contains("edge")
                        || label.contains("junction"))
                {
                    gpu_t = Some(temp);
                } else if cpu_t.is_none()
                    && (label.contains("tctl")
                        || label.contains("tdie")
                        || label.contains("package")
                        || label.contains("core 0")
                        || label.contains("cpu")
                        || label.contains("k10temp")
                        || label.contains("coretemp"))
                {
                    cpu_t = Some(temp);
                }
            }
        }

        if cpu_t.is_none() {
            cpu_t = platform::read_cpu_temp_fallback();
        }
        if gpu_t.is_none() {
            gpu_t = platform::read_gpu_temp_fallback();
        }

        self.cpu_temp = cpu_t;
        self.gpu_temp = gpu_t;
    }

    fn refresh_network(&mut self, elapsed_secs: f64) {
        let mut rx_delta: u64 = 0;
        let mut tx_delta: u64 = 0;
        let mut rx_tot: u64 = 0;
        let mut tx_tot: u64 = 0;

        for (iface, data) in &self.networks {
            if iface == "lo" || iface.to_lowercase().contains("loopback") {
                continue;
            }
            rx_delta = rx_delta.saturating_add(data.received());
            tx_delta = tx_delta.saturating_add(data.transmitted());
            rx_tot = rx_tot.saturating_add(data.total_received());
            tx_tot = tx_tot.saturating_add(data.total_transmitted());
        }

        self.rx_rate = rx_delta as f64 / elapsed_secs;
        self.tx_rate = tx_delta as f64 / elapsed_secs;
        self.rx_total = rx_tot;
        self.tx_total = tx_tot;
    }

    fn refresh_disks(&mut self) {
        let mut seen_mounts = HashSet::new();
        let mut list = Vec::new();

        for disk in &self.disks {
            let total = disk.total_space();
            if total == 0 {
                continue;
            }

            let fs_type = disk.file_system().to_string_lossy().trim().to_string();
            if is_virtual_filesystem(&fs_type) {
                continue;
            }

            let mount = disk.mount_point().to_string_lossy().into_owned();
            if mount.is_empty() || !seen_mounts.insert(mount.clone()) {
                continue;
            }

            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let disk_kind = match disk.kind() {
                DiskKind::SSD => "SSD".to_string(),
                DiskKind::HDD => "HDD".to_string(),
                DiskKind::Unknown(_) => String::new(),
            };
            let name = disk.name().to_string_lossy().trim().to_string();

            list.push(DiskStatus {
                name,
                mount_point: mount,
                disk_kind,
                total_bytes: total,
                used_bytes: used,
                available_bytes: available,
                fs_type: if fs_type.is_empty() {
                    "—".to_string()
                } else {
                    fs_type
                },
                low_space: available < self.low_space_bytes,
            });
        }

        list.sort_by(|a, b| {
            let a_root = is_primary_mount(&a.mount_point);
            let b_root = is_primary_mount(&b.mount_point);
            b_root
                .cmp(&a_root)
                .then_with(|| a.mount_point.to_lowercase().cmp(&b.mount_point.to_lowercase()))
        });

        self.disks_status = list;
    }

    fn refresh_processes(&mut self) {
        let mut list: Vec<ProcessInfo> = self
            .system
            .processes()
            .iter()
            .map(|(pid, proc_)| ProcessInfo {
                pid: pid.as_u32(),
                name: proc_.name().to_string_lossy().into_owned(),
                cpu_usage: proc_.cpu_usage(),
                memory_bytes: proc_.memory(),
            })
            .collect();

        match self.sort_mode {
            ProcessSort::Cpu => {
                list.sort_by(|a, b| {
                    b.cpu_usage
                        .partial_cmp(&a.cpu_usage)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
                });
            }
            ProcessSort::Memory => {
                list.sort_by(|a, b| {
                    b.memory_bytes
                        .cmp(&a.memory_bytes)
                        .then_with(|| {
                            b.cpu_usage
                                .partial_cmp(&a.cpu_usage)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                });
            }
        }

        list.truncate(60);
        self.processes = list;
    }

    pub fn toggle_sort(&mut self) {
        self.sort_mode = match self.sort_mode {
            ProcessSort::Cpu => ProcessSort::Memory,
            ProcessSort::Memory => ProcessSort::Cpu,
        };
        self.refresh_processes();
    }

    pub fn kill_process(&mut self, pid: u32) -> Result<String, String> {
        let sys_pid = Pid::from_u32(pid);
        if let Some(proc_) = self.system.process(sys_pid) {
            let name = proc_.name().to_string_lossy().into_owned();
            if proc_.kill() {
                self.refresh_processes();
                Ok(format!("Процесс «{name}» (PID {pid}) завершён."))
            } else {
                Err(format!(
                    "Не удалось завершить «{name}» (PID {pid}): недостаточно прав."
                ))
            }
        } else {
            Err(format!("Процесс PID {pid} уже не существует."))
        }
    }
}

fn is_primary_mount(mount: &str) -> bool {
    mount == "/" || mount.eq_ignore_ascii_case("C:\\") || mount.eq_ignore_ascii_case("C:/")
}

/// Filters out Linux pseudo/virtual filesystems so only real partitions remain.
pub fn is_virtual_filesystem(fs_type: &str) -> bool {
    let lower = fs_type.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    matches!(
        lower.as_str(),
        "tmpfs"
            | "devtmpfs"
            | "efivarfs"
            | "proc"
            | "sysfs"
            | "cgroup"
            | "cgroup2"
            | "overlay"
            | "squashfs"
            | "ramfs"
            | "debugfs"
            | "tracefs"
            | "securityfs"
            | "pstore"
            | "autofs"
            | "mqueue"
            | "hugetlbfs"
            | "fusectl"
            | "configfs"
            | "bpf"
            | "nsfs"
            | "devpts"
            | "rpc_pipefs"
            | "binfmt_misc"
            | "fuse.portal"
            | "fuse.gvfsd-fuse"
            | "fuse.lxcfs"
    )
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} ГБ", b / GB)
    } else if b >= MB {
        format!("{:.1} МБ", b / MB)
    } else if b >= KB {
        format!("{:.0} КБ", b / KB)
    } else {
        format!("{bytes} Б")
    }
}

pub fn format_gb(bytes: u64) -> String {
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    format!("{gb:.1} ГБ")
}

pub fn format_rate(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    if bytes_per_sec >= MB {
        format!("{:.1} МБ/с", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1} КБ/с", bytes_per_sec / KB)
    } else {
        format!("{:.0} Б/с", bytes_per_sec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_linux_virtual_filesystems() {
        for fs in ["tmpfs", "devtmpfs", "efivarfs", "proc", "sysfs", "TMPFS", "overlay"] {
            assert!(is_virtual_filesystem(fs), "{fs} should be filtered");
        }
        for fs in ["ext4", "btrfs", "xfs", "NTFS", "vfat", "apfs"] {
            assert!(!is_virtual_filesystem(fs), "{fs} should NOT be filtered");
        }
    }

    #[test]
    fn disk_ratio_is_clamped() {
        let d = DiskStatus {
            name: "System".to_string(),
            mount_point: "/".to_string(),
            disk_kind: "SSD".to_string(),
            total_bytes: 100,
            used_bytes: 40,
            available_bytes: 60,
            fs_type: "ext4".to_string(),
            low_space: false,
        };
        assert!((d.ratio() - 0.4).abs() < f64::EPSILON);

        let empty = DiskStatus {
            total_bytes: 0,
            used_bytes: 0,
            ..d
        };
        assert_eq!(empty.ratio(), 0.0);
    }
}
