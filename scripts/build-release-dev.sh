#!/usr/bin/env bash
# lmk if you guys want to test server code and i can deploy it for you

branch=$(git branch --show-current)
target=$1

if [[ -z $target ]]; then
    echo "Please specify the directory you want to deploy to the test server"
    exit 1
fi

echo "Deploying test branch ${target} to GCP"
git checkout release-dev
git merge ${target}

# Push to deployment server
git push private release-dev --force-with-lease

echo "Checking back out to your previous branch"
git checkout ${branch}
