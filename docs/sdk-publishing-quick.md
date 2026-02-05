# SDK Publishing Quick Reference

## Quick Commands

```bash
# Check all versions
just version

# Dry run (verify without publishing)
just publish-sdk-dry

# Publish TypeScript SDK to npm
just publish-sdk-ts

# Publish Python SDK to PyPI
just publish-sdk-py

# Publish both SDKs
just publish-sdk

# Run SDK tests
just test-sdk
just test-sdk-ts
just test-sdk-py
```

## Before Publishing

1. **Update version numbers**:
   - TypeScript: `sdk/typescript/package.json`
   - Python: `sdk/python/pyproject.toml`

2. **Run tests**:
   ```bash
   just test-sdk
   ```

3. **Verify with dry run**:
   ```bash
   just publish-sdk-dry
   ```

## Authentication

### npm (TypeScript)
```bash
npm login
```

### PyPI (Python)
Create `~/.pypirc`:
```ini
[pypi]
username = __token__
password = pypi-YOUR_API_TOKEN
```

Or set environment variables:
```bash
export TWINE_USERNAME=__token__
export TWINE_PASSWORD=pypi-YOUR_API_TOKEN
```

## Publishing

```bash
# Publish both SDKs
just publish-sdk

# Or publish individually
just publish-sdk-ts  # TypeScript → npm
just publish-sdk-py  # Python → PyPI
```

## GitHub Actions

The workflow `.github/workflows/publish-sdk.yml` can be triggered:

1. **Manually**: Actions → Publish SDKs → Run workflow
2. **Git tags**:
   ```bash
   git tag sdk-ts-v0.1.0  # For TypeScript
   git tag sdk-py-v0.1.0  # For Python
   git push origin --tags
   ```

### Required Secrets

Add to GitHub repository settings:
- `NPM_TOKEN`: npm access token
- `PYPI_TOKEN`: PyPI API token

## Files Created

- `.github/workflows/publish-sdk.yml` - GitHub Actions workflow
- `sdk/typescript/.npmignore` - npm publish exclusions
- `sdk/python/MANIFEST.in` - PyPI source distribution inclusions
- `docs/sdk-publishing.md` - Detailed publishing guide
- `docs/sdk-publishing-quick.md` - This quick reference

## Troubleshooting

**Permission denied**:
- npm: Check `npm whoami` and organization access
- PyPI: Verify API token is valid

**Version already exists**:
- Bump version in package.json/pyproject.toml

**Build errors**:
```bash
# TypeScript
cd sdk/typescript && rm -rf dist node_modules && npm install && npm run build

# Python
cd sdk/python && rm -rf dist build *.egg-info && python -m build
```

## Package URLs

- TypeScript: https://www.npmjs.com/package/@a3s-lab/code
- Python: https://pypi.org/project/a3s-code/

## Full Documentation

See `docs/sdk-publishing.md` for complete details.
