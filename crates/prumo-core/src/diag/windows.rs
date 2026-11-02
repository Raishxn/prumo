//! Coleta no Windows. ETAPA 3: por enquanto só o essencial, via PowerShell e
//! ferramentas do sistema. NÃO TESTADO em Windows real (ver docs/ETAPAS.md).

use super::{run, Diagnosis};

fn ps(cmd: &str) -> Option<String> {
    run("powershell", &["-NoProfile", "-NonInteractive", "-Command", cmd]).map(|s| s.trim().to_string())
}

/// Linhas de comando via WMI. UTF-8 forçado: caminho com acento viria
/// corrompido e a instância aberta não seria reconhecida.
fn process_cmdlines() -> Option<Vec<String>> {
    let out = ps("[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-CimInstance Win32_Process | ForEach-Object { $_.CommandLine }")?;
    crate::prism::parse_process_list(&out)
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
    d.opt(
        "hvci.running",
        ps("(Get-CimInstance -Namespace root\\Microsoft\\Windows\\DeviceGuard -ClassName Win32_DeviceGuard).SecurityServicesRunning -contains 2"),
        "Win32_DeviceGuard (Integridade de Memória, exigida conforme o caso pelo Vanguard)",
        "indisponível",
    );
    d.opt("vanguard.vgc", ps("(Get-Service vgc -ErrorAction SilentlyContinue).Status"), "Get-Service vgc", "Vanguard não instalado");
    d.opt("vanguard.vgk", ps("(Get-Service vgk -ErrorAction SilentlyContinue).Status"), "Get-Service vgk", "Vanguard não instalado");
    d.opt("printer.spooler", ps("(Get-Service Spooler -ErrorAction SilentlyContinue).Status"), "Get-Service Spooler", "serviço ausente (possível remoção pelo playbook)");
    d.opt("store.appinstaller", ps("(Get-AppxPackage Microsoft.DesktopAppInstaller).Version"), "Get-AppxPackage", "App Installer ausente");
    d.opt("webview2", ps("(Get-ItemProperty 'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue).pv"), "registro EdgeUpdate", "WebView2 não encontrado");
    d.opt("startup.items", ps("(Get-CimInstance Win32_StartupCommand | ForEach-Object { $_.Name }) -join ', '"), "Win32_StartupCommand", "lista indisponível");
    d.opt("services.disabled", ps("(Get-CimInstance Win32_Service -Filter \"StartMode='Disabled'\" | ForEach-Object { $_.Name }) -join ', '"), "Win32_Service (origem incerta: pode ser padrão ou VainOS)", "lista indisponível");
    if let Some(root) = crate::prism::default_root() {
        // Sem a lista de processos não dá para saber se há instância aberta:
        // nenhuma instância entra no plano, em vez de todas parecerem fechadas.
        match process_cmdlines() {
            Some(cmdlines) => {
                d.minecraft = crate::prism::scan(&root, &cmdlines);
                d.known("prism.instances", d.minecraft.len().to_string(), "instances/");
            }
            None => d.unknown("prism.instances", "lista de processos indisponível; não é seguro presumir instância fechada"),
        }
    }
    d.unknown("vainos.origin", "não é possível saber quais ajustes vieram do VainOS sem o arquivo do playbook; o estado atual é inventariado como está");
}
