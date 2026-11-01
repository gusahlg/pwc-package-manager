#!/usr/bin/env bash
# Project Watt Cubed: download, build and play, in one command.
#
#   curl -fsSL https://raw.githubusercontent.com/gusahlg/pwc-package-manager/main/scripts/play.sh | bash
#
# Run the same command again (or the "Project Watt Cubed" icon it installs) to update and play.
# Everything lives in ~/ProjectWattCubed: the game, its worlds and settings, and the tools. To uninstall,
# run: chmod -R u+w ~/ProjectWattCubed && rm -rf ~/ProjectWattCubed. Linux only (x86_64 or arm64; Debian/Ubuntu, Fedora, Arch, openSUSE and their
# derivatives install the needed system packages automatically).
#
# Environment (optional): PWC_PLAY_DIR (install folder), PWC_PLAY_NO_RUN=1 (set up, don't start).

set -euo pipefail

REPOS=(project_watt_cubed voxel-engine pwc-package-manager)
GITHUB=https://github.com/gusahlg

bold=$'\e[1m'; green=$'\e[32m'; yellow=$'\e[33m'; red=$'\e[31m'; reset=$'\e[0m'
say()  { printf '\n%s==> %s%s\n' "$bold$green" "$*" "$reset"; }
warn() { printf '%s!!  %s%s\n' "$bold$yellow" "$*" "$reset"; }
die()  { printf '\n%sXX  %s%s\n' "$bold$red" "$*" "$reset"; exit 1; }

install_system_packages() {
    local marker=$1
    [ -f "$marker" ] && return 0
    local sudo=""
    [ "$(id -u)" -ne 0 ] && sudo="sudo"
    say "Installing the system packages the game needs (you may be asked for your password)"
    if command -v apt-get >/dev/null; then
        $sudo apt-get update
        $sudo apt-get install -y git curl build-essential pkg-config libasound2-dev libdbus-1-dev \
            libvulkan1 mesa-vulkan-drivers libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 \
            libx11-6 libxcursor1 libxrandr2 libxi6
    elif command -v dnf >/dev/null; then
        $sudo dnf install -y git curl gcc gcc-c++ make pkgconf-pkg-config alsa-lib-devel dbus-devel \
            vulkan-loader mesa-vulkan-drivers libxkbcommon libxkbcommon-x11 libwayland-client \
            libX11 libXcursor libXrandr libXi
    elif command -v pacman >/dev/null; then
        $sudo pacman -S --needed --noconfirm git curl base-devel pkgconf alsa-lib dbus \
            vulkan-icd-loader libxkbcommon libxkbcommon-x11 wayland libx11 libxcursor libxrandr libxi
    elif command -v zypper >/dev/null; then
        $sudo zypper --non-interactive install git curl gcc gcc-c++ make pkg-config alsa-devel \
            dbus-1-devel libvulkan1 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libX11-6 \
            libXcursor1 libXrandr2 libXi6
    else
        warn "Unknown Linux distribution: install these yourself if the build fails:"
        warn "  git, curl, a C compiler, pkg-config, ALSA and D-Bus development files, the Vulkan loader"
        warn "  and your GPU's Vulkan driver, libxkbcommon, Wayland and X11 client libraries."
    fi
    touch "$marker"
}

check_vulkan_driver() {
    local d
    for d in /usr/share/vulkan/icd.d /etc/vulkan/icd.d; do
        if [ -d "$d" ] && [ -n "$(ls -A "$d" 2>/dev/null)" ]; then
            return 0
        fi
    done
    warn "No Vulkan graphics driver was found. The game needs one:"
    warn "  AMD or Intel: install your distribution's Mesa Vulkan driver (e.g. vulkan-radeon / vulkan-intel);"
    warn "  NVIDIA: install the proprietary NVIDIA driver."
}

cargo_is_recent() {
    local minor
    minor=$(cargo --version 2>/dev/null | sed -n 's/^cargo 1\.\([0-9]*\)\..*/\1/p')
    [ -n "$minor" ] && [ "$minor" -ge 90 ]
}

ensure_rust() {
    export PATH="$HOME/.cargo/bin:$PATH"
    # A recent enough Rust that is not managed by rustup (a distribution package) is used as is.
    if ! command -v rustup >/dev/null && cargo_is_recent; then
        return 0
    fi
    if ! command -v rustup >/dev/null; then
        say "Installing Rust (the language the game is written in)"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    fi
    # Use the newest stable Rust for this game without changing anyone's default toolchain.
    export RUSTUP_TOOLCHAIN=stable
    rustup toolchain install stable --profile minimal >/dev/null
}

update_repo() {
    local dir=$1 name=$2
    if [ -d "$dir/$name/.git" ]; then
        if ! git -C "$dir/$name" pull --ff-only --quiet; then
            warn "Could not update $name (local changes?); keeping your copy as it is."
        fi
    else
        git clone --quiet --depth 1 "$GITHUB/$name.git" "$dir/$name"
    fi
    printf '    %-22s %s\n' "$name" "$(git -C "$dir/$name" log -1 --format='%h %s' | cut -c1-70)"
}

install_launcher() {
    local dir=$1
    local launcher="$dir/play"
    cat > "$launcher" <<EOF
#!/usr/bin/env bash
# Update Project Watt Cubed and play it.
export PWC_PLAY_DIR="$dir"
exec bash "$dir/pwc-package-manager/scripts/play.sh"
EOF
    chmod +x "$launcher"
    local apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
    mkdir -p "$apps"
    cat > "$apps/project-watt-cubed.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Project Watt Cubed
Comment=Update and play Project Watt Cubed
Exec="$launcher"
Terminal=true
Categories=Game;
EOF
}

main() {
    [ "$(uname -s)" = Linux ] || die "This installer is for Linux."
    case "$(uname -m)" in x86_64 | aarch64) ;; *) die "Unsupported processor: $(uname -m)." ;; esac
    if [ -e /etc/NIXOS ] && [ "${PWC_PLAY_ALLOW_NIXOS:-}" != 1 ]; then
        die "On NixOS, use the game's Nix setup instead of this installer (ask in the community chat)."
    fi

    local dir="${PWC_PLAY_DIR:-$HOME/ProjectWattCubed}"
    mkdir -p "$dir"
    dir=$(cd "$dir" && pwd)

    local free_gb
    free_gb=$(df -Pk "$dir" | awk 'NR == 2 { print int($4 / 1048576) }')
    if [ "$free_gb" -lt 10 ]; then
        warn "Only ${free_gb} GB free in $dir; the first build needs about 10 GB."
    fi

    if [ "${PWC_PLAY_SKIP_SYSTEM:-}" != 1 ]; then
        install_system_packages "$dir/.system-packages-installed"
    fi
    command -v git >/dev/null || die "git is missing; install it and run this again."
    check_vulkan_driver
    ensure_rust

    say "Downloading the latest game, renderer and package manager"
    local name
    for name in "${REPOS[@]}"; do
        update_repo "$dir" "$name"
    done

    # Keep memory use safe on small machines: roughly one compile job per 2 GB of RAM.
    local mem_gb jobs
    mem_gb=$(awk '/MemTotal/ { print int($2 / 1048576) }' /proc/meminfo)
    jobs=$(( mem_gb / 2 ))
    [ "$jobs" -lt 1 ] && jobs=1
    [ "$jobs" -gt "$(nproc)" ] && jobs=$(nproc)
    export CARGO_BUILD_JOBS=$jobs

    say "Building the package manager (pwc)"
    CARGO_TARGET_DIR="$dir/.build/pwc-tools" cargo install --quiet --locked --force \
        --path "$dir/pwc-package-manager/crates/pwc-cli" --root "$dir/tools"
    local pwc="$dir/tools/bin/pwc"

    # All pwc state (instances, worlds, settings, build cache) stays inside $dir.
    export PWC_HOME="$dir/pwc-home"
    # Build with this system's own libraries, not the developers' Nix setup.
    export PWC_NO_WRAPPER=1

    say "Setting up the game with all first-party modules"
    "$pwc" setup -q --pwc "$dir/project_watt_cubed" --repository "$dir/pwc-package-manager/modules"
    "$pwc" instance create default --use -q 2>/dev/null || "$pwc" instance use default -q
    "$pwc" module add -q pwc.essentials pwc.dev-toolkit
    "$pwc" module update -q

    install_launcher "$dir"

    if [ "${PWC_PLAY_NO_RUN:-}" = 1 ]; then
        say "Set up. Start the game with: $dir/play"
        return 0
    fi
    say "Building and starting the game"
    echo "    The first build compiles the whole game and can take 15-40 minutes. Later starts are quick."
    echo "    Next time, use the 'Project Watt Cubed' icon or run: $dir/play"
    exec "$pwc" run
}

main "$@"; exit
