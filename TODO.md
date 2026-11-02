# TODO

## Respostas que faltam (bloqueiam decisões)

- [x] ~~L1 Secure Boot~~ — respondido: com Secure Boot ligado o rEFInd não inicia;
      o Windows é aberto pelo menu do firmware. → plano em `docs/SECURE-BOOT.md`.
- [ ] L2 Qual instância GTNH (2.9.0 final? RC-2?) e qual GTL você joga de fato.
- [ ] L3 O monitor (EDID "HKM M238W1F2556", vendido como TGT Altay T3H) tem entrada
      DisplayPort? Hoje está em HDMI e o Niri informa VRR não suportado.
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

- [ ] Executor elevado via pkexec (só ids do catálogo + parâmetros validados).
- [ ] Snapshot Btrfs verificado antes de ações admin (layout: `@` em `/dev/sda6`).
- [ ] Escolha de alvos por ação (`--alvos`), p.ex. excluir "[BACKUP] RC-2".
- [ ] Teste de rede guiado (latência/jitter/perda, upload sob autorização).
- [ ] Perfil novo do OBS (cópia; nunca ler/copiar `service.json`).
- [ ] Secure Boot: ler as fontes listadas e validar o caminho shim + MOK.

## Etapa 3 (Windows) — prioridade

- [ ] `cargo check --target x86_64-pc-windows-msvc` no Windows; testes do motor lá.
- [ ] Diagnóstico: build, TPM, Secure Boot, VBS/HVCI, IOMMU, serviços/tarefas/appx
      ausentes (inventário VainOS com origem "incerta").
- [ ] Ponto de restauração: testar criação, limite de 24 h e proteção desativada.
- [ ] Registro via reg.exe: testar ler/escrever/restaurar/apagar.
- [ ] Gerenciador de inicialização com descrição e reversão.

## Não esquecer

- Nada de percentuais de ganho; "não medido" quando não houver medição.
- O app nunca encerra processos nem toca em mundos sem backup próprio.
