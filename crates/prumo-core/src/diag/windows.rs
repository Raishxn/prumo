//! Coleta no Windows. ETAPA 3: por enquanto só o essencial, via PowerShell e
//! ferramentas do sistema. NÃO TESTADO em Windows real (ver docs/ETAPAS.md).

use super::{run, Diagnosis};

fn ps(cmd: &str) -> Option<String> {
    run("powershell", &["-NoProfile", "-NonInteractive", "-Command", cmd]).map(|s| s.trim().to_string())
}

pub fn collect(d: &mut Diagnosis) {
    d.known("os", "windows", "std::env::consts::OS");
    d.opt("os.build", ps("(Get-CimInstance Win32_OperatingSystem).BuildNumber"), "Win32_OperatingSystem", "WMI indisponível");
    d.opt("os.pretty", ps("(Get-CimInstance Win32_OperatingSystem).Caption"), "Win32_OperatingSystem", "WMI indisponível");
    d.opt("cpu.model", ps("(Get-CimInstance Win32_Processor).Name"), "Win32_Processor", "WMI indisponível");
    d.opt("board.name", ps("(Get-CimInstance Win32_BaseBoard).Product"), "Win32_BaseBoard", "WMI indisponível");
    d.opt("bios.version", ps("(Get-CimInstance Win32_BIOS).SMBIOSBIOSVersion"), "Win32_BIOS", "WMI indisponível");
    d.opt(
        "secureboot",
        ps("try { if (Confirm-SecureBootUEFI) {'ativado'} else {'desativado'} } catch { $null }"),
        "Confirm-SecureBootUEFI",
        "requer administrador ou firmware legado",
    );
    d.opt("tpm", ps("try { (Get-Tpm).TpmReady } catch { $null }"), "Get-Tpm", "requer administrador");
    d.opt(
        "vbs.running",
        ps("(Get-CimInstance -Namespace root\\Microsoft\\Windows\\DeviceGuard -ClassName Win32_DeviceGuard).VirtualizationBasedSecurityStatus"),
        "Win32_DeviceGuard (2 = em execução)",
        "indisponível",
    );
    d.opt(
        "restore.enabled",
        ps("try { (Get-ComputerRestorePoint -ErrorAction Stop | Measure-Object).Count } catch { 'erro: ' + $_.Exception.Message }"),
        "Get-ComputerRestorePoint (quantidade)",
        "requer administrador",
    );
    match run("nvidia-smi", &["--query-gpu=name,driver_version", "--format=csv,noheader"]) {
        Some(o) => {
            let mut c = o.trim().split(", ");
            d.known("gpu.vendor", "nvidia", "nvidia-smi");
            d.opt("gpu.name", c.next().map(String::from), "nvidia-smi", "?");
            d.opt("gpu.driver", c.next().map(String::from), "nvidia-smi", "?");
        }
        None => d.unknown("gpu.vendor", "nvidia-smi indisponível"),
    }
    d.unknown("vainos.origin", "não é possível saber quais ajustes vieram do VainOS sem o arquivo do playbook; o estado atual é inventariado como está");
}
