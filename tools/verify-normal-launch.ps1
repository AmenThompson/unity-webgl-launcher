[CmdletBinding()]
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$Screenshot
)

$ErrorActionPreference = 'Stop'
$exePath = (Resolve-Path -LiteralPath $Exe).Path
$capture = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Screenshot)
New-Item -ItemType Directory -Force (Split-Path -Parent $capture) | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class LauncherSmoke {
  [StructLayout(LayoutKind.Sequential)]
  public struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
}
'@

# No debug switches or runtime-directory overrides. Use a different current directory.
$app = Start-Process -FilePath $exePath -WorkingDirectory $env:SystemRoot -PassThru
try {
  $deadline = [DateTime]::UtcNow.AddSeconds(20)
  do {
    Start-Sleep -Milliseconds 200
    $app.Refresh()
    if ($app.HasExited) { throw "应用提前退出，退出码 $($app.ExitCode)。" }
  } while ($app.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $deadline)
  if ($app.MainWindowHandle -eq 0) { throw '等待应用窗口超时。' }
  Start-Sleep -Seconds 3
  $window = $app.MainWindowHandle
  [void][LauncherSmoke]::ShowWindow($window, 9)
  [void][LauncherSmoke]::SetForegroundWindow($window)
  Start-Sleep -Milliseconds 350
  if ([LauncherSmoke]::GetForegroundWindow() -ne $window) { throw '无法把应用置于前台以取证。' }
  $rect = [LauncherSmoke+Rect]::new()
  if (![LauncherSmoke]::GetWindowRect($window, [ref]$rect)) { throw '无法读取窗口位置。' }
  $width = $rect.Right - $rect.Left - 16
  $height = $rect.Bottom - $rect.Top - 16
  $bitmap = [Drawing.Bitmap]::new($width, $height)
  try {
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try { $graphics.CopyFromScreen($rect.Left + 8, $rect.Top + 8, 0, 0, $bitmap.Size) }
    finally { $graphics.Dispose() }
    $bitmap.Save($capture, [Drawing.Imaging.ImageFormat]::Png)
  } finally { $bitmap.Dispose() }
  $app.Refresh()
  [pscustomobject]@{ WindowTitle = $app.MainWindowTitle; Screenshot = $capture }
} finally {
  $app.Refresh()
  if (!$app.HasExited) {
    [void]$app.CloseMainWindow()
    if (!$app.WaitForExit(5000)) { Stop-Process -Id $app.Id }
  }
}
