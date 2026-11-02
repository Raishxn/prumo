# Roadmap

Critérios de aceite detalhados em [docs/PROJETO.md §7](docs/PROJETO.md#7-etapas-e-critérios-de-aceitação).

| # | Etapa | Status |
|---|---|---|
| 1 | Núcleo Rust: catálogo, diagnóstico Linux, motor reversível, diário write-ahead, CLI, testes | ✅ concluída (2026-11-01) |
| 2 | Linux completo: executor elevado (pkexec), snapshot Btrfs verificado, escolha de alvos, teste de upload, perfil novo do OBS, **Secure Boot com rEFInd (shim/MOK)** guiado | ⏳ próxima |
| 3 | Backend Windows: diagnóstico WMI/segurança/Vanguard, inventário VainOS, ponto de restauração verificado, registro, inicialização | 🔜 prioridade (troca para o Windows) |
| 4 | Playbook AME Wizard: cenário limpo e cenário existente (VainOS), chamando o mesmo `prumo.exe` | planejada |
| 5 | Medição: MangoHud/PresentMon → estatísticas, stutter definido, relatório HTML offline, JSON (`prumo medir`) | 🚧 cálculo pronto; falta CSV real e saída CSV |
| 6 | Interface egui em português (todas as páginas) | planejada |
| 7 | Drivers guiados, periféricos (OP1 8K), BIOS/PBO manual, empacotamento e desinstalação | planejada |
| 8 | **ISO Windows 11 customizada leve** (estilo tiny11), compatível com Vanguard | planejada |

## Etapa 8 — ISO customizada (anotado a pedido, 2026-11-01)

Objetivo: instalação limpa, leve e reproduzível, **sem** quebrar o que você usa.

- Base: ISO oficial da Microsoft; modificação com DISM (Windows) ou wimlib (Linux),
  script versionado no repositório, hash da ISO de origem registrado.
- `autounattend.xml`: conta local, OOBE sem telas de publicidade, idioma pt-BR,
  decisão explícita sobre criptografia de dispositivo (BitLocker).
- Remover só appx pré-instalados sem dependência conhecida (lista revisável, cada item
  com motivo). Referência de funcionalidades: tiny11builder (licença a verificar antes
  de qualquer reaproveitamento).
- **Não remover / não contornar**: Windows Update, Defender, requisitos TPM 2.0 e
  Secure Boot (Vanguard), WebView2 (NVIDIA app/Broadcast), spooler de impressão
  (Epson L3250), Visual C++ runtimes, Microsoft Store/App Installer (winget).
- Validação em VM (Hyper-V/QEMU com TPM virtual) e depois no PC: Valorant, LoL,
  Broadcast, Office 2010, Lightshot, impressão.
- O playbook AME (etapa 4) continua sendo o caminho para a instalação existente; a ISO
  aplica o mesmo catálogo no primeiro boot (`prumo.exe aplicar --origem iso`).
