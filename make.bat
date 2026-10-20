@echo off
setlocal

set "ROOT=%~dp0"
set "OUT=%ROOT%dist"

echo === Building tet-rs.exe ===
cargo build --release

echo === Creating dist ===
mkdir "dist/assets/skins"
copy target\release\tet-rs.exe dist\
copy assets\skins\default.png dist\assets\skins\
copy settings.toml dist\

echo.
echo === Done. Published to "%OUT%" ===
dir /b "%OUT%"
