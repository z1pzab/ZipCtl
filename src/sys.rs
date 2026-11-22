use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;
use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, Pid, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, System,
};

use crate::config_patcher::home_dir;
use crate::harnesses::which_exists;

#[derive(Debug, Clone)]
pub struct DiskStatus {
    pub label: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub fs_type: String,
    pub mounted: bool,
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
    pub root_disk: DiskStatus,
    pub hdd_disk: DiskStatus,
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
            low_space_bytes: low_space_gb * 1024 * 1024 * 1024,
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
            root_disk: unmounted_disk("SSD / (60 ГБ)", "/"),
            hdd_disk: unmounted_disk("HDD /mnt/hdd (500 ГБ)", "/mnt/hdd"),
            processes: Vec::new(),
            sort_mode: ProcessSort::Cpu,
        };
        monitor.refresh();
        monitor
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
            cpu_t = read_thermal_zone_fallback();
        }
        if gpu_t.is_none() {
            gpu_t = read_drm_gpu_temp_fallback();
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
            if iface == "lo" {
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
        self.root_disk = self.find_disk("SSD / (60 ГБ)", "/");
        self.hdd_disk = self.find_disk("HDD /mnt/hdd (500 ГБ)", "/mnt/hdd");
    }

    fn find_disk(&self, label: &str, mount: &str) -> DiskStatus {
        for disk in &self.disks {
            if disk.mount_point() == Path::new(mount) {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);
                return DiskStatus {
                    label: label.to_string(),
                    mount_point: mount.to_string(),
                    total_bytes: total,
                    used_bytes: used,
                    available_bytes: available,
                    fs_type: disk.file_system().to_string_lossy().into_owned(),
                    mounted: true,
                    low_space: available < self.low_space_bytes,
                };
            }
        }
        unmounted_disk(label, mount)
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

pub fn trigger_cache_cleanup() -> Result<String, String> {
    if which_exists("kitty") {
        let script = concat!(
            "echo '=== Очистка кэша пакетов Arch Linux (paccache -rk1) ==='; ",
            "if command -v paccache >/dev/null 2>&1; then ",
            "  sudo paccache -rk1 || true; ",
            "else ",
            "  echo 'paccache не найден (установите pacman-contrib)'; ",
            "fi; ",
            "echo ''; ",
            "echo '=== Очистка пользовательского кэша (~/.cache) ==='; ",
            "BEFORE=$(du -sh \"$HOME/.cache\" 2>/dev/null | cut -f1); ",
            "find \"$HOME/.cache\" -mindepth 1 -maxdepth 1 -exec rm -rf {} + 2>/dev/null || true; ",
            "echo \"Очищено ~/.cache (было: ${BEFORE:-0})\"; ",
            "echo ''; ",
            "read -rp 'Готово! Нажмите Enter для закрытия...'"
        );

        Command::new("kitty")
            .args(["--title", "zipctl: Очистка кэша", "sh", "-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Не удалось запустить Kitty: {e}"))?;

        return Ok(
            "Запущена очистка кэша (paccache -rk1 и ~/.cache) в новом окне Kitty.".to_string(),
        );
    }

    // Резервный режим без Kitty: очищаем ~/.cache напрямую и запускаем paccache если доступно
    let cache_dir = home_dir().join(".cache");
    let mut removed = 0usize;
    if let Ok(entries) = fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let res = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            if res.is_ok() {
                removed += 1;
            }
        }
    }

    if which_exists("paccache") {
        let _ = Command::new("paccache")
            .arg("-rk1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    Ok(format!(
        "Очищено объектов в ~/.cache: {removed} (paccache -rk1 выполнен в фоне)."
    ))
}

fn unmounted_disk(label: &str, mount: &str) -> DiskStatus {
    DiskStatus {
        label: label.to_string(),
        mount_point: mount.to_string(),
        total_bytes: 0,
        used_bytes: 0,
        available_bytes: 0,
        fs_type: "—".to_string(),
        mounted: false,
        low_space: false,
    }
}

fn read_thermal_zone_fallback() -> Option<f32> {
    let raw = fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").ok()?;
    let milli: f32 = raw.trim().parse().ok()?;
    if milli > 1000.0 {
        Some(milli / 1000.0)
    } else {
        Some(milli)
    }
}

fn read_drm_gpu_temp_fallback() -> Option<f32> {
    for card in 0..4 {
        let hwmon_dir = format!("/sys/class/drm/card{card}/device/hwmon");
        if let Ok(entries) = fs::read_dir(&hwmon_dir) {
            for entry in entries.flatten() {
                let temp_path = entry.path().join("temp1_input");
                if let Ok(raw) = fs::read_to_string(temp_path) {
                    if let Ok(milli) = raw.trim().parse::<f32>() {
                        if milli > 1000.0 {
                            return Some(milli / 1000.0);
                        }
                    }
                }
            }
        }
    }
    None
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
