#!/bin/sh
set -eEuo pipefail

# prepare database
diesel setup

exec "$@"