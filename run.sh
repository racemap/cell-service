#!/bin/sh
set -eEuo pipefail

# prepare database
diesel setup
diesel migration run

exec "$@"