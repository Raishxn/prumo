# ISO do Windows 11 (etapa 8)

**Nunca testada.** Valide em VM com TPM e Secure Boot antes de usar no PC.

    iso/gerar-iso.sh Win11_pt-br.iso Prumo-Win11.iso SeuUsuario

Precisa de `7zip` (7z, lê UDF) e `cdrtools` (mkisofs), `wimlib`. Mantém só a Pro em `install.esd`, retira o OneDrive e acrescenta `autounattend.xml` e `SetupComplete.cmd`.

- Partição: sempre manual. Use **só a de ~250 GB** (`docs/WINDOWS-INSTALACAO-LIMPA.md`).
- BitLocker: criptografia automática impedida (`PreventDeviceEncryption=1`).
- Conta local **sem senha**: defina uma depois de instalar.
- Apps removidos: `remover-appx.txt`; resultado em `C:\Windows\Setup\Scripts\prumo.log`.
- Instala a Pro com a chave genérica pública; a ativação vem da licença digital do PC.
