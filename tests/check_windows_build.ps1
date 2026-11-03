<#
    Swiftify, a spotify player for desktop
    Copyright (C) 2026  shred0s

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
#>

# checks tools/build-windows.ps1 before the workflow spends twenty minutes
# building with it: that it parses, that it still names what the workflow reads
# afterwards, and that a Qt path that is not there is refused at once rather
# than half way through.
#
# runs before Qt is installed, so it needs nothing but powershell.

[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

$root = Split-Path -Parent $PSScriptRoot
$buildScript = Join-Path $root 'tools\build-windows.ps1'
$failures = [System.Collections.Generic.List[string]]::new()

function Add-Failure {
    param([string]$Message)
    $failures.Add($Message)
}

if (-not (Test-Path -LiteralPath $buildScript -PathType Leaf)) {
    Write-Host "FAILED: $buildScript is missing"
    exit 1
}

# 1. it parses, so a broken edit is caught here rather than in the build
$tokens = $null
$errors = $null
[void][System.Management.Automation.Language.Parser]::ParseFile($buildScript, [ref]$tokens, [ref]$errors)
foreach ($error in $errors) {
    Add-Failure "the build script does not parse: $($error.Message) (line $($error.Extent.StartLineNumber))"
}

# 2. what the workflow and the app read out of the installed folder is still
# named the same
$text = Get-Content -LiteralPath $buildScript -Raw
foreach ($needed in @(
        'swiftify.exe',
        'Qt6Core.dll',
        'qwindows.dll',
        'share\licenses\tubify',
        'SWIFTIFY_WINDOWS_DEPLOY_QT',
        'windeployqt.exe')) {
    if ($text.IndexOf($needed, [StringComparison]::Ordinal) -lt 0) {
        Add-Failure "the build script no longer mentions $needed"
    }
}

# 3. a Qt path that is not there is refused at once, and says so
$shells = @(Get-Command -Name 'powershell.exe', 'pwsh.exe' -CommandType Application -ErrorAction SilentlyContinue)
if ($shells.Count -eq 0) {
    Add-Failure 'no powershell to run the build script with'
} else {
    $output = & $shells[0].Source -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $buildScript -QtPath 'C:\tubify-no-such-qt' 2>&1
    $code = $LASTEXITCODE
    if ($code -eq 0) {
        Add-Failure 'the build script accepted a Qt path that is not there'
    } elseif (($output | Out-String) -notmatch 'Qt') {
        Add-Failure 'the refusal of a Qt path that is not there did not mention Qt'
    }
}

if ($failures.Count -gt 0) {
    foreach ($failure in $failures) {
        Write-Host "FAILED: $failure"
    }
    exit 1
}

Write-Host 'the windows build script is as the workflow expects it'
