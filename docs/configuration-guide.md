# A3S Box Configuration Guide

## Overview

This document explains how to configure A3S Box instances, including coding agent types, business agents, resource limits, and more.

## Quick Start

### Default Configuration (using A3S Code)

```python
from a3s_box import create_box

# Use default configuration: A3S Code as the coding agent
box = await create_box()

# Use the coding agent
await box.coding.generate("Write a Python function to calculate fibonacci")
```

### Specify Coding Agent Type

```python
from a3s_box import create_box, BoxConfig, AgentConfig

# Use OpenCode as the coding agent
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(kind="opencode")
))

# Use a custom coding agent
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(
        kind="oci_image",
        image="ghcr.io/myorg/my-coding-agent:v1"
    )
))
```

## Configuration Structure

### BoxConfig

```python
@dataclass
class BoxConfig:
    """A3S Box configuration"""

    # Box ID (optional, auto-generated if not provided)
    box_id: Optional[str] = None

    # Coding agent configuration (default: a3s-code)
    coding_agent: AgentConfig = field(default_factory=lambda: AgentConfig(kind="a3s_code"))

    # Skill mount list (business agents)
    skills: List[SkillMount] = field(default_factory=list)

    # Resource configuration
    resources: ResourceConfig = field(default_factory=ResourceConfig)

    # Network configuration
    network: NetworkConfig = field(default_factory=NetworkConfig)

    # Working directory
    workspace: str = "/workspace"
```

### AgentConfig

```python
@dataclass
class AgentConfig:
    """Agent configuration"""

    # Agent type
    kind: str  # "a3s_code", "opencode", "oci_image", "local_binary", "remote_binary"

    # Version (optional)
    version: Optional[str] = None

    # OCI image (required when kind="oci_image")
    image: Optional[str] = None

    # Local binary path (required when kind="local_binary")
    path: Optional[str] = None

    # Remote binary URL (required when kind="remote_binary")
    url: Optional[str] = None

    # Checksum (required when kind="remote_binary")
    checksum: Optional[str] = None

    # Custom entrypoint
    entrypoint: Optional[str] = None

    # Environment variables
    env: Dict[str, str] = field(default_factory=dict)

    # LLM configuration (supports object or file path)
    llm: Optional[Union[LLMConfig, str]] = None

    # Skills configuration directory (mounted to /a3s/skills/ in container)
    skills_dir: Optional[str] = None
```

### LLMConfig

```python
@dataclass
class LLMConfig:
    """LLM configuration"""

    # Default provider
    default_provider: str

    # Default model
    default_model: str

    # Provider list
    providers: List[ProviderConfig]

    @classmethod
    def from_file(cls, path: str) -> "LLMConfig":
        """Load configuration from file"""
        with open(path) as f:
            data = json.load(f)
        return cls.from_dict(data)

@dataclass
class ProviderConfig:
    """LLM provider configuration"""

    # Provider name
    name: str

    # API Key
    api_key: str

    # Base URL
    base_url: str

    # Model list
    models: List[ModelConfig]

@dataclass
class ModelConfig:
    """Model configuration"""

    # Model ID
    id: str

    # Model name
    name: str

    # Model family
    family: str

    # Supports attachments
    attachment: bool

    # Supports reasoning
    reasoning: bool

    # Supports tool calling
    tool_call: bool

    # Supports temperature parameter
    temperature: bool

    # Release date (optional)
    release_date: Optional[str] = None

    # Modality support
    modalities: Optional[dict] = None

    # Cost information (optional)
    cost: Optional[dict] = None

    # Limit information
    limit: Optional[dict] = None
```

### ResourceConfig

```python
@dataclass
class ResourceConfig:
    """Resource configuration"""

    # Memory limit
    memory: int = 2 * 1024 * 1024 * 1024  # 2GB

    # CPU cores
    cpus: int = 2

    # Disk limit
    disk: int = 10 * 1024 * 1024 * 1024  # 10GB
```

### NetworkConfig

```python
@dataclass
class NetworkConfig:
    """Network configuration"""

    # Enable external network access
    enable_external: bool = True
```

## Usage Examples

### Example 1: Default Configuration (A3S Code)

```python
from a3s_box import create_box

# Simplest way: use all defaults
box = await create_box()

# Equivalent to:
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(kind="a3s_code"),
    skills=[],
    resources=ResourceConfig(),
    network=NetworkConfig(),
))
```

### Example 2: Using OpenCode + LLM Config File

```python
from a3s_box import create_box, BoxConfig, AgentConfig

box = await create_box(BoxConfig(
    coding_agent=AgentConfig(
        kind="opencode",
        version="latest",
        # Load LLM configuration from file
        llm="/path/to/llm-config.json"
    )
))
```

**llm-config.json**:
```json
{
  "defaultProvider": "anthropic",
  "defaultModel": "claude-sonnet-4-20250514",
  "providers": [
    {
      "name": "anthropic",
      "apiKey": "${ANTHROPIC_API_KEY}",
      "baseUrl": "https://api.anthropic.com/v1",
      "models": [
        {
          "id": "claude-sonnet-4-20250514",
          "name": "Claude Sonnet 4",
          "family": "claude-sonnet",
          "attachment": true,
          "reasoning": false,
          "toolCall": true,
          "temperature": true,
          "modalities": {
            "input": ["text", "image", "pdf"],
            "output": ["text"]
          },
          "cost": {
            "input": 3,
            "output": 15,
            "cacheRead": 0.3,
            "cacheWrite": 3.75
          },
          "limit": {
            "context": 200000,
            "output": 64000
          }
        }
      ]
    }
  ]
}
```

### Example 3: Using Custom OCI Image

```python
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(
        kind="oci_image",
        image="ghcr.io/myorg/my-coding-agent:v1.0.0",
        entrypoint="exec /app/agent --port 4088",
        env={
            "RUST_LOG": "debug",
            "ANTHROPIC_API_KEY": os.getenv("ANTHROPIC_API_KEY"),
        }
    )
))
```

### Example 4: Skills Directory Mounting

```python
# Method 1: Mount entire directory via skills_dir
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(
        kind="a3s_code",
        llm="/path/to/llm-config.json",
        skills_dir="/path/to/skills"  # Entire directory mounted to /a3s/skills/
    )
))

# Host directory structure:
# /path/to/skills/
#   ├── order-agent/
#   │   └── SKILL.md
#   ├── data-agent/
#   │   └── SKILL.md
#   └── payment-agent/
#       └── SKILL.md

# Container auto-mounted as:
# /a3s/skills/
#   ├── order-agent/
#   ├── data-agent/
#   └── payment-agent/

# Method 2: Mount each skill individually
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(kind="a3s_code"),
    skills=[
        SkillMount(name="order-agent", path="/path/to/order-agent"),
        SkillMount(name="data-agent", path="/other/path/data-agent"),
    ]
))

# Method 3: Mixed usage (recommended)
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(
        kind="a3s_code",
        skills_dir="/path/to/common-skills"  # Common skills
    ),
    skills=[
        SkillMount(name="custom-agent", path="/path/to/custom-agent"),  # Extra skills
    ]
))
```

### Example 5: Custom Resource Limits

```python
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(kind="a3s_code"),
    resources=ResourceConfig(
        memory=4 * 1024 * 1024 * 1024,  # 4GB
        cpus=4,
        disk=20 * 1024 * 1024 * 1024,   # 20GB
    )
))
```

### Example 6: Load from YAML File

```python
import yaml
from a3s_box import create_box, BoxConfig

# Load configuration file
with open("box-config.yaml") as f:
    config_dict = yaml.safe_load(f)

# Create configuration object
config = BoxConfig.from_dict(config_dict)

# Create Box
box = await create_box(config)
```

**box-config.yaml**:
```yaml
box_id: "my-app-box"

coding_agent:
  kind: "a3s_code"
  version: "0.1.0"
  llm: "./config/llm-config.json"
  skills_dir: "./skills"

skills:
  - name: "custom-agent"
    path: "/path/to/custom-agent"
    auto_activate: true
    env:
      DATABASE_URL: "${DATABASE_URL}"

resources:
  memory: 2147483648  # 2GB
  cpus: 2
  disk: 10737418240   # 10GB

network:
  enable_external: true

workspace: "/workspace"
```

## TypeScript SDK Examples

### Default Configuration

```typescript
import { createBox } from '@a3s/box';

// Use default configuration
const box = await createBox();

// Use coding agent
await box.coding.generate('Write a TypeScript function');
```

### Specify Coding Agent

```typescript
import { createBox, BoxConfig, AgentConfig } from '@a3s/box';

// Use OpenCode
const box = await createBox({
  codingAgent: {
    kind: 'opencode',
    version: 'latest',
    llm: {
      provider: 'anthropic',
      model: 'claude-sonnet-4-20250514',
      apiKey: process.env.ANTHROPIC_API_KEY,
    },
  },
});
```

### Complete Configuration

```typescript
const box = await createBox({
  boxId: 'my-app-box',

  codingAgent: {
    kind: 'a3s_code',
    version: '0.1.0',
    llm: '/path/to/llm-config.json',  // LLM config file
    skillsDir: '/path/to/skills',     // Skills directory
  },

  skills: [
    {
      name: 'custom-agent',
      path: '/path/to/custom-agent',
      autoActivate: true,
    },
  ],

  resources: {
    memory: 2 * 1024 * 1024 * 1024,  // 2GB
    cpus: 2,
    disk: 10 * 1024 * 1024 * 1024,   // 10GB
  },

  network: {
    enableExternal: true,
  },

  workspace: '/workspace',
});
```

## Configuration Validation

A3S Box validates configuration at creation time:

```python
from a3s_box import create_box, BoxConfig, AgentConfig
from a3s_box.errors import ConfigValidationError

try:
    box = await create_box(BoxConfig(
        coding_agent=AgentConfig(
            kind="oci_image",
            # Error: missing image parameter
        )
    ))
except ConfigValidationError as e:
    print(f"Configuration error: {e}")
    # Output: Configuration error: AgentConfig with kind='oci_image' requires 'image' parameter
```

## Best Practices

### 1. Use Environment Variables for Sensitive Information

```python
# Good practice
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(
        kind="a3s_code",
        llm=LLMConfig(
            provider="anthropic",
            api_key=os.getenv("ANTHROPIC_API_KEY"),  # Read from environment variable
        )
    )
))

# Bad practice
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(
        kind="a3s_code",
        llm=LLMConfig(
            provider="anthropic",
            api_key="sk-ant-...",  # Hardcoded API key
        )
    )
))
```

### 2. Use Configuration Files for Complex Configurations

```python
# Good practice: Use YAML configuration file
config = BoxConfig.from_yaml("box-config.yaml")
box = await create_box(config)

# Bad practice: Hardcode lots of configuration in code
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(...),
    skills=[...],
    resources=ResourceConfig(...),
    # ... lots of configuration
))
```

### 3. Use Different Configurations for Different Environments

```python
import os

env = os.getenv("ENV", "development")

if env == "production":
    config = BoxConfig.from_yaml("box-config.prod.yaml")
elif env == "staging":
    config = BoxConfig.from_yaml("box-config.staging.yaml")
else:
    config = BoxConfig.from_yaml("box-config.dev.yaml")

box = await create_box(config)
```

## Configuration Reference

### Coding Agent Types

| Type | Description | Required Parameters |
|------|-------------|---------------------|
| `a3s_code` | A3S Code (default) | None |
| `opencode` | OpenCode | None |
| `oci_image` | OCI Image | `image` |
| `local_binary` | Local Binary | `path` |
| `remote_binary` | Remote Binary | `url`, `checksum` |

### LLM Providers

| Provider | Supported Models |
|----------|-----------------|
| `anthropic` | claude-sonnet-4-20250514, claude-opus-4, claude-3-haiku-20240307 |
| `openai` | gpt-4, gpt-4-turbo, gpt-3.5-turbo |
| `google` | gemini-pro, gemini-ultra |
| `local` | Local models (requires base_url configuration) |

### Resource Limits

| Parameter | Default (Coding) | Default (Business) | Description |
|-----------|-----------------|-------------------|-------------|
| `memory` | 2GB | 1GB | Memory limit |
| `cpus` | 2 | 1 | CPU cores |
| `disk` | 10GB | 5GB | Disk limit |

## Troubleshooting

### Problem 1: Coding Agent Startup Failure

```
Error: Failed to start coding agent: connection timeout
```

**Solution**:
1. Check if the agent image exists
2. Check network connection
3. Increase startup timeout

```python
box = await create_box(BoxConfig(
    coding_agent=AgentConfig(kind="a3s_code"),
    startup_timeout=60,  # Increase to 60 seconds
))
```

### Problem 2: Invalid API Key

```
Error: Invalid API key for provider 'anthropic'
```

**Solution**:
1. Check if environment variable is set
2. Verify API key is valid

```bash
# Check environment variable
echo $ANTHROPIC_API_KEY

# Set environment variable
export ANTHROPIC_API_KEY="sk-ant-..."
```

### Problem 3: Insufficient Resources

```
Error: Failed to allocate resources: insufficient memory
```

**Solution**:
1. Reduce resource limits
2. Increase host resources

```python
box = await create_box(BoxConfig(
    resources=ResourceConfig(
        memory=1 * 1024 * 1024 * 1024,  # Reduce to 1GB
        cpus=1,
    )
))
```

---

**Version**: 1.0.0
**Last Updated**: 2026-02-04
