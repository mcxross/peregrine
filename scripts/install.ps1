[CmdletBinding()]
param(
    [string]$Release = $env:PEREGRINE_RELEASE
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

if ([string]::IsNullOrWhiteSpace($Release)) {
    $Release = "latest"
}

$NonInteractive = $env:PEREGRINE_NON_INTERACTIVE -match "^(?i:1|true|yes)$"

function Write-Step {
    param(
        [string]$Message
    )

    Write-Host "==> $Message"
}

function Write-WarningStep {
    param(
        [string]$Message
    )

    Write-Warning $Message
}

function Prompt-YesNo {
    param(
        [string]$Prompt
    )

    if ($NonInteractive) {
        return $false
    }

    if ([Console]::IsInputRedirected -or [Console]::IsOutputRedirected) {
        return $false
    }

    $choice = Read-Host "$Prompt [y/N]"
    return $choice -match "^(?i:y(?:es)?)$"
}

function Normalize-Version {
    param(
        [string]$RawVersion
    )

    if ([string]::IsNullOrWhiteSpace($RawVersion) -or $RawVersion -eq "latest") {
        return "latest"
    }

    if ($RawVersion.StartsWith("v")) {
        return $RawVersion.Substring(1)
    }

    return $RawVersion
}

function Assert-ValidReleaseVersion {
    param(
        [string]$Version
    )

    if ($Version -cne "latest" -and $Version -cnotmatch "^[0-9]+\.[0-9]+\.[0-9]+(?:-(?:alpha|beta)(?:\.[0-9]+)?)?$") {
        throw "Invalid Peregrine release version: $Version. Expected latest or x.y.z[-alpha[.N]|-beta[.N]]."
    }
}

function Get-ReleaseUrl {
    param(
        [string]$AssetName,
        [string]$ResolvedVersion
    )
    return "https://github.com/mcxross/peregrine/releases/download/v$ResolvedVersion/$AssetName"
}

function Get-ReleaseAssetDigest {
    param(
        [string]$AssetName,
        [string]$ResolvedVersion
    )

    $digestUrl = Get-ReleaseUrl -AssetName "$AssetName.sha256" -ResolvedVersion $ResolvedVersion
    try {
        $digestText = Invoke-RestMethod -Uri $digestUrl -ErrorAction Stop
    } catch {
        throw "Could not find SHA-256 digest for release asset $AssetName."
    }

    $match = [regex]::Match($digestText, "^([0-9a-fA-F]{64})")
    if (-not $match.Success) {
        throw "Could not parse SHA-256 digest for release asset $AssetName."
    }

    return $match.Groups[1].Value.ToLowerInvariant()
}

function Test-ArchiveDigest {
    param(
        [string]$ArchivePath,
        [string]$ExpectedDigest
    )

    $actualDigest = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualDigest -ne $ExpectedDigest) {
        throw "Downloaded Peregrine archive checksum did not match expected digest. Expected $ExpectedDigest but got $actualDigest."
    }
}

function Path-Contains {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    $needle = $Entry.TrimEnd("\")
    foreach ($segment in $PathValue.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)) {
        if ($segment.TrimEnd("\") -ieq $needle) {
            return $true
        }
    }

    return $false
}

function Prepend-PathEntry {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    $needle = $Entry.TrimEnd("\")
    $segments = @($Entry)
    if (-not [string]::IsNullOrWhiteSpace($PathValue)) {
        $segments += $PathValue.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries) |
            Where-Object { $_.TrimEnd("\") -ine $needle }
    }

    return ($segments -join ";")
}

function Invoke-WithInstallLock {
    param(
        [string]$LockPath,
        [scriptblock]$Script
    )

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $LockPath) | Out-Null
    $lock = $null
    while ($null -eq $lock) {
        try {
            $lock = [System.IO.File]::Open(
                $LockPath,
                [System.IO.FileMode]::OpenOrCreate,
                [System.IO.FileAccess]::ReadWrite,
                [System.IO.FileShare]::None
            )
        } catch [System.IO.IOException] {
            Start-Sleep -Milliseconds 250
        }
    }
    try {
        & $Script
    } finally {
        $lock.Dispose()
    }
}

function Remove-StaleInstallArtifacts {
    param(
        [string]$ReleasesDir
    )

    if (Test-Path -LiteralPath $ReleasesDir -PathType Container) {
        Get-ChildItem -LiteralPath $ReleasesDir -Force -Directory -Filter ".staging.*" -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Resolve-Version {
    $normalizedVersion = Normalize-Version -RawVersion $Release
    Assert-ValidReleaseVersion -Version $normalizedVersion
    if ($normalizedVersion -ne "latest") {
        return $normalizedVersion
    }

    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/mcxross/peregrine/releases/latest"
    if (-not $release.tag_name) {
        Write-Error "Failed to resolve the latest Peregrine release version."
        exit 1
    }

    $resolvedVersion = Normalize-Version -RawVersion $release.tag_name
    Assert-ValidReleaseVersion -Version $resolvedVersion
    return $resolvedVersion
}

function Get-VersionFromBinary {
    param(
        [string]$PeregrinePath
    )

    if (-not (Test-Path -LiteralPath $PeregrinePath -PathType Leaf)) {
        return $null
    }

    try {
        $versionOutput = & $PeregrinePath --version 2>$null
    } catch {
        return $null
    }

    if ($versionOutput -match '([0-9][0-9A-Za-z.+-]*)$') {
        return $matches[1]
    }

    return $null
}

function Get-CurrentInstalledVersion {
    param(
        [string]$StandaloneCurrentDir
    )

    $standaloneVersion = Get-VersionFromBinary -PeregrinePath (Join-Path $StandaloneCurrentDir "peregrine-tui.exe")
    if (-not [string]::IsNullOrWhiteSpace($standaloneVersion)) {
        return $standaloneVersion
    }

    return $null
}

function Add-JunctionSupportType {
    if (([System.Management.Automation.PSTypeName]'PeregrineInstaller.Junction').Type) {
        return
    }

    Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

namespace PeregrineInstaller
{
    public static class Junction
    {
        private const uint GENERIC_WRITE = 0x40000000;
        private const uint FILE_SHARE_READ = 0x00000001;
        private const uint FILE_SHARE_WRITE = 0x00000002;
        private const uint FILE_SHARE_DELETE = 0x00000004;
        private const uint OPEN_EXISTING = 3;
        private const uint FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;
        private const uint FILE_FLAG_OPEN_REPARSE_POINT = 0x00200000;
        private const uint FSCTL_SET_REPARSE_POINT = 0x000900A4;
        private const uint IO_REPARSE_TAG_MOUNT_POINT = 0xA0000003;
        private const int HeaderLength = 20;

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFileW(
            string lpFileName,
            uint dwDesiredAccess,
            uint dwShareMode,
            IntPtr lpSecurityAttributes,
            uint dwCreationDisposition,
            uint dwFlagsAndAttributes,
            IntPtr hTemplateFile);

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool DeviceIoControl(
            SafeFileHandle hDevice,
            uint dwIoControlCode,
            byte[] lpInBuffer,
            int nInBufferSize,
            IntPtr lpOutBuffer,
            int nOutBufferSize,
            out int lpBytesReturned,
            IntPtr lpOverlapped);

        public static void SetTarget(string linkPath, string targetPath)
        {
            string substituteName = "\\??\\" + Path.GetFullPath(targetPath);
            byte[] substituteNameBytes = Encoding.Unicode.GetBytes(substituteName);
            if (substituteNameBytes.Length > ushort.MaxValue - HeaderLength) {
                throw new ArgumentException("Junction target path is too long.", "targetPath");
            }

            byte[] reparseBuffer = new byte[substituteNameBytes.Length + HeaderLength];
            WriteUInt32(reparseBuffer, 0, IO_REPARSE_TAG_MOUNT_POINT);
            WriteUInt16(reparseBuffer, 4, checked((ushort)(substituteNameBytes.Length + 12)));
            WriteUInt16(reparseBuffer, 8, 0);
            WriteUInt16(reparseBuffer, 10, checked((ushort)substituteNameBytes.Length));
            WriteUInt16(reparseBuffer, 12, checked((ushort)(substituteNameBytes.Length + 2)));
            WriteUInt16(reparseBuffer, 14, 0);
            Buffer.BlockCopy(substituteNameBytes, 0, reparseBuffer, 16, substituteNameBytes.Length);

            using (SafeFileHandle handle = CreateFileW(
                linkPath,
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                IntPtr.Zero,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                IntPtr.Zero))
            {
                if (handle.IsInvalid) {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }

                int bytesReturned;
                if (!DeviceIoControl(
                    handle,
                    FSCTL_SET_REPARSE_POINT,
                    reparseBuffer,
                    reparseBuffer.Length,
                    IntPtr.Zero,
                    0,
                    out bytesReturned,
                    IntPtr.Zero))
                {
                    throw new Win32Exception(Marshal.GetLastWin32Error());
                }
            }
        }

        private static void WriteUInt16(byte[] buffer, int offset, ushort value)
        {
            buffer[offset] = (byte)value;
            buffer[offset + 1] = (byte)(value >> 8);
        }

        private static void WriteUInt32(byte[] buffer, int offset, uint value)
        {
            buffer[offset] = (byte)value;
            buffer[offset + 1] = (byte)(value >> 8);
            buffer[offset + 2] = (byte)(value >> 16);
            buffer[offset + 3] = (byte)(value >> 24);
        }
    }
}
"@
}

function Set-JunctionTarget {
    param(
        [string]$LinkPath,
        [string]$TargetPath
    )

    Add-JunctionSupportType
    [PeregrineInstaller.Junction]::SetTarget($LinkPath, $TargetPath)
}

function Test-IsJunction {
    param(
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }

    $item = Get-Item -LiteralPath $Path -Force
    return ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -and $item.LinkType -eq "Junction"
}

function Ensure-Junction {
    param(
        [string]$LinkPath,
        [string]$TargetPath,
        [string]$InstallerOwnedTargetPrefix
    )

    if (-not (Test-Path -LiteralPath $LinkPath)) {
        New-Item -ItemType Junction -Path $LinkPath -Target $TargetPath | Out-Null
        return
    }

    $item = Get-Item -LiteralPath $LinkPath -Force
    if (Test-IsJunction -Path $LinkPath) {
        $existingTarget = [string]$item.Target
        if (-not [string]::IsNullOrWhiteSpace($InstallerOwnedTargetPrefix)) {
            $ownedTargetPrefix = $InstallerOwnedTargetPrefix.TrimEnd("\\")
            if (-not $existingTarget.StartsWith($ownedTargetPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "Refusing to retarget junction at $LinkPath because it is not managed by this installer."
            }
        }
        if ($existingTarget.Equals($TargetPath, [System.StringComparison]::OrdinalIgnoreCase)) {
            return
        }

        Set-JunctionTarget -LinkPath $LinkPath -TargetPath $TargetPath
        return
    }

    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing to replace non-junction reparse point at $LinkPath."
    }

    if ($item.PSIsContainer) {
        if ((Get-ChildItem -LiteralPath $LinkPath -Force | Select-Object -First 1) -ne $null) {
            throw "Refusing to replace non-empty directory at $LinkPath with a junction."
        }

        Remove-Item -LiteralPath $LinkPath -Force
        New-Item -ItemType Junction -Path $LinkPath -Target $TargetPath | Out-Null
        return
    }

    throw "Refusing to replace file at $LinkPath with a junction."
}

function Test-ReleaseIsComplete {
    param(
        [string]$ReleaseDir,
        [string]$ExpectedVersion,
        [string]$ExpectedTarget
    )

    if (-not (Test-Path -LiteralPath $ReleaseDir -PathType Container)) {
        return $false
    }

    if ((Split-Path -Leaf $ReleaseDir) -ne "$ExpectedVersion-$ExpectedTarget") {
        return $false
    }

    if (-not (Test-Path -LiteralPath (Join-Path $ReleaseDir "peregrine-tui.exe") -PathType Leaf)) {
        return $false
    }

    return $true
}

function Get-ExistingPeregrineCommand {
    $existing = Get-Command peregrine -ErrorAction SilentlyContinue
    if ($null -eq $existing) {
        return $null
    }

    return $existing.Source
}

function Get-ConflictingInstall {
    param(
        [string]$VisibleBinDir
    )

    $existingPath = Get-ExistingPeregrineCommand
    if ([string]::IsNullOrWhiteSpace($existingPath)) {
        return $null
    }

    if ($existingPath.StartsWith($VisibleBinDir, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $null
    }

    Write-Step "Detected existing Peregrine at $existingPath"
    Write-WarningStep "Multiple managed Peregrine installs can be ambiguous because PATH order decides which one runs."

    return [PSCustomObject]@{
        Path = $existingPath
    }
}

function Maybe-HandleConflictingInstall {
    param(
        [object]$Conflict
    )

    if ($null -eq $Conflict) {
        return
    }

    if (Prompt-YesNo "Uninstall the existing Peregrine now?") {
        Write-Step "Removing: $($Conflict.Path)"
        try {
            Remove-Item -LiteralPath $Conflict.Path -Force
        } catch {
            Write-WarningStep "Failed to uninstall the existing Peregrine. Continuing with the standalone install."
        }
    } else {
        Write-WarningStep "Leaving the existing Peregrine installed. PATH order will determine which peregrine runs."
    }
}

function Test-VisiblePeregrineCommand {
    param(
        [string]$VisibleBinDir
    )

    $peregrineCommand = Join-Path $VisibleBinDir "peregrine.exe"
    & $peregrineCommand --version *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "Installed Peregrine command failed verification: $peregrineCommand --version"
    }
}

if ($env:OS -ne "Windows_NT") {
    Write-Error "install.ps1 supports Windows only. Use install.sh on macOS or Linux."
    exit 1
}

if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Error "Peregrine requires a 64-bit version of Windows."
    exit 1
}

$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
$target = $null
$platformLabel = $null
switch ($architecture) {
    "Arm64" {
        # We don't currently build Windows ARM64 in release.yml. Using x86_64 fallback.
        $target = "windows-x86_64"
        $platformLabel = "Windows (ARM64 fallback to x64)"
    }
    "X64" {
        $target = "windows-x86_64"
        $platformLabel = "Windows (x64)"
    }
    default {
        Write-Error "Unsupported architecture: $architecture"
        exit 1
    }
}

$peregrineHome = if ([string]::IsNullOrWhiteSpace($env:PEREGRINE_HOME)) {
    Join-Path $env:USERPROFILE ".peregrine"
} else {
    $env:PEREGRINE_HOME
}
$standaloneRoot = Join-Path $peregrineHome "standalone"
$releasesDir = Join-Path $standaloneRoot "releases"
$currentDir = Join-Path $standaloneRoot "current"
$lockPath = Join-Path $standaloneRoot "install.lock"

$defaultVisibleBinDir = Join-Path $env:LOCALAPPDATA "Programs\Peregrine\bin"
if ([string]::IsNullOrWhiteSpace($env:PEREGRINE_INSTALL_DIR)) {
    $visibleBinDir = $defaultVisibleBinDir
} else {
    $visibleBinDir = $env:PEREGRINE_INSTALL_DIR
}

$currentVersion = Get-CurrentInstalledVersion -StandaloneCurrentDir $currentDir
$resolvedVersion = Resolve-Version
$releaseName = "$resolvedVersion-$target"
$releaseDir = Join-Path $releasesDir $releaseName

if (-not [string]::IsNullOrWhiteSpace($currentVersion) -and $currentVersion -ne $resolvedVersion) {
    Write-Step "Updating Peregrine CLI from $currentVersion to $resolvedVersion"
} elseif (-not [string]::IsNullOrWhiteSpace($currentVersion)) {
    Write-Step "Updating Peregrine CLI"
} else {
    Write-Step "Installing Peregrine CLI"
}
Write-Step "Detected platform: $platformLabel"
Write-Step "Resolved version: $resolvedVersion"

$conflictingInstall = Get-ConflictingInstall -VisibleBinDir $visibleBinDir

$packageAsset = "peregrine-tui-$target.zip"
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("peregrine-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

try {
    Invoke-WithInstallLock -LockPath $lockPath -Script {
        Remove-StaleInstallArtifacts -ReleasesDir $releasesDir

        if (-not (Test-ReleaseIsComplete -ReleaseDir $releaseDir -ExpectedVersion $resolvedVersion -ExpectedTarget $target)) {
            if (Test-Path -LiteralPath $releaseDir) {
                Write-WarningStep "Found incomplete existing release at $releaseDir. Reinstalling."
            }

            $archivePath = Join-Path $tempDir $packageAsset
            $stagingDir = Join-Path $releasesDir ".staging.$releaseName.$PID"

            Write-Step "Downloading Peregrine CLI"
            $expectedDigest = Get-ReleaseAssetDigest -AssetName $packageAsset -ResolvedVersion $resolvedVersion
            $downloadUrl = Get-ReleaseUrl -AssetName $packageAsset -ResolvedVersion $resolvedVersion
            
            Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath
            Test-ArchiveDigest -ArchivePath $archivePath -ExpectedDigest $expectedDigest

            New-Item -ItemType Directory -Force -Path $releasesDir | Out-Null
            if (Test-Path -LiteralPath $stagingDir) {
                Remove-Item -LiteralPath $stagingDir -Recurse -Force
            }
            New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null
            
            Expand-Archive -Path $archivePath -DestinationPath $stagingDir -Force

            if (Test-Path -LiteralPath $releaseDir) {
                Remove-Item -LiteralPath $releaseDir -Recurse -Force
            }
            Move-Item -LiteralPath $stagingDir -Destination $releaseDir
        }

        New-Item -ItemType Directory -Force -Path $standaloneRoot | Out-Null
        Ensure-Junction -LinkPath $currentDir -TargetPath $releaseDir -InstallerOwnedTargetPrefix $releasesDir

        $visibleParent = Split-Path -Parent $visibleBinDir
        New-Item -ItemType Directory -Force -Path $visibleParent | Out-Null
        
        # In Windows we link to the directory containing peregrine.exe. Wait, no.
        # Actually, in Peregrine we need `peregrine-tui.exe` to be callable as `peregrine.exe` 
        # But Junctions only link directories.
        # To make it callable as peregrine, we could create a script wrapper, or just symlink the dir
        # and tell them to use peregrine-tui.
        # Wait, if we use a directory junction, $visibleBinDir links to $currentDir. 
        # Then `peregrine-tui.exe` is in $visibleBinDir. If the user expects `peregrine.exe`, 
        # we can copy or HardLink the exe?
        # Actually, in `install.sh` we symlink `~/.local/bin/peregrine` directly to the `peregrine-tui` binary.
        # In Windows, we can create a simple `peregrine.cmd` wrapper or rename it inside `currentDir`? No, because it needs to find sidecars alongside `peregrine-tui.exe`. 
        # If we create `peregrine.cmd` in $visibleBinDir:
        $cmdScriptPath = Join-Path $visibleBinDir "peregrine.cmd"
        $cmdScriptContent = "@ECHO OFF`n`"%~dp0peregrine-tui.exe`" %*"
        
        Ensure-Junction -LinkPath $visibleBinDir -TargetPath $currentDir -InstallerOwnedTargetPrefix $standaloneRoot
        Set-Content -Path $cmdScriptPath -Value $cmdScriptContent
        
        Test-VisiblePeregrineCommand -VisibleBinDir $visibleBinDir
    }
} finally {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
}

Maybe-HandleConflictingInstall -Conflict $conflictingInstall

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$prioritizeVisibleBin = $null -ne $conflictingInstall
if ($prioritizeVisibleBin) {
    $newUserPath = Prepend-PathEntry -PathValue $userPath -Entry $visibleBinDir
    if ($newUserPath -cne $userPath) {
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
        Write-Step "PATH updated for future PowerShell sessions."
    } else {
        Write-Step "$visibleBinDir is already first on PATH."
    }
} elseif (-not (Path-Contains -PathValue $userPath -Entry $visibleBinDir)) {
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        $newUserPath = $visibleBinDir
    } else {
        $newUserPath = "$visibleBinDir;$userPath"
    }

    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
    Write-Step "PATH updated for future PowerShell sessions."
} elseif (Path-Contains -PathValue $env:Path -Entry $visibleBinDir) {
    Write-Step "$visibleBinDir is already on PATH."
} else {
    Write-Step "PATH is already configured for future PowerShell sessions."
}

if ($prioritizeVisibleBin) {
    $env:Path = Prepend-PathEntry -PathValue $env:Path -Entry $visibleBinDir
} elseif (-not (Path-Contains -PathValue $env:Path -Entry $visibleBinDir)) {
    if ([string]::IsNullOrWhiteSpace($env:Path)) {
        $env:Path = $visibleBinDir
    } else {
        $env:Path = "$visibleBinDir;$env:Path"
    }
}

Write-Step "Current PowerShell session: peregrine"
Write-Step "Future PowerShell windows: open a new PowerShell window and run: peregrine"
Write-Host "Peregrine CLI $resolvedVersion installed successfully."

$peregrineCommand = Join-Path $visibleBinDir "peregrine.cmd"
if (Prompt-YesNo "Start Peregrine now?") {
    Write-Step "Launching Peregrine"
    & $peregrineCommand
}
