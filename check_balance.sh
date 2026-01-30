#!/bin/bash
# Quick script to check account balance from the state store

DB_PATH="${1:-./state_db}"
ADDRESS="${2}"

if [ -z "$ADDRESS" ]; then
    echo "Usage: $0 [db_path] <address>"
    echo "Example: $0 ./state_db 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    exit 1
fi

cargo run --bin statectl -- --db-path "$DB_PATH" get-account "$ADDRESS"
