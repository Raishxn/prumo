@echo off
rem Roda como SYSTEM no fim da instalação. Registro em C:\Windows\Setup\Scripts\prumo.log
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0remover-appx.ps1" > "%~dp0prumo.log" 2>&1
rem Reduz o espaço ocupado e evita NTFS travado para o Linux no dual boot.
>> "%~dp0prumo.log" 2>&1 (
  powercfg /h off
  dism /Online /Set-ReservedStorageState /State:Disabled
  dism /Online /Cleanup-Image /StartComponentCleanup /ResetBase
)
