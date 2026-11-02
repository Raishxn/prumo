# Secure Boot no dual boot (CachyOS + Windows 11)

Status: **plano a validar — nada disto é automatizado pelo app.** Mexer em chaves do
firmware é a operação mais arriscada do projeto; cada passo exige confirmação sua.

## Situação atual (medida em 2026-11-01)

- Firmware com **Secure Boot desativado**; TPM 2.0 presente.
- Entradas UEFI: `rEFInd Boot Manager` (primeira), `Linux Boot Manager`
  (systemd-boot), `Windows Boot Manager`, todas em `sda5` exceto o Windows (`sda1`).
- Relato: com Secure Boot **ligado**, o rEFInd não inicia (binários não assinados por
  uma chave que o firmware aceite). Você entra no Windows pelo menu de boot do
  firmware (DEL). O Valorant/Vanguard exige o Secure Boot ligado
  ([Riot — Vanguard Security Requirements](https://support.riotgames.com/en-us/riot/performance/vanguard-security-requirements)).

Ou seja: hoje é preciso escolher entre Linux e Valorant a cada troca. O objetivo é
**deixar o Secure Boot sempre ligado** e fazer o CachyOS iniciar assim mesmo.

## Caminhos possíveis

| Caminho | Como funciona | Risco | Manutenção |
|---|---|---|---|
| **A. shim + MOK** | `shim` (assinado pela Microsoft) carrega o rEFInd/kernel assinados com uma chave sua (MOK), registrada pelo MokManager no primeiro boot | baixo: não mexe nas chaves do firmware | hook do pacman re-assina o kernel a cada atualização |
| **B. sbctl com chaves próprias** | firmware em Setup Mode, `sbctl create-keys`, `sbctl enroll-keys -m` (mantém as chaves da Microsoft), assina rEFInd e kernels | médio: exige apagar/registrar chaves no firmware; sem `-m` a GPU (option ROM) e o Windows podem não iniciar | `sbctl` instala hook que re-assina |
| C. Trocar de bootloader | usar só systemd-boot/UKI assinados | médio | depende |
| D. Continuar como está | Secure Boot liga/desliga por troca | — | incômodo diário |

**Recomendação inicial: A (shim + MOK)**, por não alterar as chaves do firmware MAXSUN
(firmware de fabricante pouco documentado; reduzir a chance de deixar o PC sem vídeo
ou sem boot). B fica como alternativa se o shim não funcionar com o rEFInd instalado.

## Antes de qualquer tentativa (obrigatório)

1. **BitLocker/criptografia do Windows**: trocar o estado do Secure Boot muda as
   medições do TPM (PCR 7) e pode pedir a **chave de recuperação do BitLocker**. Anote
   a chave (conta Microsoft → Dispositivos → Chaves de recuperação, ou
   `manage-bde -protectors -get C:` no Windows) **antes** de mexer. Numa instalação
   limpa do 11 24H2+, a criptografia de dispositivo pode ser ligada automaticamente.
2. Pendrive com a ISO do CachyOS (live) para recuperar o boot se necessário.
3. Saber entrar no menu de boot do firmware (DEL / tecla de boot da MAXSUN) e onde fica
   "Secure Boot" e "Restore Factory Keys" na BIOS.
4. Backup de `/boot` (`sda5`).

## Validação

- Secure Boot ligado → CachyOS inicia pelo rEFInd; `bootctl status` e
  `mokutil --sb-state` mostram `SecureBoot enabled`.
- Windows inicia pelo rEFInd e pelo menu do firmware; Valorant abre sem VAN.
- Após atualizar o kernel (`pacman -Syu`), o boot continua funcionando.

## Recuperação

- Desligar Secure Boot na BIOS volta à situação de hoje (nada no disco é apagado).
- Caminho B: "Restore Factory Keys" na BIOS restaura as chaves originais.
- Boot do Linux quebrado: iniciar pelo pendrive live, `mount /dev/sda6 -o subvol=@ /mnt`,
  `mount /dev/sda5 /mnt/boot`, `arch-chroot /mnt` e reinstalar/re-assinar.

## Fontes a consultar antes de implementar (pendente)

- rEFInd — Managing Secure Boot (Rod Smith): <https://www.rodsbooks.com/refind/secureboot.html>
- CachyOS Wiki — Secure Boot: <https://wiki.cachyos.org/configuration/secure_boot_setup/>
- ArchWiki — UEFI/Secure Boot: <https://wiki.archlinux.org/title/Unified_Extensible_Firmware_Interface/Secure_Boot>
- sbctl: <https://github.com/Foxboron/sbctl>

Estas páginas **ainda não foram lidas** nesta etapa; o plano acima é o ponto de partida
e será revisado com elas antes de qualquer passo.
