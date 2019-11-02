{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  # Things to be put in $PATH
  nativeBuildInputs = with pkgs; [ pkgconfig ];

  # Libraries to be installed
  buildInputs = with pkgs; [ fuse ];

  RUST_LOG = "trace";

  shellHook = ''
    # Let wrappers take precedence.
    export PATH="/var/run/wrappers/bin:$PATH"
  '';
}
