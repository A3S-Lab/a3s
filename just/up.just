# Start recipes: `just up::registry`
#
# Use Registry local static HTTP transport for the signed
# use-registry/registry/ tree. Not the Cloud OCI registry (port 50020).

[private]
use-registry-submodule:
    sh "{{ source_directory() }}/../scripts/ensure-dev-submodules.sh" use-registry:registry/metadata/root.json

# Start the Use Registry local transport (default http://127.0.0.1:4873/)
registry: use-registry-submodule
    cd "{{ source_directory() }}/../use-registry" && ./scripts/serve_local.sh start
    @cd "{{ source_directory() }}/../use-registry" && ./scripts/serve_local.sh url
