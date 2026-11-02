# Instalação limpa do Windows 11 sem perder o CachyOS

O SSD é compartilhado. Um erro no instalador do Windows apaga o Linux. Siga isto com
calma. Layout medido em 2026-11-01 (`lsblk`, disco `sda`, XrayDisk 1 TB):

| Partição | Tamanho | Tipo | O que é | No instalador do Windows |
|---|---|---|---|---|
| sda1 | 100 MB | FAT32 (EFI) | EFI do **Windows** | não apagar; o instalador reutiliza |
| sda2 | 16 MB | MSR | reservada da Microsoft | não mexer |
| **sda3** | **250,6 GB** | NTFS | **C: atual do Windows** | **única que pode ser formatada** |
| sda4 | 851 MB | NTFS | recuperação do Windows | pode deixar como está |
| sda5 | 2 GB | FAT32 (EFI) | **/boot do Linux (rEFInd, systemd-boot, kernels)** | **NUNCA** apagar/formatar |
| sda6 | 678 GB | Btrfs | **CachyOS (/, /home, jogos, Minecraft)** | **NUNCA** apagar/formatar |

No instalador, as partições aparecem como "Unidade 0 Partição N". Confira pelo
**tamanho**: só a de ~250 GB (NTFS) é a sua C:. Se aparecer algo diferente da tabela,
**pare**.

## Antes de trocar para o Windows

1. No Linux: copie para fora do SSD (pendrive/nuvem) o que for insubstituível. A
   instalação não deve tocar no Linux, mas um clique errado sim.
2. Salve do Windows atual o que precisar (C: será formatada): saves, configs do
   Valorant/LoL ficam na nuvem da Riot; documentos, `Office 2010` (chave/instalador),
   drivers da Epson L3250 (baixáveis da Epson).
3. Se o BitLocker estiver ativo no Windows atual, não importa (C: será formatada).
4. Baixe a ISO **oficial** do Windows 11 (microsoft.com/software-download/windows11)
   e grave com o Media Creation Tool, Rufus ou Ventoy.
5. **Opcional e mais seguro**: desconectar não é possível (mesmo SSD), então o cuidado
   está todo na tela de partições.

## Durante a instalação

1. Boot pelo pendrive (menu de boot do firmware). Secure Boot pode ficar **ligado**
   (o instalador oficial é assinado).
2. "Instalação personalizada".
3. Selecione **somente** a partição de ~250 GB → **Formatar** (ou Excluir e depois
   "Novo" no espaço vazio *dessa mesma* área). Não toque nas outras.
4. Instale nela.
5. Na configuração inicial, a criptografia de dispositivo pode ser ativada
   automaticamente se você entrar com conta Microsoft. Se ativar, **anote a chave de
   recuperação** (necessária se um dia mexer no Secure Boot — ver `SECURE-BOOT.md`).

## Depois: devolver o boot ao rEFInd

O Windows costuma se colocar em primeiro na ordem de boot. As entradas do Linux
continuam na NVRAM. Para voltar:

- Pela BIOS: ordem de boot → `rEFInd Boot Manager` em primeiro; **ou**
- Do CachyOS (entrando pelo menu de boot do firmware): `sudo efibootmgr -o 0001,0000,0003,0002`
  (confira os números com `efibootmgr`; hoje 0001 = rEFInd, 0000 = Windows).

Lembrete: com Secure Boot ligado o rEFInd ainda não inicia (ver `SECURE-BOOT.md`).

## Depois: no Windows novo

1. Driver NVIDIA pelo site oficial (instalação limpa já é limpa; DDU não é
   necessário numa instalação nova).
2. Chipset AMD (site da AMD), Epson L3250 (site da Epson).
3. Programas: Chrome, Discord, NVIDIA Broadcast, Office 2010, Lightshot, Riot Client.
4. Não aplicar playbook ainda: a etapa 3/4 do Prumo vai inventariar e aplicar com
   ponto de restauração verificado. Rodar `prumo diagnostico` quando a etapa 3 sair.

## ISO customizada leve (planejado — etapa 8)

Ver [ROADMAP](../ROADMAP.md). Não fica pronta para esta instalação; a recomendação
agora é a ISO oficial + playbook depois.
