#!/usr/bin/env sh
# Refreshes skies-plugin/docs/ from its sources: verbatim copies of the convention docs in docs/ and the CLI
# reference rendered from `skies --help`. The copying lives in cli/tests/plugin_docs.rs, the same test that fails
# when the plugin drifts, so there is one list of synced files.
set -eu
cd "$(dirname "$0")/.."
SKIES_SYNC_PLUGIN_DOCS=1 cargo test --quiet --test plugin_docs
echo "skies-plugin/docs/ is in sync"
