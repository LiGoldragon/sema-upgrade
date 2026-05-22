{
  description = "sema-upgrade - runtime prototype for Sema schema upgrades";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    persona-spirit = {
      url = "github:LiGoldragon/persona-spirit?ref=v0.1.1";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix, crane, persona-spirit }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        toolchain = fenix.packages.${system}.stable.withComponents [
          "cargo"
          "rustc"
          "rustfmt"
          "clippy"
          "rust-src"
        ];
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = craneLib.filterCargoSources;
          name = "source";
        };
        cargoVendorDirectory = craneLib.vendorCargoDeps { inherit src; };
        commonArguments = {
          inherit src cargoVendorDirectory;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArguments;
        package = craneLib.buildPackage (commonArguments // { inherit cargoArtifacts; });
        spiritMigrationSandbox = pkgs.writeShellApplication {
          name = "spirit-migration-sandbox";
          runtimeInputs = [ pkgs.coreutils pkgs.gnugrep ];
          text = ''
            set -euo pipefail

            if [ "$#" -ne 1 ]; then
              echo "usage: spirit-migration-sandbox <v0.1.0-persona-spirit.redb>" >&2
              exit 2
            fi

            source_database="$1"
            if [ ! -f "$source_database" ]; then
              echo "source database does not exist: $source_database" >&2
              exit 2
            fi

            work_directory="$(mktemp -d)"
            daemon_pid=""
            cleanup() {
              if [ -n "$daemon_pid" ] && kill -0 "$daemon_pid" 2>/dev/null; then
                kill "$daemon_pid" 2>/dev/null || true
                wait "$daemon_pid" 2>/dev/null || true
              fi
              rm -rf "$work_directory"
            }
            trap cleanup EXIT

            source_copy="$work_directory/source-v0.1.0.redb"
            target_database="$work_directory/target-v0.1.1.redb"
            ordinary_socket="$work_directory/spirit.sock"
            owner_socket="$work_directory/owner.sock"
            daemon_log="$work_directory/persona-spirit-daemon.log"

            cp --reflink=auto "$source_database" "$source_copy" 2>/dev/null || cp "$source_database" "$source_copy"

            "${package}/bin/sema-upgrade-temporary" \
              "(Attempt (\"$source_copy\" \"$target_database\" (persona-spirit (0 1 0) (0 1 1))))" \
              > "$work_directory/migration.reply"

            "${persona-spirit.packages.${system}.persona-spirit-daemon}/bin/persona-spirit-daemon" \
              "(\"$ordinary_socket\" \"$owner_socket\" \"$target_database\" 384 None)" \
              > "$daemon_log" 2>&1 &
            daemon_pid="$!"

            for _ in $(seq 1 100); do
              if [ -S "$ordinary_socket" ] && [ -S "$owner_socket" ]; then
                break
              fi
              if ! kill -0 "$daemon_pid" 2>/dev/null; then
                echo "persona-spirit-daemon exited before sockets were ready" >&2
                cat "$daemon_log" >&2
                exit 1
              fi
              sleep 0.05
            done

            if [ ! -S "$ordinary_socket" ] || [ ! -S "$owner_socket" ]; then
              echo "persona-spirit-daemon did not create sandbox sockets" >&2
              cat "$daemon_log" >&2
              exit 1
            fi

            run_spirit() {
              PERSONA_SPIRIT_SOCKET="$ordinary_socket" \
              PERSONA_SPIRIT_OWNER_SOCKET="$owner_socket" \
                "${persona-spirit.packages.${system}.spirit}/bin/spirit" "$1"
            }

            topics_reply="$(run_spirit '(Observe Topics)')"
            case "$topics_reply" in
              \(TopicsObserved*) ;;
              *)
                echo "unexpected topics reply: $topics_reply" >&2
                exit 1
                ;;
            esac

            records_reply="$(run_spirit '(Observe (Records (None None SummaryOnly)))')"
            case "$records_reply" in
              \(RecordsObserved*) ;;
              *)
                echo "unexpected records reply: $records_reply" >&2
                exit 1
                ;;
            esac

            accepted_reply="$(run_spirit '(Record (testing Constraint "sandbox accepts high magnitude" "migrated live database copy" High "sandbox high quote"))')"
            case "$accepted_reply" in
              \(RecordAccepted*) ;;
              *)
                echo "unexpected record reply: $accepted_reply" >&2
                exit 1
                ;;
            esac

            high_reply="$(run_spirit '(Observe (Records ((Some testing) (Some Constraint) SummaryOnly)))')"
            echo "$high_reply" | grep -q '"sandbox accepts high magnitude"'
            echo "$high_reply" | grep -q 'High'

            printf '%s\n' "$(< "$work_directory/migration.reply")"
            printf '%s\n' "$topics_reply"
            printf '%s\n' "$accepted_reply"
            printf '%s\n' "$high_reply"
            printf '(SandboxMigrationSucceeded ("%s"))\n' "$source_database"
          '';
        };
        spiritMigrationStage = pkgs.writeShellApplication {
          name = "spirit-migration-stage";
          runtimeInputs = [ pkgs.coreutils pkgs.gnugrep ];
          text = ''
            set -euo pipefail

            if [ "$#" -ne 2 ]; then
              echo "usage: spirit-migration-stage <v0.1.0-persona-spirit.redb> <v0.1.1-persona-spirit.redb>" >&2
              exit 2
            fi

            source_database="$1"
            target_database="$2"
            target_directory="$(dirname "$target_database")"

            if [ ! -f "$source_database" ]; then
              echo "source database does not exist: $source_database" >&2
              exit 2
            fi
            if [ ! -d "$target_directory" ]; then
              echo "target directory does not exist: $target_directory" >&2
              exit 2
            fi

            stage_directory="$(mktemp -d "$target_directory/.spirit-migration-stage.XXXXXX")"
            daemon_pid=""
            cleanup() {
              if [ -n "$daemon_pid" ] && kill -0 "$daemon_pid" 2>/dev/null; then
                kill "$daemon_pid" 2>/dev/null || true
                wait "$daemon_pid" 2>/dev/null || true
              fi
              rm -rf "$stage_directory"
            }
            trap cleanup EXIT

            source_copy="$stage_directory/source-v0.1.0.redb"
            staged_database="$stage_directory/target-v0.1.1.redb"
            probe_database="$stage_directory/probe-v0.1.1.redb"
            ordinary_socket="$stage_directory/spirit.sock"
            owner_socket="$stage_directory/owner.sock"
            daemon_log="$stage_directory/persona-spirit-daemon.log"
            backup_database=""

            cp --reflink=auto "$source_database" "$source_copy" 2>/dev/null || cp "$source_database" "$source_copy"

            "${package}/bin/sema-upgrade-temporary" \
              "(Attempt (\"$source_copy\" \"$staged_database\" (persona-spirit (0 1 0) (0 1 1))))" \
              > "$stage_directory/migration.reply"

            cp --reflink=auto "$staged_database" "$probe_database" 2>/dev/null || cp "$staged_database" "$probe_database"

            "${persona-spirit.packages.${system}.persona-spirit-daemon}/bin/persona-spirit-daemon" \
              "(\"$ordinary_socket\" \"$owner_socket\" \"$probe_database\" 384 None)" \
              > "$daemon_log" 2>&1 &
            daemon_pid="$!"

            for _ in $(seq 1 100); do
              if [ -S "$ordinary_socket" ] && [ -S "$owner_socket" ]; then
                break
              fi
              if ! kill -0 "$daemon_pid" 2>/dev/null; then
                echo "persona-spirit-daemon exited before sockets were ready" >&2
                cat "$daemon_log" >&2
                exit 1
              fi
              sleep 0.05
            done

            if [ ! -S "$ordinary_socket" ] || [ ! -S "$owner_socket" ]; then
              echo "persona-spirit-daemon did not create staging sockets" >&2
              cat "$daemon_log" >&2
              exit 1
            fi

            run_spirit() {
              PERSONA_SPIRIT_SOCKET="$ordinary_socket" \
              PERSONA_SPIRIT_OWNER_SOCKET="$owner_socket" \
                "${persona-spirit.packages.${system}.spirit}/bin/spirit" "$1"
            }

            topics_reply="$(run_spirit '(Observe Topics)')"
            case "$topics_reply" in
              \(TopicsObserved*) ;;
              *)
                echo "unexpected topics reply: $topics_reply" >&2
                exit 1
                ;;
            esac

            records_reply="$(run_spirit '(Observe (Records ((Some spirit) (Some Decision) SummaryOnly)))')"
            case "$records_reply" in
              \(RecordsObserved*) ;;
              *)
                echo "unexpected records reply: $records_reply" >&2
                exit 1
                ;;
            esac

            echo "$records_reply" | grep -q '"start using spirit v0.1.1 after database update"'

            accepted_reply="$(run_spirit '(Record (testing Constraint "staged database accepts high magnitude" "verified before persistent install" High "staging high quote"))')"
            case "$accepted_reply" in
              \(RecordAccepted*) ;;
              *)
                echo "unexpected record reply: $accepted_reply" >&2
                exit 1
                ;;
            esac

            high_reply="$(run_spirit '(Observe (Records ((Some testing) (Some Constraint) SummaryOnly)))')"
            echo "$high_reply" | grep -q '"staged database accepts high magnitude"'
            echo "$high_reply" | grep -q 'High'

            if kill -0 "$daemon_pid" 2>/dev/null; then
              kill "$daemon_pid" 2>/dev/null || true
              wait "$daemon_pid" 2>/dev/null || true
              daemon_pid=""
            fi

            if [ -e "$target_database" ]; then
              timestamp="$(date -u +%Y%m%d%H%M%S)"
              backup_database="$target_database.backup-$timestamp"
              mv "$target_database" "$backup_database"
            fi

            mv "$staged_database" "$target_database"

            printf '%s\n' "$(< "$stage_directory/migration.reply")"
            printf '(StageReadProbeSucceeded)\n'
            printf '(StageWriteProbeSucceeded)\n'
            if [ -n "$backup_database" ]; then
              printf '(StageInstalled ("%s" "%s" "%s"))\n' "$source_database" "$target_database" "$backup_database"
            else
              printf '(StageInstalled ("%s" "%s" None))\n' "$source_database" "$target_database"
            fi
          '';
        };
      in
      {
        packages = {
          default = package;
          spirit-migration-sandbox = spiritMigrationSandbox;
          spirit-migration-stage = spiritMigrationStage;
        };
        apps.spirit-migration-sandbox = flake-utils.lib.mkApp {
          drv = spiritMigrationSandbox;
          name = "spirit-migration-sandbox";
        };
        apps.spirit-migration-stage = flake-utils.lib.mkApp {
          drv = spiritMigrationStage;
          name = "spirit-migration-stage";
        };
        checks = {
          build = craneLib.cargoBuild (commonArguments // { inherit cargoArtifacts; });
          test = craneLib.cargoTest (commonArguments // { inherit cargoArtifacts; });
          doc = craneLib.cargoDoc (commonArguments // {
            inherit cargoArtifacts;
            RUSTDOCFLAGS = "-D warnings";
          });
          fmt = craneLib.cargoFmt { inherit src; };
          clippy = craneLib.cargoClippy (commonArguments // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });
        };
        devShells.default = pkgs.mkShell {
          name = "sema-upgrade";
          packages = [ pkgs.jujutsu pkgs.pkg-config toolchain ];
        };
      });
}
