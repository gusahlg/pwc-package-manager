# Project Watt Cubed for Windows: download, build and play, in one command.
#
# Press Win+R, paste this line and press Enter:
#
#   powershell -ExecutionPolicy Bypass -NoProfile -Command "irm https://raw.githubusercontent.com/gusahlg/pwc-package-manager/main/scripts/play.ps1 | iex"
#
# It installs what is missing (Git, the Visual Studio C++ build tools and Rust; Windows asks you to allow
# the installers), downloads the game, the renderer and the package manager, builds the game with all
# first-party modules (the essentials, chat and commands, and the developer toolkit) and starts it. Afterwards the "Project Watt Cubed" shortcut on the desktop and in the
# Start menu updates everything and plays. Everything lives in %USERPROFILE%\ProjectWattCubed.
#
# Environment (optional): PWC_PLAY_DIR (install folder), PWC_PLAY_NO_RUN=1 (set up, don't start).

function Invoke-PwcPlay {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    $repos = @('project_watt_cubed', 'voxel-engine', 'pwc-package-manager')
    $github = 'https://github.com/gusahlg'

    function Say($text) { Write-Host "`n==> $text" -ForegroundColor Green }
    function Warn($text) { Write-Host "!!  $text" -ForegroundColor Yellow }
    function Fail($text) { throw $text }
    # Run a program, show its output, and stop with a clear message if it fails.
    function Run($what, $exe, [string[]]$arguments) {
        $saved = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        & $exe @arguments
        $code = $LASTEXITCODE
        $ErrorActionPreference = $saved
        if ($code -ne 0) { Fail "$what failed (exit code $code)." }
    }
    function Refresh-Path {
        $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
        $user = [Environment]::GetEnvironmentVariable('Path', 'User')
        $env:Path = "$env:USERPROFILE\.cargo\bin;$machine;$user"
    }
    function Have($command) { [bool](Get-Command $command -ErrorAction SilentlyContinue) }
    function Winget-Install($id, [string[]]$extra) {
        if (-not (Have 'winget')) {
            Fail "winget is missing. Install 'App Installer' from the Microsoft Store, then run this again."
        }
        $arguments = @('install', '--id', $id, '-e', '--accept-source-agreements', '--accept-package-agreements') + $extra
        $saved = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        & winget @arguments
        $code = $LASTEXITCODE
        $ErrorActionPreference = $saved
        # -1978335189 means "already installed, no newer version".
        if ($code -ne 0 -and $code -ne -1978335189) { Fail "Installing $id failed (exit code $code)." }
        Refresh-Path
    }

    if (-not [Environment]::Is64BitOperatingSystem) { Fail 'Project Watt Cubed needs 64-bit Windows.' }
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($arch -eq 'ARM64') { $hostTriple = 'aarch64-pc-windows-msvc' } else { $hostTriple = 'x86_64-pc-windows-msvc' }

    if ($env:PWC_PLAY_DIR) { $dir = $env:PWC_PLAY_DIR } else { $dir = Join-Path $env:USERPROFILE 'ProjectWattCubed' }
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $dir = (Resolve-Path $dir).Path
    Refresh-Path

    $drive = Get-PSDrive -Name ($dir.Substring(0, 1))
    if ($drive.Free -lt 15GB) { Warn ("Only {0:N0} GB free on {1}:; the first build needs about 15 GB." -f ($drive.Free / 1GB), $drive.Name) }

    if (-not (Have 'git')) {
        Say 'Installing Git'
        Winget-Install 'Git.Git' @('--silent')
        if (-not (Have 'git')) { Fail 'Git was installed but is not on PATH yet. Close this window and run the command again.' }
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $haveCpp = $false
    if (Test-Path $vswhere) {
        $found = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        $haveCpp = [bool]$found
    }
    if (-not $haveCpp) {
        Say 'Installing the Visual Studio C++ build tools (a large download; this can take 10-20 minutes)'
        Winget-Install 'Microsoft.VisualStudio.2022.BuildTools' @('--override',
            '--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended')
    }

    if (-not (Have 'rustup')) {
        Say 'Installing Rust'
        $installer = Join-Path $env:TEMP 'rustup-init.exe'
        Invoke-WebRequest "https://static.rust-lang.org/rustup/dist/$hostTriple/rustup-init.exe" -OutFile $installer
        Run 'Installing Rust' $installer @('-y', '--profile', 'minimal', '--default-host', $hostTriple)
        Refresh-Path
    }
    # Use the newest stable Rust for this game without changing anyone's default toolchain.
    $env:RUSTUP_TOOLCHAIN = 'stable'
    Run 'Updating Rust' 'rustup' @('toolchain', 'install', 'stable', '--profile', 'minimal')

    if (-not (Test-Path (Join-Path $env:SystemRoot 'System32\vulkan-1.dll'))) {
        Warn 'No Vulkan driver was found. Update your graphics driver (NVIDIA, AMD or Intel); the game needs Vulkan.'
    }

    Say 'Downloading the latest game, renderer and package manager'
    foreach ($name in $repos) {
        $path = Join-Path $dir $name
        if (Test-Path (Join-Path $path '.git')) {
            $saved = $ErrorActionPreference
            $ErrorActionPreference = 'Continue'
            & git -C $path pull --ff-only --quiet
            if ($LASTEXITCODE -ne 0) { Warn "Could not update $name (local changes?); keeping your copy." }
            $ErrorActionPreference = $saved
        } else {
            Run "Downloading $name" 'git' @('-c', 'core.longpaths=true', 'clone', '--quiet', '--depth', '1', "$github/$name.git", $path)
        }
        Write-Host ("    {0,-22} {1}" -f $name, (& git -C $path log -1 --format='%h %s'))
    }

    # Keep memory use safe on small machines: roughly one compile job per 2 GB of RAM.
    $memGb = [int]((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
    $jobs = [Math]::Max(1, [Math]::Min([Environment]::ProcessorCount, [int]($memGb / 2)))
    $env:CARGO_BUILD_JOBS = "$jobs"

    Say 'Building the package manager (pwc)'
    $env:CARGO_TARGET_DIR = Join-Path $dir '.build\pwc-tools'
    Run 'Building pwc' 'cargo' @('install', '--quiet', '--locked', '--force',
        '--path', (Join-Path $dir 'pwc-package-manager\crates\pwc-cli'), '--root', (Join-Path $dir 'tools'))
    Remove-Item Env:CARGO_TARGET_DIR
    $pwc = Join-Path $dir 'tools\bin\pwc.exe'

    # All pwc state (instances, worlds, settings, build cache) stays inside the install folder.
    $env:PWC_HOME = Join-Path $dir 'pwc-home'

    Say 'Setting up the game with all first-party modules'
    Run 'Configuring pwc' $pwc @('setup', '-q', '--pwc', (Join-Path $dir 'project_watt_cubed'),
        '--repository', (Join-Path $dir 'pwc-package-manager\modules'))
    $instances = & $pwc instance list
    if ($instances -match '\bdefault\b') {
        Run 'Selecting the game instance' $pwc @('instance', 'use', 'default', '-q')
    } else {
        Run 'Creating the game instance' $pwc @('instance', 'create', 'default', '--use', '-q')
    }
    Run 'Adding the modules' $pwc @('module', 'add', '-q', 'pwc.essentials', 'pwc.dev-toolkit')
    Run 'Updating the modules' $pwc @('module', 'update', '-q')

    # Shortcuts that update and play.
    $script = Join-Path $dir 'pwc-package-manager\scripts\play.ps1'
    $shell = New-Object -ComObject WScript.Shell
    $places = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))
    foreach ($place in $places) {
        $link = $shell.CreateShortcut((Join-Path $place 'Project Watt Cubed.lnk'))
        $link.TargetPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $link.Arguments = "-ExecutionPolicy Bypass -NoProfile -File `"$script`""
        $link.WorkingDirectory = $dir
        $link.Description = 'Update and play Project Watt Cubed'
        $link.Save()
    }

    if ($env:PWC_PLAY_NO_RUN -eq '1') {
        Say "Set up. Start the game with the 'Project Watt Cubed' shortcut."
        return
    }
    Say 'Building and starting the game'
    Write-Host '    The first build compiles the whole game and can take 20-60 minutes. Later starts are quick.'
    Write-Host "    Next time, use the 'Project Watt Cubed' shortcut on your desktop or in the Start menu."
    $saved = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $pwc run
    $code = $LASTEXITCODE
    $ErrorActionPreference = $saved
    if ($code -ne 0) { Fail "The game stopped with exit code $code. Scroll up for the reason." }
}

try {
    Invoke-PwcPlay
} catch {
    Write-Host "`nXX  $($_.Exception.Message)" -ForegroundColor Red
    Write-Host '    Nothing is lost: fix the problem above (or ask in the community chat) and run the same command again.'
    Read-Host 'Press Enter to close'
}
