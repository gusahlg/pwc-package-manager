{
  description = "pwc — the Project Watt Cubed package manager and mod builder";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        # The tooling is pure Rust: no C toolchain, pkg-config or system
        # libraries. Building a game instance (`pwc build`) runs Cargo inside
        # the game's own flake shell, which provides the game's native
        # libraries (docs/spec/filesystem.md, `[build] wrapper`). `reuse`
        # checks per-file licensing (REUSE.toml).
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [ rustc cargo rustfmt clippy git reuse ];
        };

        # The `pwc` binary as a Nix package. Install it with `nix profile install <this flake>`:
        # a profile is a garbage-collection root, so the C library it links against stays alive.
        # (A binary built in the dev shell and copied onto PATH points at a store path nothing
        # roots; the next garbage collection removes it and the binary stops starting.)
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "pwc";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
          src = pkgs.lib.cleanSource ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "pwc-cli" ];
          # The workspace tests run in CI and the dev shell; the package only builds the tool.
          doCheck = false;
          meta = {
            description = "The Project Watt Cubed package manager and mod builder";
            license = pkgs.lib.licenses.agpl3Plus;
            mainProgram = "pwc";
          };
        };
      });
}
