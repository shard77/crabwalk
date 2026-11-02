{
  description = "Development environment and static musl release builds for crabwalk";

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
          releaseTarget = pkgs.pkgsStatic.stdenv.hostPlatform.rust.rustcTarget;
          releaseTargetEnv = pkgs.pkgsStatic.stdenv.hostPlatform.rust.cargoEnvVarTarget;
          releaseToolchain = pkgs.rust-bin.stable.latest.minimal.override {
            targets = [ releaseTarget ];
          };
          releasePlatform = pkgs.makeRustPlatform {
            cargo = releaseToolchain;
            rustc = releaseToolchain;
          };
          manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          releasePackage = releasePlatform.buildRustPackage {
            pname = manifest.package.name;
            version = manifest.package.version;
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./src
              ];
            };
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "ratatui-diff-0.1.0" = "sha256-fVoIavlx1OVlB49fJOS4FD5LhcnUBlir6kUO5viuvzU=";
              };
            };
            # Build scripts and proc macros run on the GNU host. Only the final
            # application and its C dependencies use the musl target toolchain.
            env = {
              TARGET_CC = "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
              "CARGO_TARGET_${releaseTargetEnv}_LINKER" = "rust-lld";
              "CARGO_TARGET_${releaseTargetEnv}_RUSTFLAGS" = "-C target-feature=+crt-static";
            };
            buildPhase = ''
              runHook preBuild
              cargo build --offline --locked --release --bin crabwalk \
                --target ${releaseTarget} -j "$NIX_BUILD_CORES"
              runHook postBuild
            '';
            checkPhase = ''
              runHook preCheck
              cargo test --offline --locked --release --workspace --all-features \
                --target ${releaseTarget} -j "$NIX_BUILD_CORES"
              runHook postCheck
            '';
            installPhase = ''
              runHook preInstall
              install -Dm755 target/${releaseTarget}/release/crabwalk "$out/bin/crabwalk"
              runHook postInstall
            '';
            doCheck = true;
            nativeBuildInputs = [ pkgs.binutils ];
            postFixup = ''
              readelf --program-headers "$out/bin/crabwalk" > program-headers.txt
              readelf --dynamic "$out/bin/crabwalk" > dynamic-section.txt
              if grep -q INTERP program-headers.txt || grep -q NEEDED dynamic-section.txt; then
                echo "Release binary must not require a dynamic loader or shared libraries" >&2
                exit 1
              fi
            '';
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
          inherit pkgs releasePackage;
          devShell = pkgs.mkShell {
            packages = [
              rustToolchain
            ]
            ++ (with pkgs; [
              nixd
              nixfmt
              prek
              cocogitto
              cargo-edit
              actionlint
            ])
            ++ preCommit.enabledPackages;
            shellHook = preCommit.shellHook;
          };
        }
      );
    in
    {
      packages = nixpkgs.lib.mapAttrs (_: value: {
        default = value.releasePackage;
        crabwalk = value.releasePackage;
      }) perSystem;
      devShells = nixpkgs.lib.mapAttrs (_: value: { default = value.devShell; }) perSystem;
      formatter = nixpkgs.lib.mapAttrs (_: value: value.pkgs.nixfmt) perSystem;
    };
}
