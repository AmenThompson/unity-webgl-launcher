[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $root
try {
  $package = Get-Content -Raw -LiteralPath 'package.json' | ConvertFrom-Json
  $tauri = Get-Content -Raw -LiteralPath 'src-tauri/tauri.conf.json' | ConvertFrom-Json
  $cargo = Select-String -LiteralPath 'src-tauri/Cargo.toml' -Pattern '^version = "([^\"]+)"$'
  if ($cargo.Count -ne 1 -or $package.version -ne $tauri.version -or $package.version -ne $cargo.Matches[0].Groups[1].Value) {
    throw 'package.json、Tauri 与 Cargo 的应用版本不一致。'
  }

  # Explicitly select x64 regardless of the active Rust host target.
  & npm run tauri -- build --no-bundle --target x86_64-pc-windows-msvc -- --locked
  if ($LASTEXITCODE -ne 0) { throw "Tauri 构建失败 ($LASTEXITCODE)。" }

  $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'src-tauri/target' }
  if (![IO.Path]::IsPathRooted($target)) { $target = Join-Path (Join-Path $root 'src-tauri') $target }
  $source = Join-Path $target 'x86_64-pc-windows-msvc/release/unity-webgl-launcher.exe'
  $output = Join-Path $root "release/$($package.version)"
  $name = "Unity-WebGL-Launcher_$($package.version)_x64.exe"
  New-Item -ItemType Directory -Force $output | Out-Null
  $destination = Join-Path $output $name
  $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash
  if (Test-Path -LiteralPath $destination) {
    if ($hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash) {
      throw '发布目录已有同版本但不同内容的 EXE。请先按版本规则递增版本，或由维护者处理未发布的构建。'
    }
  } else {
    Copy-Item -LiteralPath $source -Destination $destination
    if ($hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash) {
      throw '最终 EXE 与构建原件的 SHA-256 不一致。'
    }
  }
  "$($hash.ToLowerInvariant()) *$name" | Set-Content -Encoding ascii -LiteralPath (Join-Path $output 'SHA256SUMS.txt')
  Write-Output $destination
} finally {
  Pop-Location
}
