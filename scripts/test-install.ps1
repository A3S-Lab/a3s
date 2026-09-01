[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repoRoot = Split-Path -Parent $PSScriptRoot
$installer = Join-Path $repoRoot 'install.ps1'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("a3s-installer-test-$([Guid]::NewGuid().ToString('D'))")
$fixtureRoot = Join-Path $testRoot 'fixture'

function Fail-Test {
    param([string]$Message)
    throw "installer test failed: $Message"
}

function Assert-File {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Fail-Test "expected file $Path"
    }
}

function Assert-NoGeneratedPaths {
    param([string]$Root)
    $leftovers = @(Get-ChildItem -LiteralPath $Root -Recurse -Force |
        Where-Object { $_.Name -match '^\.a3s(?:-webview)?\.(new|backup|failed)\.' })
    if ($leftovers.Count -ne 0) {
        Fail-Test "installer left temporary path $($leftovers[0].FullName)"
    }
}

function Expect-Failure {
    param(
        [string]$Description,
        [scriptblock]$Action
    )
    try {
        & $Action
    } catch {
        return
    }
    Fail-Test "$Description unexpectedly succeeded"
}

$global:A3sInstallerMockRelease = $null
$global:A3sInstallerMockReleaseList = $null
$global:A3sInstallerMockLegacyRelease = $null
$global:A3sInstallerMockLegacyReleaseList = $null
$global:A3sInstallerMockPrimaryNotFound = $false
$global:A3sInstallerMockArchive = ''
$global:A3sInstallerMoveFault = ''
$global:A3sInstallerMoveFaultVersion = ''
$global:A3sInstallerMoveFaultTriggered = $false
$global:A3sInstallerRestFailures = 0
$global:A3sInstallerWebFailures = 0
$global:A3sInstallerRetryDelays = 0

function Invoke-RestMethod {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]$Uri,
        [Parameter(Mandatory = $true)]$Headers,
        [int]$TimeoutSec
    )
    if ($global:A3sInstallerRestFailures -gt 0) {
        $global:A3sInstallerRestFailures--
        throw 'injected transient release lookup failure'
    }
    if ($null -eq $global:A3sInstallerMockRelease -and
        $null -eq $global:A3sInstallerMockLegacyRelease) {
        throw 'mock release was not configured'
    }
    $isLegacyRepository = [string]$Uri -like '*repos/A3S-Lab/a3s/*'
    if (-not $isLegacyRepository -and
        [string]$Uri -notlike '*releases?per_page=100' -and
        $global:A3sInstallerMockPrimaryNotFound) {
        throw 'injected missing canonical release'
    }
    if ($isLegacyRepository -and
        [string]$Uri -like '*releases?per_page=100' -and
        $null -ne $global:A3sInstallerMockLegacyReleaseList) {
        return $global:A3sInstallerMockLegacyReleaseList
    }
    if ($isLegacyRepository -and $null -ne $global:A3sInstallerMockLegacyRelease) {
        return $global:A3sInstallerMockLegacyRelease
    }
    if ([string]$Uri -like '*releases?per_page=100' -and
        $null -ne $global:A3sInstallerMockReleaseList) {
        return $global:A3sInstallerMockReleaseList
    }
    return $global:A3sInstallerMockRelease
}

function Invoke-WebRequest {
    [CmdletBinding()]
    param(
        [switch]$UseBasicParsing,
        [Parameter(Mandatory = $true)]$Uri,
        [Parameter(Mandatory = $true)][string]$OutFile,
        [Parameter(Mandatory = $true)]$Headers,
        [int]$TimeoutSec
    )
    if ($global:A3sInstallerWebFailures -gt 0) {
        $global:A3sInstallerWebFailures--
        throw 'injected transient asset download failure'
    }
    Copy-Item -LiteralPath $global:A3sInstallerMockArchive -Destination $OutFile
}

function Start-Sleep {
    [CmdletBinding()]
    param([double]$Seconds)
    $global:A3sInstallerRetryDelays++
}

function Move-Item {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$LiteralPath,
        [Parameter(Mandatory = $true)][string]$Destination,
        [switch]$Force
    )

    Microsoft.PowerShell.Management\Move-Item @PSBoundParameters
    if ([string]::IsNullOrEmpty($global:A3sInstallerMoveFault) -or
        $global:A3sInstallerMoveFaultTriggered) {
        return
    }

    $sourceLeaf = Split-Path -Leaf $LiteralPath
    $destinationLeaf = Split-Path -Leaf $Destination
    $inject = switch ($global:A3sInstallerMoveFault) {
        'binary-activate' {
            $sourceLeaf -match '^\.a3s\.new\.[0-9a-f-]+\.exe$' -and
                $destinationLeaf -ceq 'a3s.exe'
            break
        }
        'webview-activate' {
            $sourceLeaf -match '^\.a3s-webview\.new\.[0-9a-f-]+\.exe$' -and
                $destinationLeaf -ceq 'a3s-webview.exe'
            break
        }
        default { $false }
    }
    if ($inject) {
        $global:A3sInstallerMoveFaultTriggered = $true
        throw "injected interruption after $($global:A3sInstallerMoveFault) mutation"
    }
}

function New-FixtureExecutable {
    param(
        [string]$Version,
        [string]$Destination,
        [ValidateSet('a3s', 'webview')][string]$Product = 'a3s'
    )
    $typeName = 'Program_' + $Version.Replace('.', '_') + '_' + [Guid]::NewGuid().ToString('N')
    $source = if ($Product -eq 'webview') {
        @"
using System;
public static class $typeName
{
    public static int Main(string[] args)
    {
        if (args.Length > 0 && args[0] == "--agent-island")
        {
            Console.Error.WriteLine("usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>");
            return 2;
        }
        Console.WriteLine("a3s-webview $Version");
        return 0;
    }
}
"@
    } else {
        @"
using System;
public static class $typeName
{
    public static int Main(string[] args)
    {
        Console.WriteLine("a3s $Version");
        return 0;
    }
}
"@
    }
    $compilerCandidates = @(
        (Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'),
        (Join-Path $env:WINDIR 'Microsoft.NET\Framework\v4.0.30319\csc.exe')
    )
    $compiler = $compilerCandidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $compiler) {
        throw 'could not find the Windows C# compiler required by this test'
    }

    $sourcePath = Join-Path $fixtureRoot "$typeName.cs"
    try {
        Set-Content -LiteralPath $sourcePath -Value $source -Encoding UTF8
        & $compiler /nologo /target:exe "/out:$Destination" $sourcePath
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $Destination -PathType Leaf)) {
            throw "fixture compilation failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Remove-Item -LiteralPath $sourcePath -Force -ErrorAction SilentlyContinue
    }
}

function Set-ReleaseFixture {
    param(
        [string]$Version,
        [switch]$UnsafeMember,
        [switch]$WithoutWebview,
        [switch]$WithLegacyPayload,
        [string]$Repository = 'A3S-Lab/CLI'
    )

    $payload = Join-Path $fixtureRoot 'payload'
    if (Test-Path -LiteralPath $payload) {
        Remove-Item -LiteralPath $payload -Recurse -Force
    }
    [IO.Directory]::CreateDirectory($payload) | Out-Null
    New-FixtureExecutable -Version $Version -Destination (Join-Path $payload 'a3s.exe')
    if (-not $WithoutWebview) {
        New-FixtureExecutable -Version $Version -Destination (Join-Path $payload 'a3s-webview.exe') -Product webview
    }
    if ($WithLegacyPayload) {
        [IO.Directory]::CreateDirectory((Join-Path $payload 'support')) | Out-Null
        [IO.Directory]::CreateDirectory((Join-Path $payload 'release-compat')) | Out-Null
        Set-Content -LiteralPath (Join-Path $payload 'support/legacy-runtime.txt') `
            -Value 'legacy runtime payload' -Encoding UTF8
        Set-Content -LiteralPath (Join-Path $payload 'release-compat/README.md') `
            -Value 'legacy release marker' -Encoding UTF8
    }
    if ($UnsafeMember) {
        Set-Content -LiteralPath (Join-Path $payload 'escape.txt') -Value 'unexpected' -Encoding UTF8
    }

    $assetName = "a3s-v$Version-x86_64-pc-windows-msvc.zip"
    $archive = Join-Path $fixtureRoot $assetName
    if (Test-Path -LiteralPath $archive) {
        Remove-Item -LiteralPath $archive -Force
    }
    Compress-Archive -Path (Join-Path $payload '*') -DestinationPath $archive
    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $asset = [pscustomobject]@{
        name = $assetName
        state = 'uploaded'
        digest = "sha256:$digest"
        browser_download_url = "https://github.com/$Repository/releases/download/v$Version/$assetName"
    }
    $global:A3sInstallerMockRelease = [pscustomobject]@{
        tag_name = "v$Version"
        draft = $false
        prerelease = $false
        assets = @($asset)
    }
    $global:A3sInstallerMockArchive = $archive
}

function Invoke-TestInstall {
    param(
        [string]$Version,
        [string]$InstallDir
    )
    & $installer -Version $Version -InstallDir $InstallDir
}

$savedEnvironment = @{
    A3S_MODIFY_PATH = $env:A3S_MODIFY_PATH
    A3S_GITHUB_TOKEN = $env:A3S_GITHUB_TOKEN
    LOCALAPPDATA = $env:LOCALAPPDATA
    PROCESSOR_ARCHITECTURE = $env:PROCESSOR_ARCHITECTURE
    PROCESSOR_ARCHITEW6432 = $env:PROCESSOR_ARCHITEW6432
}

[IO.Directory]::CreateDirectory($fixtureRoot) | Out-Null

try {
    $env:A3S_MODIFY_PATH = $null
    $env:A3S_GITHUB_TOKEN = $null
    $env:PROCESSOR_ARCHITECTURE = 'AMD64'
    $env:PROCESSOR_ARCHITEW6432 = $null
    $env:LOCALAPPDATA = Join-Path $testRoot 'Local AppData 用户'

    # Transient API and archive transport failures are retried without leaving
    # a partial download or requiring the user to restart the installer.
    $retryRoot = Join-Path $testRoot 'transient-network-retry'
    Set-ReleaseFixture -Version '1.2.1' -WithoutWebview
    $global:A3sInstallerRestFailures = 1
    $global:A3sInstallerWebFailures = 1
    $global:A3sInstallerRetryDelays = 0
    Invoke-TestInstall -Version '1.2.1' -InstallDir (Join-Path $retryRoot 'bin')
    Assert-File (Join-Path $retryRoot 'bin\a3s.exe')
    if ($global:A3sInstallerRestFailures -ne 0 -or
        $global:A3sInstallerWebFailures -ne 0 -or
        $global:A3sInstallerRetryDelays -ne 2) {
        Fail-Test 'transient network failures were not retried exactly once each'
    }
    Assert-NoGeneratedPaths -Root $retryRoot

    # Stable archives published before the companion bundle remain installable;
    # Code owns their verified WebView first-use setup.
    $legacyRoot = Join-Path $testRoot 'legacy-without-webview'
    Set-ReleaseFixture -Version '1.2.2' -WithoutWebview
    Invoke-TestInstall -Version '1.2.2' -InstallDir (Join-Path $legacyRoot 'bin')
    Assert-File (Join-Path $legacyRoot 'bin\a3s.exe')
    if (Test-Path -LiteralPath (Join-Path $legacyRoot 'bin\a3s-webview.exe')) {
        Fail-Test 'legacy release unexpectedly installed a WebView companion'
    }
    Assert-NoGeneratedPaths -Root $legacyRoot

    # Historical CLI archives may contain runtime payloads that are no longer
    # supported. The installer validates their paths, extracts only binaries,
    # and never copies the legacy payload into the installation directory.
    $legacyPayloadRoot = Join-Path $testRoot 'legacy-runtime-payload'
    Set-ReleaseFixture -Version '1.2.9' -WithLegacyPayload
    Invoke-TestInstall -Version '1.2.9' -InstallDir (Join-Path $legacyPayloadRoot 'bin')
    Assert-File (Join-Path $legacyPayloadRoot 'bin\a3s.exe')
    Assert-File (Join-Path $legacyPayloadRoot 'bin\a3s-webview.exe')
    if (Test-Path -LiteralPath (Join-Path $legacyPayloadRoot 'bin\support')) {
        Fail-Test 'legacy runtime payload was installed'
    }
    if (Test-Path -LiteralPath (Join-Path $legacyPayloadRoot 'bin\release-compat')) {
        Fail-Test 'legacy release marker was installed'
    }
    Assert-NoGeneratedPaths -Root $legacyPayloadRoot

    # `latest` ignores other product tags and prereleases in the monorepo.
    Set-ReleaseFixture -Version '1.2.2' -WithoutWebview
    $global:A3sInstallerMockReleaseList = @(
        [pscustomobject]@{
            tag_name = 'a3s-code-v9.0.0'
            draft = $false
            prerelease = $false
        },
        [pscustomobject]@{
            tag_name = 'v9.0.0'
            draft = $false
            prerelease = $true
        },
        $global:A3sInstallerMockRelease
    )
    $latestRoot = Join-Path $testRoot 'latest-stable-cli'
    Invoke-TestInstall -Version 'latest' -InstallDir (Join-Path $latestRoot 'bin')
    Assert-File (Join-Path $latestRoot 'bin\a3s.exe')
    Assert-NoGeneratedPaths -Root $latestRoot
    $global:A3sInstallerMockReleaseList = $null

    # During the release transition, latest selects the higher stable version
    # from the canonical CLI repository and the former monorepo source.
    Set-ReleaseFixture -Version '1.3.0' -WithoutWebview `
        -Repository 'A3S-Lab/a3s'
    $global:A3sInstallerMockLegacyRelease = $global:A3sInstallerMockRelease
    $global:A3sInstallerMockLegacyReleaseList = @($global:A3sInstallerMockLegacyRelease)
    $global:A3sInstallerMockReleaseList = @(
        [pscustomobject]@{
            tag_name = 'v1.2.2'
            draft = $false
            prerelease = $false
        }
    )
    $transitionRoot = Join-Path $testRoot 'release-transition'
    Invoke-TestInstall -Version 'latest' -InstallDir (Join-Path $transitionRoot 'bin')
    $transitionVersion = (& (Join-Path $transitionRoot 'bin\a3s.exe') --version | Out-String).Trim()
    if ($transitionVersion -cne 'a3s 1.3.0') {
        Fail-Test 'latest did not select the newer former-monorepo stable release'
    }
    Assert-NoGeneratedPaths -Root $transitionRoot

    $global:A3sInstallerMockPrimaryNotFound = $true
    $explicitTransitionRoot = Join-Path $testRoot 'explicit-release-transition'
    Invoke-TestInstall -Version '1.3.0' -InstallDir (Join-Path $explicitTransitionRoot 'bin')
    $explicitTransitionVersion = (& (Join-Path $explicitTransitionRoot 'bin\a3s.exe') --version | Out-String).Trim()
    if ($explicitTransitionVersion -cne 'a3s 1.3.0') {
        Fail-Test 'an explicit version did not fall back to the former-monorepo release'
    }
    Assert-NoGeneratedPaths -Root $explicitTransitionRoot
    $global:A3sInstallerMockPrimaryNotFound = $false
    $global:A3sInstallerMockReleaseList = $null
    $global:A3sInstallerMockLegacyRelease = $null
    $global:A3sInstallerMockLegacyReleaseList = $null

    # Initial installation and upgrade replace the binary and companion payloads.
    $upgradeRoot = Join-Path $testRoot 'upgrade path 用户'
    $installDir = Join-Path $upgradeRoot 'bin'
    Set-ReleaseFixture -Version '1.2.3'
    Invoke-TestInstall -Version '1.2.3' -InstallDir $installDir
    Assert-File (Join-Path $installDir 'a3s.exe')
    Assert-File (Join-Path $installDir 'a3s-webview.exe')
    $installedVersion = (& (Join-Path $installDir 'a3s.exe') --version | Out-String).Trim()
    if ($installedVersion -cne 'a3s 1.2.3') {
        Fail-Test "initial binary reported $installedVersion"
    }
    $installedWebviewVersion = (& (Join-Path $installDir 'a3s-webview.exe') | Out-String).Trim()
    if ($installedWebviewVersion -cne 'a3s-webview 1.2.3') {
        Fail-Test "initial WebView companion reported $installedWebviewVersion"
    }

    Set-ReleaseFixture -Version '1.2.4'
    Invoke-TestInstall -Version '1.2.4' -InstallDir $installDir
    $installedVersion = (& (Join-Path $installDir 'a3s.exe') --version | Out-String).Trim()
    if ($installedVersion -cne 'a3s 1.2.4') {
        Fail-Test "upgraded binary reported $installedVersion"
    }
    $installedWebviewVersion = (& (Join-Path $installDir 'a3s-webview.exe') | Out-String).Trim()
    if ($installedWebviewVersion -cne 'a3s-webview 1.2.4') {
        Fail-Test "upgraded WebView companion reported $installedWebviewVersion"
    }
    Assert-NoGeneratedPaths -Root $upgradeRoot

    # Digest errors fail before activation and preserve the old installation.
    Set-ReleaseFixture -Version '1.2.5'
    $global:A3sInstallerMockRelease.assets[0].digest = 'sha256:' + ('f' * 64)
    Expect-Failure 'digest mismatch' {
        Invoke-TestInstall -Version '1.2.5' -InstallDir $installDir
    }
    $installedVersion = (& (Join-Path $installDir 'a3s.exe') --version | Out-String).Trim()
    if ($installedVersion -cne 'a3s 1.2.4') {
        Fail-Test 'digest failure changed the installed binary'
    }
    $installedWebviewVersion = (& (Join-Path $installDir 'a3s-webview.exe') | Out-String).Trim()
    if ($installedWebviewVersion -cne 'a3s-webview 1.2.4') {
        Fail-Test 'digest failure changed the installed WebView companion'
    }

    # Missing digest metadata fails closed.
    Set-ReleaseFixture -Version '1.2.6'
    $global:A3sInstallerMockRelease.assets[0].PSObject.Properties.Remove('digest')
    Expect-Failure 'missing digest' {
        Invoke-TestInstall -Version '1.2.6' -InstallDir $installDir
    }

    # Unexpected archive members are rejected before activation.
    Set-ReleaseFixture -Version '1.2.7' -UnsafeMember
    Expect-Failure 'unsafe archive member' {
        Invoke-TestInstall -Version '1.2.7' -InstallDir $installDir
    }
    $installedVersion = (& (Join-Path $installDir 'a3s.exe') --version | Out-String).Trim()
    if ($installedVersion -cne 'a3s 1.2.4') {
        Fail-Test 'unsafe archive changed the installed binary'
    }
    $installedWebviewVersion = (& (Join-Path $installDir 'a3s-webview.exe') | Out-String).Trim()
    if ($installedWebviewVersion -cne 'a3s-webview 1.2.4') {
        Fail-Test 'unsafe archive changed the installed WebView companion'
    }

    # A locked executable forces rollback without losing the old installation.
    $lockedRoot = Join-Path $testRoot 'locked'
    $lockedInstallDir = Join-Path $lockedRoot 'bin'
    Set-ReleaseFixture -Version '2.0.0'
    Invoke-TestInstall -Version '2.0.0' -InstallDir $lockedInstallDir
    Set-ReleaseFixture -Version '2.0.1'
    $lockedBinary = Join-Path $lockedInstallDir 'a3s.exe'
    $lock = [IO.File]::Open($lockedBinary, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        Expect-Failure 'locked executable upgrade' {
            Invoke-TestInstall -Version '2.0.1' -InstallDir $lockedInstallDir
        }
    } finally {
        $lock.Dispose()
    }
    $installedVersion = (& $lockedBinary --version | Out-String).Trim()
    if ($installedVersion -cne 'a3s 2.0.0') {
        Fail-Test 'locked upgrade did not preserve the old binary'
    }
    $installedWebviewVersion = (& (Join-Path $lockedInstallDir 'a3s-webview.exe') | Out-String).Trim()
    if ($installedWebviewVersion -cne 'a3s-webview 2.0.0') {
        Fail-Test 'locked upgrade did not restore the old WebView companion'
    }
    Assert-NoGeneratedPaths -Root $lockedRoot

    # Faults raised after successful filesystem mutations but before the next
    # state assignment must restore the state visible before the installation.
    $faultRoot = Join-Path $testRoot 'fault-injection'
    $faultInstallDir = Join-Path $faultRoot 'bin'
    Set-ReleaseFixture -Version '4.0.0'
    Invoke-TestInstall -Version '4.0.0' -InstallDir $faultInstallDir
    $initialWebviewFaultRoot = Join-Path $testRoot 'initial-webview-fault'
    $global:A3sInstallerMoveFault = 'webview-activate'
    $global:A3sInstallerMoveFaultVersion = '4.0.1'
    $global:A3sInstallerMoveFaultTriggered = $false
    Set-ReleaseFixture -Version '4.0.1'
    Expect-Failure 'interruption after initial WebView companion activation' {
        Invoke-TestInstall -Version '4.0.1' -InstallDir (Join-Path $initialWebviewFaultRoot 'bin')
    }
    if (-not $global:A3sInstallerMoveFaultTriggered) {
        Fail-Test 'WebView companion fault was not injected'
    }
    if (Test-Path -LiteralPath (Join-Path $initialWebviewFaultRoot 'bin\a3s-webview.exe')) {
        Fail-Test 'WebView activation interruption left the new companion active'
    }
    if (Test-Path -LiteralPath (Join-Path $initialWebviewFaultRoot 'bin\a3s.exe')) {
        Fail-Test 'WebView activation interruption left the new binary active'
    }
    Assert-NoGeneratedPaths -Root $initialWebviewFaultRoot

    $initialFaultRoot = Join-Path $testRoot 'initial-binary-fault'
    $global:A3sInstallerMoveFault = 'binary-activate'
    $global:A3sInstallerMoveFaultVersion = '4.1.0'
    $global:A3sInstallerMoveFaultTriggered = $false
    Set-ReleaseFixture -Version '4.1.0'
    Expect-Failure 'interruption after initial binary activation' {
        Invoke-TestInstall -Version '4.1.0' -InstallDir (Join-Path $initialFaultRoot 'bin')
    }
    if (-not $global:A3sInstallerMoveFaultTriggered) {
        Fail-Test 'binary activation fault was not injected'
    }
    if (Test-Path -LiteralPath (Join-Path $initialFaultRoot 'bin\a3s.exe')) {
        Fail-Test 'binary activation interruption left the new binary active'
    }
    if (Test-Path -LiteralPath (Join-Path $initialFaultRoot 'bin\a3s-webview.exe')) {
        Fail-Test 'binary activation interruption left the new WebView companion active'
    }
    Assert-NoGeneratedPaths -Root $initialFaultRoot
    $global:A3sInstallerMoveFault = ''
    $global:A3sInstallerMoveFaultVersion = ''

    # Relative install roots and unsupported Windows architectures fail closed.
    Set-ReleaseFixture -Version '3.0.0'
    Push-Location $testRoot
    try {
        Expect-Failure 'relative install directory' {
            & $installer -Version '3.0.0' -InstallDir 'relative-bin'
        }
    } finally {
        Pop-Location
    }
    if (Test-Path -LiteralPath (Join-Path $testRoot 'relative-bin')) {
        Fail-Test 'relative install directory was created'
    }

    $env:PROCESSOR_ARCHITECTURE = 'ARM64'
    $env:PROCESSOR_ARCHITEW6432 = $null
    Expect-Failure 'Windows ARM64' {
        Invoke-TestInstall -Version '3.0.0' -InstallDir (Join-Path $testRoot 'arm\bin')
    }

    Write-Host 'install.ps1 tests passed'
} finally {
    foreach ($name in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process')
    }
    Remove-Variable -Name A3sInstallerMockRelease -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMockReleaseList -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMockLegacyRelease -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMockLegacyReleaseList -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMockPrimaryNotFound -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMockArchive -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMoveFault -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMoveFaultVersion -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name A3sInstallerMoveFaultTriggered -Scope Global -ErrorAction SilentlyContinue
    $fullTestRoot = [IO.Path]::GetFullPath($testRoot)
    $fullTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if ($fullTestRoot.StartsWith($fullTempRoot, [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($fullTestRoot) -match '^a3s-installer-test-[0-9a-f-]+$') {
        Remove-Item -LiteralPath $fullTestRoot -Recurse -Force
    } else {
        Write-Warning "refusing to remove unexpected test directory $fullTestRoot"
    }
}
