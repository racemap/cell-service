#!/bin/sh

# prepare database
diesel setup

exec "$@"