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
      });
}
