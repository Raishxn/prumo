# Prumo — desenho do projeto

Nome de trabalho: **Prumo** (o fio de prumo mede se algo está reto: aqui, frame time
reto e sistema estável). Alternativas: **Compasso**, **Fôlego**, **Quadro Firme**,
**Linha d'Água**. O nome só aparece em `Cargo.toml`, no binário e nos textos; trocar é
uma busca-e-substitui.

Data desta revisão: 2026-11-01.

---

## 1. O que entendi

Um otimizador pessoal para **um** computador (Ryzen 7 5700X, RTX 3060 Ti, 32 GB,
SSD compartilhado em dual boot), com dois focos:

- **CachyOS + Niri/Wayland**: Minecraft GTL e GTNH (com shaders quando compatível) e
  lives no YouTube pelo OBS; impressora Epson L3250.
- **Windows 11**: Valorant (1920×1080 e 1440×1080) e League of Legends, sem quebrar
  Vanguard, Chrome, Discord, NVIDIA Broadcast, Office 2010, Lightshot e a impressora.
  Instalação atual já passou por um playbook VainOS de origem desconhecida.

O objetivo é **consistência de frame time** (menos stuttering), não "mais ajustes".
Toda alteração precisa ser explicada, aplicada só se compatível, registrada com o
estado anterior real e reversível; ganhos só são afirmados quando medidos.

## 2. O que o diagnóstico automático já descobriu neste PC (Linux)

Medido em 2026-11-01 com `prumo diagnostico` (fonte de cada valor no JSON):

| Item | Valor | Observação |
|---|---|---|
| Placa-mãe | MAXSUN **MS-Terminator B550M**, BIOS 5.17 (09/06/2026) | variante descoberta via DMI |
| Rede | **Ethernet** `enp3s0`, RTL8111 (r8169), link 1000 Mb/s | não é Wi-Fi; nenhum ajuste de Wi-Fi se aplica |
| GPU | RTX 3060 Ti, driver 615.78.08 (módulo **open**), VBIOS 94.04.6B.00.C0 | |
| **Link PCIe da GPU** | **x8**, mas GPU e porta da CPU (00:03.1) suportam **x16** | achado de hardware; ver §4 H1 |
| Monitor | EDID "HKM M238W1F2556" em HDMI-A-1, 1920×1080 @ 165 Hz, **VRR não suportado** nesta conexão | ver lacuna L3 |
| Armazenamento | SSD SATA "XrayDisk 1TB", Btrfs com zstd:3, `discard=async` | sem snapper/timeshift |
| Boot | rEFInd, **Secure Boot desativado**, TPM2 presente | ver lacuna L1 |
| Memória | 32 GB; **~9–12 GB em swap zram** durante o uso normal | achado; ver §4 |
| Cache de shaders NVIDIA | **993 MB** em `~/.cache/nvidia/GLCache` | perto do limite; ação recomendada |
| CPU | amd-pstate-epp, governador performance, EPP performance | já agressivo |
| Agendador | kernel BORE; scx_loader instalado e inativo; ananicy-cpp ativo | |
| OBS 32.2 | 1080p60, NVENC H.264 CBR, keyint 2 s, **bitrate não definido no perfil**, captura PipeWire | |
| Minecraft | Prism Launcher, 43 instâncias; GTNH 2.9 RC/Beta em Java 25; GTL em Java 17/21 | todas herdam 512–14096 MB |

Desconhecidos (o app mostra como "desconhecido", com o motivo): módulos de RAM (exige
root), fonte, cooler, upload real, FPS atual, versões de shaders, FPS/quadro do OBS
em live.

## 3. Lacunas que realmente bloqueiam decisões

- ~~**L1 — Secure Boot.**~~ Respondido: o Vanguard exige Secure Boot; com ele ligado o
  rEFInd não inicia, então hoje o Windows é aberto pelo menu do firmware. Plano em
  [SECURE-BOOT.md](SECURE-BOOT.md) (etapa 2, guiado, nada automático).
- **L2 — Quais instâncias são as "reais".** O plano GTNH hoje alcança 5 instâncias
  (Beta1, RC-1, RC-2, [ADDONS], [BACKUP]). O GTNH 2.9.0 final saiu em 31/10. Qual
  instância de GTNH e qual de GTL você joga de fato?
- **L3 — Monitor por HDMI sem VRR.** O Niri informa VRR não suportado na conexão
  atual. O monitor tem entrada DisplayPort? (VRR com G-SYNC Compatible costuma exigir
  DP em monitores desse tipo; precisa confirmar no manual do seu modelo.)
- **L4 — Live.** Resolução/FPS desejados, se grava ao mesmo tempo, e autorização para
  medir o upload contra um servidor público (ex.: speed.cloudflare.com).

O resto (shaders, versões, temperaturas sob carga) é descoberto ou medido pelo app.

## 4. Arquitetura e escolha tecnológica

### Comparação

| Opção | Repouso (estimativa*) | Wayland/Niri | Windows | APIs nativas | Distribuição | Observação |
|---|---|---|---|---|---|---|
| **Rust + egui (eframe)** | dezenas de MB, ~0% CPU (redesenha só com evento) | winit nativo Wayland | sim | FFI direto | binário único | MIT/Apache; visual menos "nativo" |
| Rust + Slint | similar | sim | sim | FFI direto | binário único | licença royalty-free exige atribuição, ou GPLv3 |
| Rust + Tauri | WebView: ~100 MB+ | WebKitGTK no NVIDIA/Wayland tem histórico de falhas de renderização | WebView2 | sim | instalador + WebView | risco justo nesta máquina |
| C# + Avalonia | runtime .NET | X11/XWayland na prática | ótimo | P/Invoke | 60–100 MB self-contained | dois idiomas se o núcleo for Rust |

\* Estimativas de literatura/experiência, **não medidas**; viram metas abaixo.

**Escolha: Rust** para núcleo, CLI e executor privilegiado; **egui/eframe** para a
interface (etapa 6). Motivos: um único código para Linux e Windows, binário pequeno
sem runtime, nenhum processo residente, FFI direta (NVML, WMI, registro) e
interface que não consome CPU parada. Se o visual do egui não agradar, a interface é
uma casca fina sobre `prumo-core` e pode ser trocada por Slint sem tocar no motor.

### Componentes

```
prumo-core   catálogo (TOML embutido) · diagnóstico · motor (planejar/aplicar/desfazer)
             · diário write-ahead · proteção (ponto de restauração / backups)
prumo (CLI)  sem interface gráfica; também é o caminho de recuperação
prumo-gui    (etapa 6) interface sem privilégio; chama o executor elevado só para
             ações Admin
playbook/    (etapa 4) AME Wizard: empacota prumo.exe e chama as MESMAS ações
```

- **Separação de privilégio**: a interface roda como usuário. Ações `admin` são
  executadas por `prumo` elevado (pkexec no Linux, UAC no Windows) recebendo **só ids
  do catálogo e parâmetros validados** (`int:MIN:MAX` / `enum:`): o conteúdo escrito
  vem do binário, nunca de arquivo externo.
- **Sem serviço permanente**: nada fica rodando depois de aplicar.
- **Uma só fonte de verdade**: app e playbook usam o mesmo catálogo e o mesmo
  executável, então não podem aplicar versões divergentes de uma ação; o diário
  registra `origin = app | playbook` e a versão de cada ação.

### Metas (a medir; até lá são só metas)

| Métrica | Meta | Medido |
|---|---|---|
| CLI: diagnóstico completo | < 2 s | **1,5 s** (release, 43 instâncias) |
| CLI: binário | < 5 MB | **1,3 MB** (release, Linux) |
| GUI: abertura até a janela | < 1 s | não medido (etapa 6) |
| GUI: RAM em repouso | < 60 MB | não medido |
| GUI: CPU em repouso | ~0% (sem polling) | não medido |
| Durante o jogo | 0 processos do app | por projeto |

## 5. Motor de otimizações

Cada ação (`catalog/*.toml`) tem id estável, versão, nível (`recommended`, `advanced`,
`manual`, `rejected`), evidência, privilégio, reinício, os 10 itens de documentação,
condições verificáveis contra o diagnóstico, dependências, conflitos, parâmetros
validados e passos declarativos (`write_file`, `ini_set`, `prism_instance`, `reg_set`).

Fluxo: **diagnóstico → plano (prévia com valor atual → proposto) → proteção
verificada → aplicação → validação por operação → diário → desfazer**.

Garantias implementadas e testadas (`crates/prumo-core/tests/engine.rs`):

- idempotente (segunda execução = "já configurado", nenhuma entrada nova);
- estados distintos: aplicado, já configurado, incompatível, bloqueado (ex.: jogo
  aberto), ignorado, falhou;
- **write-ahead**: o estado anterior vai para o disco antes da alteração; queda no meio
  deixa a entrada como `intent`, e `prumo recuperar`/`desfazer` restauram;
- ação atômica: falha numa operação reverte as anteriores da mesma ação; ações
  independentes continuam; dependentes da que falhou são ignoradas;
- desfazer por sessão ou por ação, em ordem inversa, restaurando o **estado anterior
  real** (inclusive "não existia");
- se o valor foi mudado depois por você/outro programa, o desfazer **não sobrescreve**
  sem `--forcar`;
- backup com SHA-256 conferido antes de restaurar;
- conflitos entre ações e ações rejeitadas nunca são aplicados.

## 6. Matriz inicial de otimizações

Legenda de evidência: **DOC** documentação oficial do funcionamento (não prova ganho) ·
**EXT** medição externa relevante · **AQUI** medido neste PC · **HIP** hipótese ·
**REJ** rejeitado.

### CachyOS

| Ação | Evidência | Decisão |
|---|---|---|
| Cache de shaders NVIDIA maior (`__GL_SHADER_DISK_CACHE_SIZE` via environment.d) | DOC (README NVIDIA) + estado medido aqui (993 MB) | **Recomendado** (implementado) |
| GTNH: mín = máx 6 GB e `-XX:+UseCompactObjectHeaders` (Java 25+) | DOC (wiki GTNH) | **Recomendado** (implementado; alvos a confirmar, L2) |
| GTL: heap fixo | HIP (página do GTL não orienta) | Avançado, só com A/B (implementado) |
| Reduzir swap durante o jogo (fechar processos pesados) | HIP | Orientação; o app nunca encerra processos |
| GPU em PCIe x8 em vez de x16 | AQUI (estado), impacto não medido | **Orientação de hardware** (H1) |
| VRR (DisplayPort/G-SYNC Compatible) | depende de L3 | A avaliar |
| Bitrate da live a partir do upload medido | DOC (YouTube, matriz NVENC) | Etapa 2, após medir upload |
| sched_ext (scx_lavd/bpfland) | HIP | Manual, A/B |
| Snapshot Btrfs antes de ações admin | DOC (btrfs) | Etapa 2 |
| `mitigations=off`, "limpar RAM", pacotes TCP/DNS | REJ | Rejeitados, com motivo no catálogo |
| `__GL_THREADED_OPTIMIZATIONS` | DOC diz que pode piorar apps com glGet* e exige Xlib thread-safe | Rejeitado até medição |

### Windows 11 (backend na etapa 3; nada foi executado em Windows ainda)

| Ação | Evidência | Decisão |
|---|---|---|
| Manter TPM, Secure Boot, VBS/HVCI, IOMMU | DOC (Riot Vanguard) | **Obrigatório**; desligar = rejeitado |
| Ponto de restauração verificado antes de alterar | DOC (Checkpoint-Computer: 1 por 24 h) | Implementado (não testado em Windows) |
| Gravação em segundo plano do Game Bar | HIP | Avançado |
| HAGS | DOC diz "transparente", sem promessa de ganho | Manual, A/B |
| NVIDIA Reflex no Valorant/LoL (opção do jogo) | DOC NVIDIA (a consultar na etapa 3) | Orientação |
| Inicialização (Lightshot, Discord, Broadcast…) | — | Gerenciar com descrição e reversão; nada removido por padrão |
| Inventário VainOS (serviços/tarefas/appx removidos) | — | Diagnóstico; origem marcada como incerta |
| Limpar standby/working set, "timer resolution", desativar Defender | REJ | Rejeitados |
| Ajustes de rede genéricos | REJ (Ethernet 1 Gb/s confirmado) | Rejeitados sem diagnóstico |

### Hardware e periféricos (sempre manual)

| Tema | Orientação |
|---|---|
| H1 PCIe x8 | Confirmar com carga (`nvidia-smi -q -d PCIE` com jogo aberto). Se seguir x8: desligar, reencaixar a placa no slot superior, limpar contatos, conferir opção de bifurcação/PCIe na BIOS. Impacto de x8 PCIe 4.0 em jogos costuma ser pequeno em medições externas; o motivo de investigar é também a saúde do link. |
| RAM 3600 CL18 | Confirmar XMP ativo e FCLK 1800 (1:1) na BIOS; validar estabilidade (TestMem5/y-cruncher no Windows, `stress-ng --vm` no Linux) antes de PBO/CO. |
| PBO / Curve Optimizer | Só depois de estabilidade de RAM; valores por núcleo, negativos pequenos, testados um a um; critérios: 0 erros WHEA/MCE, Tctl < 90 °C sob carga, reprovação = voltar ao anterior. Nenhum valor universal. |
| OP1 8K | Testar 1000/2000/4000/8000 Hz com o jogo e o OBS abertos e comparar frame time e uso de CPU; 8000 Hz não é presumido melhor. |
| ATK68 | Nenhuma alteração de firmware. |

## 7. Etapas e critérios de aceitação

| Etapa | Entrega | Aceite |
|---|---|---|
| **1** ✅ | Núcleo Rust, catálogo v1, diagnóstico Linux, CLI, diário, desfazer, testes | `cargo test` verde; diagnóstico real sem inventar valores; prévia real correta |
| 2 | Linux completo: executor elevado (pkexec), snapshot Btrfs verificado, teste de rede/upload, escolha de alvos (instâncias), perfil novo do OBS (cópia, sem chave) | aplicar → reiniciar → validar → desfazer no seu CachyOS |
| 3 | Backend Windows: diagnóstico (WMI, Vanguard/segurança, inventário VainOS), ponto de restauração verificado, registro, inicialização | rodado por você no Windows com o roteiro de validação |
| 4 | Playbook AME (dois cenários: limpo / existente com VainOS) chamando o mesmo `prumo.exe` | validação contra a doc do AME + teste em VM Windows 11 |
| 5 | Medição: MangoHud (Linux) e PresentMon (Windows) → estatísticas, definição de stutter, relatório HTML offline, export JSON/CSV | relatório com dados reais ou "não medido" |
| 6 | Interface egui em português com todas as páginas | metas de RAM/CPU medidas |
| 7 | Drivers (fluxo guiado), periféricos, empacotamento (PKGBUILD, zip/instalador), desinstalação | instalar/desinstalar limpo nos dois sistemas |
