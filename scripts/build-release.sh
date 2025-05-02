#!/usr/bin/env bash

branch=$(git branch --show-current)

if [[ "$branch" != "main" ]]; then
    echo "Not in main branch"
    exit 1
fi

# Make sure we are on the latest version of main
git pull origin main

# Push to deployment server
git push private release --force-with-lease
