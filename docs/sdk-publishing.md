# SDK Publishing Guide

This guide explains how to publish the A3S Code SDKs to npm and PyPI.

## Prerequisites

### TypeScript SDK (npm)

1. **npm account**: Create an account at https://www.npmjs.com/signup
2. **Organization access**: Request access to `@a3s-lab` organization
3. **Login**: Run `npm login` to authenticate

### Python SDK (PyPI)

1. **PyPI account**: Create an account at https://pypi.org/account/register/
2. **API token**: Generate a token at https://pypi.org/manage/account/token/
3. **Install tools**:
   ```bash
   pip install build twine
   ```

## Publishing Methods

### Method 1: Using Justfile Commands (Recommended)

The project includes convenient justfile commands for publishing:

```bash
# Dry run (verify without publishing)
just publish-sdk-dry

# Publish TypeScript SDK only
just publish-sdk-ts

# Publish Python SDK only
just publish-sdk-py

# Publish both SDKs
just publish-sdk
```

### Method 2: Manual Publishing

#### TypeScript SDK

```bash
cd sdk/typescript

# 1. Install dependencies
npm install

# 2. Run tests
npm test

# 3. Build
npm run build

# 4. Verify (dry run)
npm publish --dry-run

# 5. Publish
npm publish --access public
```

#### Python SDK

```bash
cd sdk/python

# 1. Run tests
python -m pytest tests/ -v

# 2. Clean old builds
rm -rf dist/ build/ *.egg-info/

# 3. Build
python -m build

# 4. Verify
twine check dist/*

# 5. Publish
twine upload dist/*
```

### Method 3: GitHub Actions (Automated)

The repository includes a GitHub Actions workflow for automated publishing.

#### Setup Secrets

Add these secrets to your GitHub repository (Settings → Secrets and variables → Actions):

- `NPM_TOKEN`: Your npm access token
- `PYPI_TOKEN`: Your PyPI API token

#### Trigger Publishing

**Option A: Manual Workflow Dispatch**

1. Go to Actions → Publish SDKs
2. Click "Run workflow"
3. Select which SDK to publish (typescript/python/both)
4. Click "Run workflow"

**Option B: Git Tags**

Create and push a tag to trigger automatic publishing:

```bash
# For TypeScript SDK
git tag sdk-ts-v0.1.0
git push origin sdk-ts-v0.1.0

# For Python SDK
git tag sdk-py-v0.1.0
git push origin sdk-py-v0.1.0
```

**Option C: GitHub Release**

Create a GitHub release with a tag starting with `sdk-ts-` or `sdk-py-` to trigger publishing.

## Version Management

### TypeScript SDK

Edit `sdk/typescript/package.json`:

```json
{
  "version": "0.2.0"
}
```

Or use npm version command:

```bash
cd sdk/typescript

# Patch: 0.1.0 → 0.1.1
npm version patch

# Minor: 0.1.0 → 0.2.0
npm version minor

# Major: 0.1.0 → 1.0.0
npm version major
```

### Python SDK

Edit `sdk/python/pyproject.toml`:

```toml
[project]
version = "0.2.0"
```

## Pre-Publish Checklist

Before publishing, ensure:

- [ ] All tests pass (`just test-sdk`)
- [ ] Version number is updated in package.json/pyproject.toml
- [ ] CHANGELOG is updated (if exists)
- [ ] README is up to date
- [ ] Git changes are committed
- [ ] Git tag is created (optional, for tracking)

## Authentication Setup

### npm

**Interactive login:**
```bash
npm login
```

**Using token (CI/CD):**
```bash
npm config set //registry.npmjs.org/:_authToken=$NPM_TOKEN
```

**Generate token:**
1. Go to https://www.npmjs.com/settings/YOUR_USERNAME/tokens
2. Click "Generate New Token"
3. Select "Automation" type
4. Copy the token

### PyPI

**Create `~/.pypirc`:**
```ini
[pypi]
username = __token__
password = pypi-YOUR_API_TOKEN

[testpypi]
username = __token__
password = pypi-YOUR_TEST_API_TOKEN
```

**Or use environment variables:**
```bash
export TWINE_USERNAME=__token__
export TWINE_PASSWORD=pypi-YOUR_API_TOKEN
```

**Generate token:**
1. Go to https://pypi.org/manage/account/token/
2. Click "Add API token"
3. Set scope (entire account or specific project)
4. Copy the token

## Testing Before Publishing

### Test on TestPyPI (Python only)

```bash
cd sdk/python

# Build
python -m build

# Upload to TestPyPI
twine upload --repository testpypi dist/*

# Test install
pip install --index-url https://test.pypi.org/simple/ a3s-code
```

### Dry Run

```bash
# Test both SDKs
just publish-sdk-dry

# Or manually
cd sdk/typescript && npm publish --dry-run
cd sdk/python && twine check dist/*
```

## Troubleshooting

### npm: "You do not have permission to publish"

- Ensure you're logged in: `npm whoami`
- Check organization access: Contact @a3s-lab org admin
- Verify package name is available

### PyPI: "403 Forbidden"

- Check your API token is valid
- Ensure token has correct scope
- Verify package name is available

### "Version already exists"

- Bump the version number
- You cannot republish the same version

### Build Errors

```bash
# TypeScript: Clean and rebuild
cd sdk/typescript
rm -rf dist node_modules
npm install
npm run build

# Python: Clean and rebuild
cd sdk/python
rm -rf dist build *.egg-info
python -m build
```

## Post-Publishing

After successful publishing:

1. **Verify installation**:
   ```bash
   # npm
   npm install @a3s-lab/code

   # PyPI
   pip install a3s-code
   ```

2. **Create Git tag** (if not already done):
   ```bash
   git tag sdk-ts-v0.1.0
   git tag sdk-py-v0.1.0
   git push origin --tags
   ```

3. **Update documentation**:
   - Update README with new version
   - Update CHANGELOG
   - Announce on relevant channels

## Package URLs

- **TypeScript SDK**: https://www.npmjs.com/package/@a3s-lab/code
- **Python SDK**: https://pypi.org/project/a3s-code/

## Support

For issues with publishing:
- Check the [GitHub Actions logs](../../actions)
- Review this guide
- Contact the maintainers
