[CmdletBinding()]
param(
    [ValidateSet("x64", "x86", "all")]
    [string]$Architecture = "all",

    [string]$ClientRoot = "C:\Program Files (x86)\3DMVS",

    [string]$DevelopmentRoot = $env:MV3DLP_DEV_ENV,

    [string]$LpRuntimeRoot = "C:\Program Files (x86)\Common Files\Mv3dLpSDK",

    [string]$Mv3dRuntimeRoot = "C:\Program Files (x86)\Common Files\MV3D",

    [ValidateRange(1, 600)]
    [int]$ProbeTimeoutSeconds = 30
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$baselinePath = Join-Path $repoRoot "m0\sdk-baseline.json"
$baseline = Get-Content -LiteralPath $baselinePath -Raw -Encoding UTF8 | ConvertFrom-Json

if ([string]::IsNullOrWhiteSpace($DevelopmentRoot)) {
    $DevelopmentRoot = Join-Path $ClientRoot "Development"
}

$rootMap = @{
    client = $ClientRoot
    development = $DevelopmentRoot
    lpsdk = $LpRuntimeRoot
    runtime_x86 = Join-Path $LpRuntimeRoot "Runtime\Win32_i86"
    runtime_x64 = Join-Path $LpRuntimeRoot "Runtime\Win64_x64"
    mv3d_runtime_x86 = Join-Path $Mv3dRuntimeRoot "Runtime\Win32_i86"
    mv3d_runtime_x64 = Join-Path $Mv3dRuntimeRoot "Runtime\Win64_x64"
}

function Assert-Equal {
    param(
        [Parameter(Mandatory = $true)]$Expected,
        [Parameter(Mandatory = $true)]$Actual,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if ("$Expected" -cne "$Actual") {
        throw "$Label mismatch: expected '$Expected', got '$Actual'"
    }
}

function Test-BaselineArtifacts {
    foreach ($artifact in $baseline.artifacts) {
        $root = $rootMap[$artifact.root]
        if ([string]::IsNullOrWhiteSpace($root)) {
            throw "Unknown root '$($artifact.root)' for artifact '$($artifact.name)'"
        }

        $path = Join-Path $root $artifact.path
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Missing baseline artifact '$($artifact.name)': $path"
        }

        $item = Get-Item -LiteralPath $path
        Assert-Equal $artifact.length $item.Length "$($artifact.name) length"

        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
        Assert-Equal $artifact.sha256 $hash "$($artifact.name) SHA-256"
    }

    $versionChecks = @(
        @{
            Label = "3DMVS client"
            Path = Join-Path $ClientRoot "Applications\Win64\3DMVS.exe"
            Expected = $baseline.versions.client_file_version
        },
        @{
            Label = "x86 LPSDK runtime"
            Path = Join-Path $rootMap.runtime_x86 "Mv3dLp.dll"
            Expected = $baseline.versions.runtime_file_version
        },
        @{
            Label = "x64 LPSDK runtime"
            Path = Join-Path $rootMap.runtime_x64 "Mv3dLp.dll"
            Expected = $baseline.versions.runtime_file_version
        },
        @{
            Label = "x86 MV3D runtime"
            Path = Join-Path $rootMap.mv3d_runtime_x86 "MvCameraControl3D.dll"
            Expected = $baseline.versions.mv3d_runtime
        },
        @{
            Label = "x64 MV3D runtime"
            Path = Join-Path $rootMap.mv3d_runtime_x64 "MvCameraControl3D.dll"
            Expected = $baseline.versions.mv3d_runtime
        }
    )
    foreach ($check in $versionChecks) {
        if (-not (Test-Path -LiteralPath $check.Path -PathType Leaf)) {
            throw "Missing versioned artifact '$($check.Label)': $($check.Path)"
        }
        $fileVersion = (Get-Item -LiteralPath $check.Path).VersionInfo.FileVersion
        Assert-Equal $check.Expected $fileVersion "$($check.Label) file version"
    }

    Write-Host "Artifact hashes and versions: PASS"
}

function Find-VisualStudio {
    $vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "vswhere.exe was not found; install Visual Studio with the C++ toolchain."
    }

    $installation = (& $vswhere -latest -products "*" -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
    if ([string]::IsNullOrWhiteSpace($installation)) {
        throw "No Visual Studio installation with a discoverable installation path was found."
    }

    return $installation.Trim()
}

function Get-Dumpbin {
    param([Parameter(Mandatory = $true)][string]$VisualStudioRoot)

    $msvcRoot = Join-Path $VisualStudioRoot "VC\Tools\MSVC"
    $toolset = Get-ChildItem -LiteralPath $msvcRoot -Directory |
        Sort-Object Name -Descending |
        Select-Object -First 1
    if ($null -eq $toolset) {
        throw "No MSVC toolset was found below $msvcRoot"
    }

    $dumpbin = Join-Path $toolset.FullName "bin\Hostx64\x64\dumpbin.exe"
    if (-not (Test-Path -LiteralPath $dumpbin -PathType Leaf)) {
        throw "dumpbin.exe was not found at $dumpbin"
    }

    return $dumpbin
}

function Get-PublicHeaderFunctions {
    $headers = @(
        (Join-Path $DevelopmentRoot "Includes\Mv3dLpApi.h"),
        (Join-Path $DevelopmentRoot "Includes\Mv3dLpImgProc.h")
    )
    $source = ($headers | ForEach-Object {
        Get-Content -LiteralPath $_ -Raw -Encoding UTF8
    }) -join [Environment]::NewLine

    return [regex]::Matches($source, "\b(MV3D_LP_[A-Za-z0-9_]+)\s*\(") |
        ForEach-Object { $_.Groups[1].Value } |
        Sort-Object -Unique
}

function Test-ApiContracts {
    param([Parameter(Mandatory = $true)][string[]]$HeaderFunctions)

    $contractsRelativePath = $baseline.contracts.api
    if ([string]::IsNullOrWhiteSpace($contractsRelativePath)) {
        throw "No API contract path is configured in the baseline manifest."
    }
    $contractsPath = Join-Path (Split-Path $baselinePath -Parent) $contractsRelativePath
    $contracts = Get-Content -LiteralPath $contractsPath -Raw -Encoding UTF8 |
        ConvertFrom-Json
    $contractNames = @($contracts.apis | ForEach-Object { $_.name })
    $uniqueContractNames = @($contractNames | Sort-Object -Unique)

    Assert-Equal $contracts.source_header_function_count $HeaderFunctions.Count "contract source function count"
    Assert-Equal $HeaderFunctions.Count $contractNames.Count "API contract entry count"
    Assert-Equal $contractNames.Count $uniqueContractNames.Count "unique API contract entry count"

    $missing = @($HeaderFunctions | Where-Object { $_ -notin $uniqueContractNames })
    $extra = @($uniqueContractNames | Where-Object { $_ -notin $HeaderFunctions })
    if ($missing.Count -ne 0 -or $extra.Count -ne 0) {
        throw "API contract coverage mismatch. Missing: $($missing -join ', '); extra: $($extra -join ', ')"
    }

    $validSurfaceValues = @(
        $contracts.safe_surface_values.PSObject.Properties |
            ForEach-Object { $_.Name }
    )
    foreach ($api in $contracts.apis) {
        if ($api.safe_surface -notin $validSurfaceValues) {
            throw "Invalid safe_surface '$($api.safe_surface)' for $($api.name)"
        }
        if ([string]::IsNullOrWhiteSpace($api.signature)) {
            throw "Missing signature for $($api.name)"
        }
    }

    Write-Host "API contract coverage: PASS ($($contractNames.Count) public declarations)"
}

function Test-ExportSurface {
    param(
        [Parameter(Mandatory = $true)][string]$Dumpbin,
        [Parameter(Mandatory = $true)][string[]]$HeaderFunctions,
        [Parameter(Mandatory = $true)][string]$Arch
    )

    $runtimeRoot = if ($Arch -eq "x64") { $rootMap.runtime_x64 } else { $rootMap.runtime_x86 }
    $dll = Join-Path $runtimeRoot "Mv3dLp.dll"
    $dump = (& $Dumpbin /exports $dll | Out-String)
    if ($LASTEXITCODE -ne 0) {
        throw "dumpbin failed for $dll"
    }

    $countMatch = [regex]::Match($dump, "(?m)^\s*(\d+)\s+number of functions\s*$")
    if (-not $countMatch.Success) {
        throw "Could not parse the PE export count for $dll"
    }
    $peCount = [int]$countMatch.Groups[1].Value
    $expectedPeCount = if ($Arch -eq "x64") {
        $baseline.api_surface.pe_exports_x64
    } else {
        $baseline.api_surface.pe_exports_x86
    }
    Assert-Equal $expectedPeCount $peCount "$Arch PE export count"

    $sdkExports = [regex]::Matches($dump, "MV3D_LP_[A-Za-z0-9_]+") |
        ForEach-Object { $_.Value } |
        Sort-Object -Unique
    $expectedSdkCount = if ($Arch -eq "x64") {
        $baseline.api_surface.mv3d_lp_exports_x64
    } else {
        $baseline.api_surface.mv3d_lp_exports_x86
    }
    Assert-Equal $expectedSdkCount $sdkExports.Count "$Arch MV3D_LP export count"

    $missing = @($HeaderFunctions | Where-Object { $_ -notin $sdkExports })
    if ($missing.Count -ne 0) {
        throw "$Arch runtime is missing public header exports: $($missing -join ', ')"
    }

    Write-Host "$Arch export surface: PASS ($($HeaderFunctions.Count) public declarations, $($sdkExports.Count) MV3D_LP exports, $peCount PE exports)"
}

function Invoke-AbiProbe {
    param(
        [Parameter(Mandatory = $true)][string]$VisualStudioRoot,
        [Parameter(Mandatory = $true)][string]$Arch
    )

    $vcvars = Join-Path $VisualStudioRoot "VC\Auxiliary\Build\vcvarsall.bat"
    if (-not (Test-Path -LiteralPath $vcvars -PathType Leaf)) {
        throw "vcvarsall.bat was not found at $vcvars"
    }

    $outputDirectory = Join-Path $repoRoot "target\m0\$Arch"
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

    $source = Join-Path $repoRoot "tools\m0\abi_probe.cpp"
    $executable = Join-Path $outputDirectory "abi_probe.exe"
    $object = Join-Path $outputDirectory "abi_probe.obj"
    $include = Join-Path $DevelopmentRoot "Includes"
    $library = if ($Arch -eq "x64") {
        Join-Path $DevelopmentRoot "Libraries\win64"
    } else {
        Join-Path $DevelopmentRoot "Libraries\win32"
    }

    $compile = 'call "{0}" {1} >nul && cl.exe /nologo /utf-8 /std:c++17 /EHsc /W4 /WX /DWIN32 /Gd /I"{2}" "{3}" /Fe:"{4}" /Fo:"{5}" /link /LIBPATH:"{6}" Mv3dLp.lib' -f $vcvars, $Arch, $include, $source, $executable, $object, $library
    & $env:ComSpec /d /s /c $compile
    if ($LASTEXITCODE -ne 0) {
        throw "$Arch ABI probe compilation failed with exit code $LASTEXITCODE"
    }

    $runtime = if ($Arch -eq "x64") { $rootMap.runtime_x64 } else { $rootMap.runtime_x86 }
    $mv3dRuntime = if ($Arch -eq "x64") { $rootMap.mv3d_runtime_x64 } else { $rootMap.mv3d_runtime_x86 }
    $applicationRelative = if ($Arch -eq "x64") { "Applications\Win64" } else { "Applications\Win32" }
    $application = Join-Path $ClientRoot $applicationRelative

    $oldPath = $env:PATH
    $process = $null
    try {
        $env:PATH = "$runtime;$mv3dRuntime;$application;$oldPath"

        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = $executable
        $startInfo.WorkingDirectory = $outputDirectory
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true

        $process = New-Object System.Diagnostics.Process
        $process.StartInfo = $startInfo
        if (-not $process.Start()) {
            throw "Failed to start the $Arch ABI probe."
        }

        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($ProbeTimeoutSeconds * 1000)) {
            try {
                $process.Kill()
                $process.WaitForExit()
            } catch {
                # Preserve the timeout as the primary error.
            }
            throw "$Arch ABI probe exceeded the $ProbeTimeoutSeconds second timeout."
        }

        $actualText = $stdoutTask.Result
        $stderrText = $stderrTask.Result
        if ($process.ExitCode -ne 0) {
            throw "$Arch ABI probe failed with exit code $($process.ExitCode). stderr: $stderrText"
        }
    } finally {
        if ($null -ne $process) {
            $process.Dispose()
        }
        $env:PATH = $oldPath
    }

    $expectedRelativePath = $baseline.abi.$Arch
    if ([string]::IsNullOrWhiteSpace($expectedRelativePath)) {
        throw "No ABI baseline path is configured for $Arch"
    }
    $expectedPath = Join-Path (Split-Path $baselinePath -Parent) $expectedRelativePath
    $expected = Get-Content -LiteralPath $expectedPath -Raw -Encoding UTF8 |
        ConvertFrom-Json |
        ConvertTo-Json -Depth 20 -Compress
    $actualObject = $actualText | ConvertFrom-Json
    Assert-Equal $baseline.versions.runtime_api $actualObject.sdk_version "$Arch runtime API version"
    Assert-Equal $baseline.lifecycle_smoke.initialize_status $actualObject.lifecycle.initialize_status "$Arch Initialize status"
    Assert-Equal $baseline.lifecycle_smoke.get_device_number_status $actualObject.lifecycle.get_device_number_status "$Arch GetDeviceNumber status"
    Assert-Equal $baseline.lifecycle_smoke.finalize_status $actualObject.lifecycle.finalize_status "$Arch Finalize status"
    $actual = $actualObject | ConvertTo-Json -Depth 20 -Compress

    Assert-Equal $expected $actual "$Arch ABI probe"
    Write-Host "$Arch ABI and MV3D_LP_GetVersion: PASS"
}

Test-BaselineArtifacts

$visualStudio = Find-VisualStudio
$dumpbin = Get-Dumpbin $visualStudio
$headerFunctions = @(Get-PublicHeaderFunctions)
Assert-Equal $baseline.api_surface.public_header_functions $headerFunctions.Count "public header function count"
Test-ApiContracts $headerFunctions

$architectures = if ($Architecture -eq "all") { @("x64", "x86") } else { @($Architecture) }
foreach ($arch in $architectures) {
    Test-ExportSurface $dumpbin $headerFunctions $arch
    Invoke-AbiProbe $visualStudio $arch
}

Write-Host "M0 verification completed successfully."
