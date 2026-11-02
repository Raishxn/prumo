# Recuperação sem o aplicativo

Estas instruções funcionam mesmo que a interface (ou o próprio `prumo`) não abra.

## Onde ficam os dados

| Sistema | Pasta |
|---|---|
| Linux, usuário | `~/.local/state/prumo/sessions/<sessão>/` |
| Linux, ações administrativas | `/var/lib/prumo/sessions/<sessão>/` |
| Windows | `C:\ProgramData\Prumo\sessions\<sessão>\` |

Cada sessão tem:

- `journal.json` — lista de alterações (`entries`), em ordem. Para cada uma: a operação
  (`op`), o estado anterior real (`pre`) e o status (`applied`, `undone`, `intent` …).
- `backups/NNNN.bak` — cópia completa do arquivo antes da alteração NNNN.

## Com o `prumo` funcionando

```sh
prumo sessoes                 # lista
prumo recuperar               # mostra sessões interrompidas
prumo desfazer ultima         # desfaz a última sessão com alterações
prumo desfazer <sessão> --acao <id>
```

## À mão (sem o `prumo`)

Percorra `entries` **de trás para frente** e, para cada `status` igual a `applied` ou
`intent`:

| `pre.kind` | O que fazer |
|---|---|
| `file_absent` | apague o arquivo `op.path` (ele não existia antes) |
| `file_content` | copie `backups/<pre.backup>` por cima de `op.path` |
| `ini_value` | copie `backups/<pre.backup>` por cima de `op.path` (cópia do arquivo inteiro) |
| `reg_value` (Windows) | `pre.previous` nulo → `reg delete "<path>" /v <name> /f`; senão `reg add "<path>" /v <name> /t <previous_kind> /d <previous> /f` |

Exemplo Linux para a ação do cache de shaders:

```sh
rm ~/.config/environment.d/90-prumo-nvidia-shader-cache.conf   # e reabra a sessão
```

Exemplo para uma instância do Prism (sem o app):

```sh
S=~/.local/state/prumo/sessions/<sessão>
python3 -c "import json;[print(e['seq'],e['op']['path'],e['pre'].get('backup')) for e in json.load(open('$S/journal.json'))['entries']]"
cp "$S/backups/0002.bak" "<caminho do instance.cfg mostrado acima>"
```

## O que estas cópias NÃO cobrem

- Arquivos e configurações que o app não alterou (os backups são específicos).
- No Windows, o ponto de restauração cobre registro e arquivos de sistema, mas não
  documentos pessoais; e o Windows só permite um ponto a cada 24 h por padrão.
- Mundos do Minecraft: nenhuma ação desta etapa toca em mundos. Ações futuras que
  tocarem farão backup próprio antes.
- Partições, bootloader (rEFInd) e entradas de boot: o app **nunca** altera.

## Se o Linux não iniciar a sessão gráfica

As ações desta etapa só mexem em arquivos do usuário. Num TTY (Ctrl+Alt+F3), faça
login e rode `prumo desfazer ultima` ou apague os arquivos listados acima.
