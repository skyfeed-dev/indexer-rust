{
  description = "bluesky indexer in rust";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            fenix.overlays.default
          ];
        };

        lib = pkgs.lib;

        fenixPkgs = fenix.packages.${system};
        rustToolchain = fenixPkgs.combine [
          fenixPkgs.complete.toolchain
          (fenixPkgs.complete.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
          ])
        ];
      in
      {
        name = "indexer-rust";

        devShell = pkgs.mkShell {
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LD_LIBRARY_PATH = lib.makeLibraryPath [
            pkgs.libgcc.lib
            pkgs.openssl
          ];
          buildInputs = [
            rustToolchain
            pkgs.rust-analyzer-nightly

            pkgs.openssl
            pkgs.pkg-config
            pkgs.clang
          ];

          shellHook = ''
            DATABASE_DIR="/tmp/safjlgfdgsfdgsfdgsd"
            mkdir -p $DATABASE_DIR
            export DATABASE_DIR

            echo 'cargo run -- --db "rocksdb://'$DATABASE_DIR'" --mode full -c '$(pwd)'/ISRG_Root_X1.pem'
          '';
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
