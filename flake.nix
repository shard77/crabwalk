{
  description = "Development environment for crabwalk";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      git-hooks,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      perSystem = nixpkgs.lib.genAttrs systems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
              "clippy"
              "rustfmt"
            ];
          };
          preCommit = git-hooks.lib.${system}.run {
            src = ./.;
            package = pkgs.prek;
            hooks = {
              clippy = {
                enable = true;
                packageOverrides = {
                  cargo = rustToolchain;
                  clippy = rustToolchain;
                };
              };
              rustfmt = {
                enable = true;
                packageOverrides = {
                  cargo = rustToolchain;
                  rustfmt = rustToolchain;
                };
              };
            };
          };
        in
        {
          inherit pkgs;
          devShell = pkgs.mkShell {
            packages = [
              rustToolchain
            ]
            ++ (with pkgs; [
              nixd
              nixfmt
              prek
            ])
            ++ preCommit.enabledPackages;
            shellHook = preCommit.shellHook;
          };
        }
      );
    in
    {
      devShells = nixpkgs.lib.mapAttrs (_: value: { default = value.devShell; }) perSystem;
      formatter = nixpkgs.lib.mapAttrs (_: value: value.pkgs.nixfmt) perSystem;
    };
}
