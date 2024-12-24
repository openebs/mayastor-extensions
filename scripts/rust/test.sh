#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$0")"

ARGS=""
OPTS=""
DO_ARGS=
while [ "$#" -gt 0 ]; do
  case $1 in
    --)
      DO_ARGS="y"
      shift;;
    *)
      if [ "$DO_ARGS" == "y" ]; then
        ARGS="$ARGS $1"
      else
        OPTS="$OPTS $1"
      fi
      shift;;
  esac
done

set -euxo pipefail

# build test dependencies
cargo build --bins

cargo test ${OPTS} -- ${ARGS} --test-threads=1

