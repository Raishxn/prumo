# Dependências e licenças

Prumo: MIT. Todas as dependências da etapa 1 são permissivas e compiladas no binário
(nenhuma ferramenta ou driver de terceiros é redistribuído).

| Crate | Uso | Licença |
|---|---|---|
| serde, serde_json, toml | catálogo e diário | MIT OR Apache-2.0 |
| anyhow | erros | MIT OR Apache-2.0 |
| sha2 | hash dos backups | MIT OR Apache-2.0 |
| time | datas | MIT OR Apache-2.0 |
| tempfile (só testes) | diretórios temporários | MIT OR Apache-2.0 |

Transitivas (cargo metadata, 2026-11-01): todas MIT, Apache-2.0, Unlicense OR MIT,
Unicode-3.0 ou combinações dessas; nenhuma copyleft obrigatória.

## Referência

- [ResetExplode](https://github.com/GustavoReset/ResetExplode) (MIT, © 2026 Gustavo) —
  usado como referência de funcionalidades e experiência. Nenhum código foi copiado.

## Ferramentas externas previstas (não redistribuídas)

| Ferramenta | Etapa | Licença | Como entra |
|---|---|---|---|
| MangoHud (já instalado) | 5 | MIT | chamado se presente |
| PresentMon | 5 | MIT | baixado da release oficial com verificação de hash, ou orientação |
| DDU | 7 | freeware, redistribuição restrita | só orientação + link oficial |
| Drivers NVIDIA | 7 | proprietária | download do site da NVIDIA pelo usuário/app, nunca empacotado |
