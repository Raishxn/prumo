#!/usr/bin/env bash
# Uso: iso/gerar-iso.sh <Win11-oficial.iso> <saida.iso> [usuario]
# Mantém só a edição Pro, comprimida; não burla TPM nem Secure Boot.
set -euo pipefail
[ $# -ge 2 ] || { echo "uso: $0 <origem.iso> <saida.iso> [usuario]" >&2; exit 2; }
ORIGEM=$1 SAIDA=$2 USUARIO=${3:-Usuario}
AQUI=$(cd "$(dirname "$0")" && pwd)
[[ $USUARIO =~ ^[A-Za-z0-9_-]{1,20}$ ]] || { echo "usuário inválido" >&2; exit 2; }
[ -f "$ORIGEM" ] || { echo "ISO de origem não encontrada" >&2; exit 2; }
[ ! -e "$SAIDA" ] || { echo "saída já existe, não sobrescrevo: $SAIDA" >&2; exit 2; }
for b in 7z mkisofs wimlib-imagex wiminfo sha256sum; do command -v "$b" >/dev/null || { echo "falta: $b" >&2; exit 2; }; done

T=$(mktemp -d); trap 'chmod -R u+w "$T"; rm -rf "$T"' EXIT
7z x -y -bd -o"$T" "$ORIGEM" >/dev/null; chmod -R u+w "$T"
for f in boot/etfsboot.com efi/microsoft/boot/efisys.bin sources/boot.wim; do
    [ -f "$T/$f" ] || { echo "não parece mídia do Windows: falta $f" >&2; exit 1; }
done
# Uma edição só, com compressão sólida: é daqui que vem a redução da ISO.
IDX=$(wiminfo "$T/sources/install.wim" | awk '/^Index:/{i=$2} /^Edition ID:/{if($3=="Professional")print i}')
[ -n "$IDX" ] || { echo "edição Pro não encontrada na ISO" >&2; exit 1; }
wimlib-imagex export "$T/sources/install.wim" "$IDX" "$T/sources/install.esd" --solid
rm -f "$T/sources/install.wim"
# WebView2 e EdgeUpdate ficam: Broadcast depende do primeiro, e ele se atualiza pelo segundo.
printf '%s\n' 'delete --force /Windows/System32/OneDriveSetup.exe' \
    'delete --force --recursive "/Program Files (x86)/Microsoft/Edge"' \
    'delete --force --recursive "/Program Files (x86)/Microsoft/EdgeCore"' | wimlib-imagex update "$T/sources/install.esd" 1 --rebuild
sed "s/@@USUARIO@@/$USUARIO/" "$AQUI/autounattend.xml" > "$T/autounattend.xml"
S="$T/sources/\$OEM\$/\$\$/Setup/Scripts"; mkdir -p "$S"
cp "$AQUI/oem/"* "$AQUI/remover-appx.txt" "$S/"

mkisofs -quiet -iso-level 3 -udf -J -joliet-long -D -N \
    -V PRUMO_WIN11 \
    -b boot/etfsboot.com -no-emul-boot -boot-load-size 8 -boot-info-table \
    -eltorito-alt-boot -eltorito-platform efi -b efi/microsoft/boot/efisys.bin -no-emul-boot \
    -o "$SAIDA" "$T"
{ sha256sum "$ORIGEM"; sha256sum "$SAIDA"; } | tee "$SAIDA.sha256"
