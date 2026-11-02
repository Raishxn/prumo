# Prumo

Otimizador pessoal para **Windows 11** e **CachyOS**, focado em **consistência de
frame time** (menos stuttering), com diagnóstico da máquina real, explicação de cada
ajuste, ponto de restauração/backups verificados e reversão.

> Status: **etapa 1 de 8**. Núcleo, CLI e diagnóstico Linux funcionam e têm testes.
> Windows, interface gráfica, playbook AME e relatório HTML ainda não. Detalhes em
> [STATUS.md](STATUS.md) · [ROADMAP.md](ROADMAP.md) · [TODO.md](TODO.md).

Este projeto é feito para um PC específico (Ryzen 7 5700X, RTX 3060 Ti, MAXSUN
B550M, dual boot). Nada aqui promete ganho universal; cada ação diz a evidência que
tem, e "não medido" quando não há medição.

## Princípios

- Só aplica o que o diagnóstico mostrou ser compatível; prévia antes de tudo.
- Guarda o **estado anterior real** antes de cada alteração (write-ahead) e desfaz por
  sessão ou por ação.
- Nunca desliga segurança exigida pelo Vanguard (TPM, Secure Boot, VBS/HVCI, IOMMU),
  nunca encerra processos, nunca mexe em partições/bootloader.
- Sem serviço residente: roda, aplica e fecha.

## Compilar

Requer Rust estável (o repositório fixa `stable` em `rust-toolchain.toml`).

```sh
cargo build --release      # binário em target/release/prumo
cargo test                 # 18 testes
```

## Usar (CLI)

```sh
prumo diagnostico          # inventário (somente leitura); --json para dados brutos
prumo plano                # prévia do plano recomendado: atual → proposto
prumo explicar <id>        # os 10 itens de documentação de uma ação
prumo aplicar              # aplica com confirmação; mostra o que precisa reiniciar
prumo sessoes              # histórico
prumo desfazer ultima      # reverte a última sessão (ou --acao <id>)
prumo recuperar            # sessões interrompidas
```

Ações avançadas só entram com seleção explícita:
`prumo plano --acoes linux.minecraft.gtl-heap-fixo --param linux.minecraft.gtl-heap-fixo.heap_mb=10240`.

## Documentação

- [Desenho, lacunas, arquitetura, matriz de otimizações e etapas](docs/PROJETO.md)
- [Recuperação sem o aplicativo](docs/RECUPERACAO.md)
- [Instalação limpa do Windows 11 sem perder o CachyOS](docs/WINDOWS-INSTALACAO-LIMPA.md)
- [Secure Boot no dual boot](docs/SECURE-BOOT.md)
- [Dependências e licenças](docs/LICENCAS.md)
- Catálogo de ações: [`catalog/linux.toml`](catalog/linux.toml), [`catalog/windows.toml`](catalog/windows.toml)

## Estrutura

```
crates/prumo-core   catálogo, diagnóstico, motor, diário, proteção
crates/prumo-cli    binário `prumo`
catalog/            definições das ações (compartilhadas com o futuro playbook)
docs/               documentação
```

## Licença

MIT. Referência de funcionalidades: [ResetExplode](https://github.com/GustavoReset/ResetExplode) (MIT); nenhum código copiado.
