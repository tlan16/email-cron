#!/usr/bin/env bash
set -euro pipefail

git clone "git@github.com:tlan16/email-cron.git" --no-checkout --depth 1 --filter=blob:none
cd email-cron || exit 1
git sparse-checkout init --cone
echo "/email-cron-aarch64" > .git/info/sparse-checkout
git checkout
