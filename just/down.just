# Stop recipes: `just down::registry`
#
# Use Registry local static HTTP transport. Not the Cloud OCI registry.

[private]
use-registry-submodule:
    sh "{{ source_directory() }}/../scripts/ensure-dev-submodules.sh" \
        use-registry:registry/metadata/root.json \
        use-registry:registry/targets/extensions/a3s/applet-demo/0.1.0/stable/any/a3s-applet-demo-0.1.0-any.tar.gz

# Stop the Use Registry local transport
registry: use-registry-submodule
    cd "{{ source_directory() }}/../use-registry" && ./scripts/serve_local.sh stop
