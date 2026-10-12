#!/usr/bin/env bash

export MIRI_BORROW_MODEL=tree-borrows
export MIRI_STRICT_PROVENANCE=1
exec "$(dirname "${BASH_SOURCE[0]}")/check-miri.sh" "$@"
