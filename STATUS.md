# Status — 2026-11-02

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

| Cópia sem sobrescrever + edição + desfazer (formato do perfil do OBS); segredo fora do diário | teste em diretório temporário |
| Parâmetro vindo do diagnóstico (`fact:`): ausente/inválido deixa a ação incompatível | teste |
| Cálculo do bitrate e leitura do `ping` | testes unitários |

`--excluir` foi conferido na prévia real.

Total: **31 testes passando** (`cargo test`).

## Escrito, mas NÃO testado

- Backend Windows (registro via `reg.exe`, diagnóstico WMI, ponto de restauração
  verificado): **compila e os 18 testes passam no Windows do CI** (GitHub Actions,
  windows-latest), mas essas funções específicas **nunca rodaram num Windows real**
  (o CI não cria ponto de restauração nem altera registro).
- Nenhuma ação foi aplicada de verdade neste PC ainda (só prévia). Para aplicar o
  plano recomendado no CachyOS: `prumo aplicar` (pede confirmação).

- `prumo rede`: **nunca executado de verdade** (envia 75 MB à Cloudflare; aguarda sua
  autorização). Até lá, o perfil do OBS aparece como incompatível no plano.
- Perfil `Prumo Live 1080p60`: testado só em diretório temporário; nunca aberto no OBS.
- Detecção de VRR pelo EDID: leitura testada com saídas escritas à mão, não com um EDID real.
- Instâncias do Prism no Windows: `prism::scan` agora roda lá, com lista de processos via WMI; nunca executado em Windows real. Se a lista falhar, nenhuma instância entra no plano. Java aberto como administrador pode passar despercebido se o Prumo rodar sem elevação.

## Interface gráfica (`prumo-gui`)

Páginas Início, Diagnóstico, Plano e Histórico. Abriu no Niri/Wayland; aplicar e desfazer pela interface nunca foram clicados. Compartilha `engine::apply_protected` com a CLI. Memória medida ~5 s após abrir: 167 MB (meta de 60 MB **não atingida**). Binário Linux: 7,5 MB. No Windows nunca foi aberta.

## Ainda não existe

Medição e rede na interface, playbook AME, executor elevado, snapshot
Btrfs, fluxo de drivers, ISO customizada. Ver
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
