//! Coleta no Linux: /proc, /sys e ferramentas do sistema, sem root.

use super::{run, Diagnosis};
use std::fs;
use std::path::Path;

fn read(p: &str) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

fn meminfo_kb(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find(|l| l.starts_with(&format!("{key}:")))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

fn dir_size_mb(p: &Path) -> Option<u64> {
    fn walk(p: &Path) -> u64 {
        let Ok(rd) = fs::read_dir(p) else { return 0 };
        rd.flatten()
            .map(|e| match e.metadata() {
                Ok(m) if m.is_dir() => walk(&e.path()),
                Ok(m) => m.len(),
                Err(_) => 0,
            })
            .sum()
    }
    p.exists().then(|| walk(p) / (1024 * 1024))
}

pub fn collect(d: &mut Diagnosis) {
    d.known("os", "linux", "std::env::consts::OS");
    let osr = read("/etc/os-release").unwrap_or_default();
    let field = |k: &str| {
        osr.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .map(|v| v.trim_matches('"').to_string())
    };
    d.opt("os.id", field("ID"), "/etc/os-release", "sem /etc/os-release");
    d.opt("os.pretty", field("PRETTY_NAME"), "/etc/os-release", "sem /etc/os-release");
    d.opt("kernel", read("/proc/sys/kernel/osrelease"), "/proc/sys/kernel/osrelease", "ilegível");

    // CPU
    let cpuinfo = read("/proc/cpuinfo").unwrap_or_default();
    let model = cpuinfo.lines().find_map(|l| l.strip_prefix("model name")).map(|v| v.trim_start_matches([' ', '\t', ':']).to_string());
    d.opt("cpu.model", model, "/proc/cpuinfo", "ilegível");
    let threads = cpuinfo.lines().filter(|l| l.starts_with("processor")).count();
    d.known("cpu.threads", threads.to_string(), "/proc/cpuinfo");
    let cf = "/sys/devices/system/cpu/cpu0/cpufreq";
    d.opt("cpu.driver", read(&format!("{cf}/scaling_driver")), "cpufreq", "sem cpufreq");
    d.opt("cpu.governor", read(&format!("{cf}/scaling_governor")), "cpufreq", "sem cpufreq");
    d.opt("cpu.epp", read(&format!("{cf}/energy_performance_preference")), "cpufreq", "EPP indisponível");
    d.opt("cpu.boost", read("/sys/devices/system/cpu/cpufreq/boost"), "cpufreq/boost", "indisponível");

    // Memória
    let mi = read("/proc/meminfo").unwrap_or_default();
    let kb = |k| meminfo_kb(&mi, k).map(|v| (v / 1024).to_string());
    d.opt("mem.total_mb", kb("MemTotal"), "/proc/meminfo", "ilegível");
    d.opt("mem.available_mb", kb("MemAvailable"), "/proc/meminfo", "ilegível");
    if let (Some(t), Some(f)) = (meminfo_kb(&mi, "SwapTotal"), meminfo_kb(&mi, "SwapFree")) {
        d.known("swap.total_mb", (t / 1024).to_string(), "/proc/meminfo");
        d.known("swap.used_mb", ((t - f) / 1024).to_string(), "/proc/meminfo");
    }
    let swaps = read("/proc/swaps").unwrap_or_default();
    let kinds: Vec<&str> = swaps.lines().skip(1).filter_map(|l| l.split_whitespace().next()).collect();
    d.known("swap.devices", if kinds.is_empty() { "nenhum".into() } else { kinds.join(",") }, "/proc/swaps");
    for (k, p) in [("psi.memory", "/proc/pressure/memory"), ("psi.io", "/proc/pressure/io"), ("psi.cpu", "/proc/pressure/cpu")] {
        if let Some(t) = read(p) {
            let some300 = t.lines().find(|l| l.starts_with("some")).and_then(|l| l.split_whitespace().find_map(|w| w.strip_prefix("avg300=")));
            d.opt(&format!("{k}.some_avg300"), some300.map(String::from), p, "sem PSI");
        }
    }
    d.unknown("mem.modules", "requer root (dmidecode -t memory); informe ou rode o diagnóstico elevado");
    for k in ["vm.swappiness", "vm.max_map_count"] {
        let p = format!("/proc/sys/{}", k.replace('.', "/"));
        d.opt(&format!("sysctl.{k}"), read(&p), &p, "ilegível");
    }
    d.opt("thp.enabled", read("/sys/kernel/mm/transparent_hugepage/enabled"), "sysfs THP", "indisponível");

    // Placa-mãe / BIOS (DMI é legível sem root para estes campos)
    for (k, f) in [("board.vendor", "board_vendor"), ("board.name", "board_name"), ("bios.version", "bios_version"), ("bios.date", "bios_date")] {
        d.opt(k, read(&format!("/sys/class/dmi/id/{f}")), "/sys/class/dmi/id", "DMI indisponível");
    }

    // GPU NVIDIA
    let q = "name,driver_version,vbios_version,memory.total,pcie.link.gen.current,pcie.link.gen.max,pcie.link.width.current,pcie.link.width.max,temperature.gpu,power.limit";
    match run("nvidia-smi", &[&format!("--query-gpu={q}"), "--format=csv,noheader,nounits"]) {
        Some(out) => {
            let cols: Vec<&str> = out.lines().next().unwrap_or("").split(", ").collect();
            let keys = ["gpu.name", "gpu.driver", "gpu.vbios", "gpu.vram_mb", "gpu.pcie_gen_current", "gpu.pcie_gen_max", "gpu.pcie_width_current", "gpu.pcie_width_max", "gpu.temp_c", "gpu.power_limit_w"];
            d.known("gpu.vendor", "nvidia", "nvidia-smi");
            for (k, v) in keys.iter().zip(cols.iter()) {
                d.known(k, *v, "nvidia-smi");
            }
        }
        None => d.unknown("gpu.vendor", "nvidia-smi indisponível; outra GPU não é suportada nesta etapa"),
    }
    pcie_link(d);
    d.opt("gpu.kernel_module", read("/sys/module/nvidia/version"), "/sys/module/nvidia/version", "módulo nvidia não carregado");
    let open = read("/proc/driver/nvidia/version").map(|v| if v.contains("Open Kernel") { "open" } else { "proprietário" }.to_string());
    d.opt("gpu.kernel_flavor", open, "/proc/driver/nvidia/version", "indisponível");
    let home = std::env::var("HOME").unwrap_or_default();
    let cache = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| format!("{home}/.cache"));
    d.opt("nvidia.glcache_mb", dir_size_mb(&Path::new(&cache).join("nvidia/GLCache")).map(|v| v.to_string()), "tamanho de ~/.cache/nvidia/GLCache", "cache inexistente");
    let env_size = std::env::var("__GL_SHADER_DISK_CACHE_SIZE").ok();
    d.opt("nvidia.glcache_size_env", env_size, "variável de ambiente", "não definida (tamanho padrão do driver)");

    // Armazenamento e boot
    if let Some(out) = run("findmnt", &["-no", "SOURCE,FSTYPE,OPTIONS", "/"]) {
        let mut it = out.split_whitespace();
        d.opt("root.source", it.next().map(String::from), "findmnt /", "?");
        d.opt("root.fs", it.next().map(String::from), "findmnt /", "?");
        d.opt("root.options", it.next().map(String::from), "findmnt /", "?");
    }
    let disks = run("lsblk", &["-dno", "NAME,MODEL,SIZE,ROTA,TYPE"]).unwrap_or_default();
    let list: Vec<String> = disks.lines().filter(|l| l.trim_end().ends_with("disk")).map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    d.opt("disks", (!list.is_empty()).then(|| list.join("; ")), "lsblk", "lsblk indisponível");
    let parts = run("lsblk", &["-no", "NAME,FSTYPE,SIZE,PARTLABEL"]).unwrap_or_default();
    let ntfs = parts.lines().filter(|l| l.contains("ntfs")).count();
    d.known("disk.ntfs_partitions", ntfs.to_string(), "lsblk (indica partições do Windows)");
    d.opt("snapper", which("snapper"), "PATH", "snapper não instalado");
    d.opt("timeshift", which("timeshift"), "PATH", "timeshift não instalado");
    let sb = fs::read("/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c").ok().and_then(|b| b.last().copied());
    d.opt("secureboot", sb.map(|b| if b == 1 { "ativado" } else { "desativado" }.into()), "efivars SecureBoot", "efivars inacessíveis");
    let boot = run("efibootmgr", &[]).and_then(|o| o.lines().find(|l| l.starts_with("BootCurrent")).map(|bc| {
        let n = bc.split(':').nth(1).unwrap_or("").trim().to_string();
        o.lines().find(|l| l.starts_with(&format!("Boot{n}"))).unwrap_or(bc).to_string()
    }));
    d.opt("boot.current", boot, "efibootmgr", "efibootmgr indisponível");

    // Rede
    let mut nets = vec![];
    if let Ok(rd) = fs::read_dir("/sys/class/net") {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n == "lo" {
                continue;
            }
            let base = format!("/sys/class/net/{n}");
            if read(&format!("{base}/operstate")).as_deref() != Some("up") {
                continue;
            }
            let kind = if Path::new(&format!("{base}/wireless")).exists() { "wifi" } else if Path::new(&format!("{base}/device")).exists() { "ethernet" } else { "virtual" };
            if kind == "virtual" {
                continue;
            }
            let speed = read(&format!("{base}/speed")).unwrap_or_else(|| "?".into());
            let drv = fs::read_link(format!("{base}/device/driver")).ok().and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string())).unwrap_or_else(|| "?".into());
            nets.push(format!("{n} {kind} {speed}Mb/s driver={drv}"));
            d.known("net.kind", kind, "/sys/class/net");
        }
    }
    d.opt("net.links", (!nets.is_empty()).then(|| nets.join("; ")), "/sys/class/net", "nenhum link físico ativo");
    d.unknown("net.upload_mbps", "não medido; use o teste de rede guiado (etapa de rede) antes de definir bitrate");

    // Sessão gráfica
    d.opt("session.type", std::env::var("XDG_SESSION_TYPE").ok(), "XDG_SESSION_TYPE", "não definido");
    d.opt("session.desktop", std::env::var("XDG_CURRENT_DESKTOP").ok(), "XDG_CURRENT_DESKTOP", "não definido");
    if let Some(o) = run("niri", &["msg", "outputs"]) {
        let mode = o.lines().find_map(|l| l.trim().strip_prefix("Current mode:")).map(|s| s.trim().to_string());
        let vrr = o.lines().find_map(|l| l.trim().strip_prefix("Variable refresh rate:")).map(|s| s.trim().to_string());
        let name = o.lines().next().map(|l| l.trim_start_matches("Output ").to_string());
        d.opt("display.output", name, "niri msg outputs (EDID)", "?");
        d.opt("display.mode", mode, "niri msg outputs", "?");
        d.opt("display.vrr", vrr, "niri msg outputs", "?");
    }
    d.opt("niri.version", run("niri", &["--version"]), "niri --version", "niri ausente");

    // Agendamento e energia
    d.opt("scx.loader", run("systemctl", &["is-active", "scx_loader"]).or(Some("inactive".into())), "systemctl is-active scx_loader", "?");
    d.opt("ananicy", run("systemctl", &["is-active", "ananicy-cpp"]).or(Some("inactive".into())), "systemctl is-active ananicy-cpp", "?");
    d.opt("power.profile", run("powerprofilesctl", &["get"]), "powerprofilesctl", "power-profiles-daemon ausente");

    // Sensores
    match run("sensors", &["-j"]) {
        Some(j) => {
            let v: serde_json::Value = serde_json::from_str(&j).unwrap_or_default();
            let tctl = v.as_object().and_then(|o| o.iter().find(|(k, _)| k.starts_with("k10temp"))).and_then(|(_, c)| c["Tctl"]["temp1_input"].as_f64());
            d.opt("cpu.tctl_c", tctl.map(|t| format!("{t:.1}")), "lm_sensors k10temp", "k10temp indisponível");
        }
        None => d.unknown("cpu.tctl_c", "lm_sensors indisponível"),
    }
    d.unknown("hw.psu", "não detectável por software; informe o modelo da fonte");
    d.unknown("hw.cooler", "não detectável por software; informe o modelo do cooler");

    // Programas
    d.opt("java.system", java_version_stderr(), "java -version (stderr)", "java ausente no PATH");
    obs(d, &home);
    d.opt("cups", run("systemctl", &["is-active", "cups"]).or(Some("inactive".into())), "systemctl is-active cups", "?");
    let printers = run("lpstat", &["-e"]).map(|o| o.lines().collect::<Vec<_>>().join(","));
    d.opt("printers", printers, "lpstat -e", "nenhuma fila de impressão");

    // Minecraft
    let cmdlines = process_cmdlines();
    if let Some(root) = crate::prism::default_root() {
        d.known("prism.root", root.to_string_lossy(), "PrismLauncher");
        d.minecraft = crate::prism::scan(&root, &cmdlines);
        d.known("prism.instances", d.minecraft.len().to_string(), "instances/");
    } else {
        d.unknown("prism.root", "Prism Launcher não encontrado");
    }
}

/// Largura/velocidade do link PCIe da GPU NVIDIA e da porta acima dela, via
/// sysfs (legível sem root). Velocidade baixa em repouso é economia de
/// energia normal; largura menor que a máxima de ambos os lados não é.
fn pcie_link(d: &mut Diagnosis) {
    let Ok(rd) = fs::read_dir("/sys/bus/pci/devices") else { return };
    for e in rd.flatten() {
        let p = e.path();
        let vendor = read(&p.join("vendor").to_string_lossy());
        let class = read(&p.join("class").to_string_lossy()).unwrap_or_default();
        if vendor.as_deref() != Some("0x10de") || !class.starts_with("0x03") {
            continue;
        }
        let r = |f: &str| read(&p.join(f).to_string_lossy());
        d.opt("gpu.pcie.address", Some(e.file_name().to_string_lossy().to_string()), "sysfs", "?");
        d.opt("gpu.pcie.width", r("current_link_width"), "sysfs current_link_width", "?");
        d.opt("gpu.pcie.width_max", r("max_link_width"), "sysfs max_link_width", "?");
        d.opt("gpu.pcie.speed", r("current_link_speed"), "sysfs current_link_speed (cai em repouso)", "?");
        if let Ok(up) = fs::canonicalize(p.join("..")) {
            let ur = |f: &str| read(&up.join(f).to_string_lossy());
            d.opt("gpu.pcie.port", up.file_name().map(|s| s.to_string_lossy().to_string()), "sysfs", "?");
            d.opt("gpu.pcie.port_width_max", ur("max_link_width"), "sysfs da porta acima da GPU", "?");
        }
        break;
    }
}

fn java_version_stderr() -> Option<String> {
    let o = std::process::Command::new("java").arg("-version").output().ok()?;
    String::from_utf8_lossy(&o.stderr).lines().next().map(String::from)
}

fn which(bin: &str) -> Option<String> {
    std::env::var("PATH").ok()?.split(':').map(|d| format!("{d}/{bin}")).find(|p| Path::new(p).exists())
}

fn obs(d: &mut Diagnosis, home: &str) {
    let base = format!("{home}/.config/obs-studio/basic/profiles");
    let Ok(rd) = fs::read_dir(&base) else {
        d.unknown("obs.profile", "perfil do OBS não encontrado");
        return;
    };
    // Lê só configurações de vídeo/codificação. service.json (chave de
    // transmissão) NUNCA é lido nem copiado pelo diagnóstico.
    let names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    d.known("obs.profiles", names.join(","), "~/.config/obs-studio/basic/profiles");
    let Some(p) = names.first() else { return };
    let ini = fs::read_to_string(format!("{base}/{p}/basic.ini")).unwrap_or_default();
    let g = |s, k| crate::ini::get(&ini, Some(s), k);
    let mode = g("Output", "Mode");
    d.opt("obs.output_mode", mode.clone(), "basic.ini", "?");
    let adv = mode.as_deref() == Some("Advanced");
    let enc = if adv { g("AdvOut", "Encoder") } else { g("SimpleOutput", "StreamEncoder") };
    d.opt("obs.stream_encoder", enc, "basic.ini", "?");
    if let (Some(w), Some(h)) = (g("Video", "OutputCX"), g("Video", "OutputCY")) {
        d.known("obs.output_res", format!("{w}x{h}"), "basic.ini [Video]");
    }
    let fps = match g("Video", "FPSType").as_deref() {
        Some("0") | None => g("Video", "FPSCommon"),
        Some("1") => g("Video", "FPSInt"),
        _ => g("Video", "FPSNum").zip(g("Video", "FPSDen")).map(|(n, d)| format!("{n}/{d}")),
    };
    d.opt("obs.fps", fps, "basic.ini [Video]", "?");
    let se: serde_json::Value = fs::read_to_string(format!("{base}/{p}/streamEncoder.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let br = if adv { se["bitrate"].as_i64().map(|b| b.to_string()) } else { g("SimpleOutput", "VBitrate") };
    d.opt("obs.stream_bitrate_kbps", br, "streamEncoder.json", "não definido no perfil (o OBS usa o padrão do encoder; confira na interface)");
    d.opt("obs.rate_control", se["rate_control"].as_str().map(String::from), "streamEncoder.json", "?");
    d.opt("obs.keyint_sec", se["keyint_sec"].as_i64().map(|v| v.to_string()), "streamEncoder.json", "?");
}

fn process_cmdlines() -> Vec<String> {
    let Ok(rd) = fs::read_dir("/proc") else { return vec![] };
    rd.flatten()
        .filter(|e| e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()))
        .filter_map(|e| fs::read(e.path().join("cmdline")).ok())
        .map(|b| String::from_utf8_lossy(&b).replace('\0', " "))
        .filter(|c| c.contains("java"))
        .collect()
}
