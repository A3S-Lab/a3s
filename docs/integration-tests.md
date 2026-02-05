# Integration Tests

This directory contains integration tests for A3S Code and its SDKs. These tests use the real configuration file from `.a3s/config.json` to verify that all components work correctly with actual configuration.

## Test Structure

### Rust Integration Tests
**Location**: `crates/code/tests/integration_test.rs`

Tests the A3S Code configuration loading and provider management:
- Loading config from `.a3s/config.json`
- Provider and model lookup
- LLM configuration extraction
- Model capabilities, costs, limits, and modalities
- Alternate provider support

**Run**: `just test-integration-code`

### TypeScript SDK Integration Tests
**Location**: `sdk/typescript/ts/__tests__/integration.test.ts`

Tests the TypeScript SDK configuration loading:
- Config file loading
- Provider and model discovery
- API key extraction
- Model metadata (cost, limits, modalities)
- Alternate provider support

**Run**: `just test-integration-ts`

### Python SDK Integration Tests
**Location**: `sdk/python/tests/test_integration.py`

Tests the Python SDK configuration loading:
- Config file and directory loading
- Provider and model discovery
- API key extraction
- Model metadata (cost, limits, modalities)
- Default address configuration
- Alternate provider support

**Run**: `just test-integration-py`

## Running Tests

### Run All Integration Tests
```bash
just test-integration
```

This runs all integration tests across Rust, TypeScript, and Python SDKs.

### Run Individual Component Tests
```bash
# Rust only
just test-integration-code

# TypeScript SDK only
just test-integration-ts

# Python SDK only
just test-integration-py
```

### Run All Tests (Unit + Integration)
```bash
just test-all
```

This runs both unit tests and integration tests for all components.

## Test Results

Current test counts:
- **Rust Integration Tests**: 9 tests
- **TypeScript SDK Integration Tests**: 11 tests
- **Python SDK Integration Tests**: 13 tests
- **Total Integration Tests**: 33 tests

Combined with unit tests:
- **Total Tests**: 848 tests (7 components)

## Configuration File

The integration tests use the real configuration file at:
```
a3s/.a3s/config.json
```

This file contains:
- Provider configurations (anthropic, openai)
- Model definitions with capabilities
- API keys and base URLs
- Cost and limit information
- Modality specifications

## Test Coverage

The integration tests verify:

✅ **Configuration Loading**
- Loading from file path
- Loading from directory
- Default configuration fallback
- Environment variable support

✅ **Provider Management**
- Finding providers by name
- Listing all providers
- Default provider selection
- API key extraction
- Base URL configuration

✅ **Model Discovery**
- Finding models by ID
- Listing all available models
- Model capabilities (tool_call, reasoning, attachment)
- Model families and release dates

✅ **Model Metadata**
- Cost information (input, output, cache read/write)
- Token limits (context, output)
- Modalities (input/output types)

✅ **Multi-Provider Support**
- Primary provider (anthropic)
- Alternate providers (openai)
- Model-specific overrides

## Adding New Integration Tests

When adding new features to A3S Code or SDKs:

1. **Add Rust tests** to `crates/code/tests/integration_test.rs`
2. **Add TypeScript tests** to `sdk/typescript/ts/__tests__/integration.test.ts`
3. **Add Python tests** to `sdk/python/tests/test_integration.py`

Ensure tests:
- Use the real config file from `.a3s/config.json`
- Skip gracefully if config file doesn't exist
- Print useful output for debugging
- Verify actual behavior, not mocked responses

## CI/CD Integration

Integration tests can be run in CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Run Integration Tests
  run: just test-integration
```

Note: Ensure `.a3s/config.json` is available in the CI environment or tests will be skipped.

## Troubleshooting

### Config File Not Found
If tests are skipped with "Config file not found":
- Ensure `.a3s/config.json` exists in the workspace root
- Check file permissions
- Verify the path is correct relative to test location

### API Key Issues
If tests fail with API key errors:
- Verify API keys are set in `.a3s/config.json`
- Check that keys have proper permissions
- Ensure base URLs are accessible

### Port Configuration
All SDKs should default to `localhost:4088`:
- Rust: Uses port 4088
- TypeScript: Defaults to `localhost:4088`
- Python: Defaults to `localhost:4088`

## Related Documentation

- [A3S Code README](../../crates/code/README.md)
- [TypeScript SDK README](../../sdk/typescript/README.md)
- [Python SDK README](../../sdk/python/README.md)
- [Configuration Guide](../../docs/configuration.md)
