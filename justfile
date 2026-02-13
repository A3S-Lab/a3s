# A3S - Justfile

default:
    @just --list

# ============================================================================
# A3S Box
# ============================================================================

# Build a3s-box
box-build:
    cd crates/box && just build

# Run a3s-box unit tests
box-test:
    cd crates/box && just test

# Run a3s-box VM integration tests (requires built binary + HVF/KVM)
# Usage: just box-test-vm                          # run all tests
#        just box-test-vm test_alpine_full_lifecycle  # run a specific test
box-test-vm *ARGS:
    cd crates/box && just test-vm {{ARGS}}

# Run a3s-box TEE integration tests (requires built binary + HVF/KVM)
# Usage: just box-test-tee                              # run all TEE tests
#        just box-test-tee test_tee_seal_unseal_lifecycle  # run a specific test
box-test-tee *ARGS:
    cd crates/box && just test-tee {{ARGS}}

# ============================================================================
# A3S Code
# ============================================================================

# Start a3s-code server (dev mode)
code:
    cd crates/code && just serve
