# TODO

## Respostas que faltam (bloqueiam decisões)

- [x] ~~L1 Secure Boot~~ — respondido: com Secure Boot ligado o rEFInd não inicia;
      o Windows é aberto pelo menu do firmware. → plano em `docs/SECURE-BOOT.md`.
- [x] ~~L2~~ — respondido: joga GTNH nas instâncias 2.9 RC e posteriores; GTL é o
      GregTech Leisure Enhanced 2.9.2.
- [x] ~~L3~~ — respondido: o monitor tem DisplayPort, mas está em HDMI. VRR depende de
      trocar o cabo (ação `linux.monitor.vrr-displayport`).
- [ ] L4 Live: resolução/FPS alvo, gravação simultânea, e autorização para medir upload
      em servidor público.
- [ ] Modelos da fonte e do cooler.

## Verificações no PC (feitas por você)

- [ ] **PCIe x8**: com um jogo aberto no Linux, rodar
      `cat /sys/bus/pci/devices/0000:04:00.0/current_link_width` (esperado 16). Se der 8,
      ver orientação H1 em `docs/PROJETO.md`.
- [ ] Antes da instalação limpa do Windows: seguir `docs/WINDOWS-INSTALACAO-LIMPA.md`.
- [ ] Depois da instalação: anotar a chave de recuperação do BitLocker, se ativado.

## Etapa 2 (Linux)

- [ ] Executor elevado via pkexec (só ids do catálogo + parâmetros validados). Aguarda a primeira ação admin aplicável; hoje nenhuma existe.
- [ ] Snapshot Btrfs verificado antes de ações admin (layout: `@` em `/dev/sda6`).
- [x] Excluir alvos: `--excluir "[BACKUP]"` (vale para todas as ações do plano).
- [ ] Executar `prumo rede` de verdade (código pronto; falta autorização) e conferir o bitrate sugerido.
- [ ] Abrir o perfil `Prumo Live 1080p60` no OBS e validar numa live não listada.
- [ ] Secure Boot: ler as fontes listadas e validar o caminho shim + MOK.

## Etapa 3 (Windows) — prioridade

- [x] Compilar e rodar os testes do motor no Windows (CI windows-latest).
- [x] Diagnóstico escrito: build, TPM, Secure Boot, VBS/HVCI, Vanguard, spooler, WebView2, App Installer, inicialização, serviços desativados (falta testar em Windows real).
- [ ] Diagnóstico: IOMMU, tarefas e appx ausentes.
- [ ] Ponto de restauração: testar criação, limite de 24 h e proteção desativada.
- [ ] Registro via reg.exe: testar ler/escrever/restaurar/apagar.
- [x] Detectar instância do Minecraft aberta no Windows (WMI; escrito, falta testar em Windows real).
- [ ] Gerenciador de inicialização com descrição e reversão.

## Não esquecer

- Nada de percentuais de ganho; "não medido" quando não houver medição.
- O app nunca encerra processos nem toca em mundos sem backup próprio.
