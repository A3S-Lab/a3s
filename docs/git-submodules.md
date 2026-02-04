# Understanding Git Submodules

This document explains what Git submodules are, why they're useful, and how A3S Box uses them.

## What Are Git Submodules?

Git submodules allow you to include one Git repository inside another as a subdirectory while keeping their histories separate. Think of it as a "pointer" to a specific commit in an external repository.

```
your-project/
├── src/
├── docs/
└── vendor/
    └── external-lib/     ← This is a submodule (separate Git repo)
        ├── .git          ← Has its own Git history
        └── src/
```

## Why Use Submodules?

### Problem: Managing External Dependencies

When your project depends on external code, you have several options:

| Approach | Pros | Cons |
|----------|------|------|
| **Copy code directly** | Simple, no extra tools | Hard to update, loses history |
| **Package manager** (npm, cargo) | Easy updates | Not all code is published |
| **Git submodule** | Version-pinned, full history | Slightly more complex workflow |

### When Submodules Make Sense

1. **Vendored dependencies**: Include a specific version of an external library
2. **Monorepo components**: Share code between related projects
3. **Fork management**: Track your fork while staying linked to upstream
4. **Build-time dependencies**: Include source code needed only for compilation

## How A3S Box Uses Submodules

A3S Box uses a submodule for `libkrun`, the MicroVM hypervisor library:

```
src/deps/libkrun-sys/
└── vendor/
    └── libkrun/          ← Submodule pointing to containers/libkrun
        ├── Cargo.toml
        ├── src/
        └── init/
```

### Why Submodule for libkrun?

1. **Not on crates.io**: libkrun isn't published as a Cargo crate
2. **Build from source**: We need to compile it with specific features
3. **Version pinning**: Lock to a known-working commit
4. **Cross-compilation**: The build process needs access to source files

### The .gitmodules File

```ini
# .gitmodules
[submodule "src/deps/libkrun-sys/vendor/libkrun"]
    path = src/deps/libkrun-sys/vendor/libkrun
    url = https://github.com/containers/libkrun.git
```

This file tells Git:
- **path**: Where to place the submodule in your project
- **url**: The remote repository to clone

## Common Commands

### Initial Setup (Clone with Submodules)

```bash
# Option 1: Clone and initialize in one command
git clone --recursive https://github.com/a3s-lab/box.git

# Option 2: Clone first, then initialize
git clone https://github.com/a3s-lab/box.git
cd box
git submodule update --init --recursive
```

### Update Submodule to Latest

```bash
# Update to the latest commit on the submodule's default branch
cd src/deps/libkrun-sys/vendor/libkrun
git fetch origin
git checkout main
git pull

# Go back to root and commit the update
cd ../../../../../
git add src/deps/libkrun-sys/vendor/libkrun
git commit -m "chore: update libkrun submodule"
```

### Check Submodule Status

```bash
# Show submodule status
git submodule status

# Example output:
#  a1b2c3d src/deps/libkrun-sys/vendor/libkrun (v1.17.0)
#  ^^^^^^^                                      ^^^^^^^^
#  commit hash                                  tag/description
```

### Add a New Submodule

```bash
git submodule add https://github.com/example/repo.git path/to/submodule
git commit -m "feat: add example submodule"
```

### Remove a Submodule

```bash
# 1. Remove from .gitmodules
git config -f .gitmodules --remove-section submodule.path/to/submodule

# 2. Remove from .git/config
git config --remove-section submodule.path/to/submodule

# 3. Remove the submodule directory
git rm --cached path/to/submodule
rm -rf path/to/submodule

# 4. Commit
git commit -m "chore: remove submodule"
```

## How Submodules Work Internally

### The Pointer Mechanism

When you add a submodule, Git stores:
1. **In `.gitmodules`**: The URL and path mapping
2. **In the tree**: A special entry pointing to a specific commit

```bash
# View what Git sees
git ls-tree HEAD src/deps/libkrun-sys/vendor/

# Output:
# 160000 commit a1b2c3d... libkrun
# ^^^^^^
# This is a "gitlink" - a pointer to a commit in another repo
```

### Detached HEAD State

Submodules are always checked out in "detached HEAD" state, pointing to a specific commit rather than a branch:

```
Main repo                    Submodule
─────────────────           ─────────────────
commit: abc123               commit: xyz789 ← detached HEAD
  └── submodule: xyz789      (not on any branch)
```

This ensures reproducibility: everyone gets the exact same code.

## Common Issues and Solutions

### Issue: Empty Submodule Directory

**Symptom**: The submodule folder exists but is empty.

**Solution**:
```bash
git submodule update --init --recursive
```

### Issue: Submodule Has Local Changes

**Symptom**: `git status` shows submodule has modifications.

**Solution**:
```bash
# Discard local changes in submodule
cd path/to/submodule
git checkout .

# Or update to the committed version
cd ..
git submodule update --force
```

### Issue: Wrong Submodule Version After Pull

**Symptom**: After `git pull`, submodule is at wrong commit.

**Solution**:
```bash
# Always run after pull
git submodule update --recursive
```

### Issue: Merge Conflicts in Submodule

**Symptom**: Conflict on submodule pointer during merge/rebase.

**Solution**:
```bash
# Check which commit you want
git log --oneline -3 -- path/to/submodule

# Set to desired commit
cd path/to/submodule
git checkout <desired-commit>
cd ..
git add path/to/submodule
git rebase --continue  # or git merge --continue
```

## Best Practices

### 1. Always Use `--recursive`

```bash
# When cloning
git clone --recursive <url>

# When updating
git submodule update --init --recursive
```

### 2. Pin to Tags When Possible

```bash
cd vendor/libkrun
git checkout v1.17.0  # Use a tag, not a branch
cd ..
git add vendor/libkrun
git commit -m "chore: pin libkrun to v1.17.0"
```

### 3. Document Submodule Purpose

Add a comment in your README or a dedicated doc explaining why each submodule exists.

### 4. Use CI to Verify Submodules

```yaml
# GitHub Actions example
- uses: actions/checkout@v4
  with:
    submodules: recursive
```

### 5. Consider Alternatives for Simple Cases

- **Single file**: Just copy it
- **Published package**: Use the package manager
- **Frequently updated**: Consider a monorepo instead

## Submodules vs Alternatives

| Feature | Submodule | Subtree | Monorepo |
|---------|-----------|---------|----------|
| Separate history | ✅ Yes | ❌ Merged | ❌ Shared |
| Easy updates | ⚠️ Manual | ⚠️ Manual | ✅ Automatic |
| Version pinning | ✅ Exact commit | ⚠️ At merge time | ❌ Always latest |
| Clone complexity | ⚠️ Extra step | ✅ Normal clone | ✅ Normal clone |
| Upstream contribution | ✅ Easy | ⚠️ Extract first | ❌ Fork needed |

## Quick Reference

```bash
# Setup
git clone --recursive <url>           # Clone with submodules
git submodule update --init --recursive  # Initialize after clone

# Daily use
git submodule status                  # Check versions
git submodule update --recursive      # Sync to committed versions

# Updates
cd submodule && git pull && cd ..     # Update submodule
git add submodule && git commit       # Commit the update

# Troubleshooting
git submodule update --force          # Reset to committed version
git submodule foreach git clean -fd   # Clean all submodules
```

## Further Reading

- [Git Documentation: Submodules](https://git-scm.com/book/en/v2/Git-Tools-Submodules)
- [GitHub: Working with Submodules](https://github.blog/2016-02-01-working-with-submodules/)
- [Atlassian: Git Submodules](https://www.atlassian.com/git/tutorials/git-submodule)
