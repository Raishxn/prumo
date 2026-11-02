# Status — 2026-11-01

## Pronto e verificado (Linux, CachyOS)

| Item | Como foi verificado |
|---|---|
| Motor: planejar, prévia, aplicar, validar por operação, desfazer sessão/ação | 9 testes de integração (`crates/prumo-core/tests/engine.rs`) |
| Idempotência, falha parcial com reversão, dependências, conflitos | testes |
| Diário write-ahead e recuperação de sessão interrompida | teste simula queda |
| Desfazer não sobrescreve alteração externa sem `--forcar` | teste |
| Backup com SHA-256 conferido | teste com backup adulterado |
| Edição INI preservando o arquivo | 4 testes unitários |
| Leitura das instâncias do Prism e detecção de instância aberta | teste + execução real (43 instâncias) |
| Catálogo embutido validado (fontes, ids, parâmetros, regras de privilégio) | teste |
| Diagnóstico Linux real (sem inventar; desconhecidos com motivo) | executado neste PC, 1,5 s |
| Prévia real do plano recomendado | executada neste PC (nada aplicado) |

Total: **18 testes passando** (`cargo test`).

## Escrito, mas NÃO testado

- Backend Windows (registro via `reg.exe`, diagnóstico WMI, ponto de restauração
  verificado): código existe, **não compilado nem executado em Windows**.
- Nenhuma ação foi aplicada de verdade neste PC ainda (só prévia). Para aplicar o
  plano recomendado no CachyOS: `prumo aplicar` (pede confirmação).

## Ainda não existe

Interface gráfica, playbook AME, medição/relatório HTML, executor elevado, snapshot
Btrfs, teste de rede, perfil do OBS, fluxo de drivers, ISO customizada. Ver
[ROADMAP.md](ROADMAP.md) e [TODO.md](TODO.md).

## Achados do diagnóstico deste PC

1. GPU em **PCIe x8** (placa e porta suportam x16) — verificar com carga.
2. **9–12 GB em swap** durante o uso normal (vários Java, OBS, navegador).
3. Cache de shaders NVIDIA com **993 MB** (perto do limite) — ação recomendada.
4. **Secure Boot desligado** e rEFInd não inicia com ele ligado — plano em
   `docs/SECURE-BOOT.md`.
5. Conexão é **Ethernet 1 Gb/s** (não Wi-Fi).
6. OBS sem bitrate definido no perfil (usa o padrão do encoder).
7. Monitor em HDMI sem VRR.
