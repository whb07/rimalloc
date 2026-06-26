#!/usr/bin/env bash

export MIRI_BORROW_MODEL=tree-borrows
exec "$(dirname "${BASH_SOURCE[0]}")/check-miri.sh" "$@"
