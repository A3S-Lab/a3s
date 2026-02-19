# Rava — Product Requirements & Technical Architecture

> Version: 1.1 | Date: 2026-02-19 | Author: A3S Team

---

# Part I — Product Requirements

## 1. Overview

### 1.1 Problem Statement

The Java ecosystem is powerful but unwieldy. Developers face:

- **Slow startup**: JVM cold-start takes seconds — unsuitable for CLI tools, serverless, and edge computing
- **High memory footprint**: Even a Hello World consumes 100 MB+
- **Fragmented toolchain**: Maven / Gradle / Ant each do things their own way, with verbose config (XML / Groovy / Kotlin DSL)
- **Painful dependency management**: `pom.xml` files balloon to hundreds of lines; dependency conflicts are a daily headache
- **Complex deployment**: Requires a pre-installed JRE/JDK; Docker images are bloated (200 MB+)
- **Outdated developer experience**: Compared to what Bun/Deno deliver for TypeScript, the Java toolchain feels like the previous decade

### 1.2 Product Vision

**Rava** is a Java AOT compiler and all-in-one toolchain written in Rust. It is to Java what Bun is to TypeScript.

```
Bun : TypeScript = Rava : Java

One binary. Everything included:
  rava run Main.java          → run Java source directly
  rava build                  → AOT-compile to a native binary
  rava init                   → initialize a project (generates rava.hcl)
  rava add spring-boot-web    → add a dependency
  rava test                   → run tests
  rava fmt                    → format code
```

### 1.3 Core Principles

**Principle 1: Zero-configuration out of the box.**

```bash
# No JDK installation, no pom.xml, no build.gradle needed
# One file, just run it
echo 'class Main { public static void main(String[] args) { System.out.println("Hello"); } }' > Main.java
rava run Main.java
# → Hello
```

**Principle 2: One binary replaces the entire toolchain.**

| Traditional Java | Rava |
|-----------------|------|
| JDK (javac) | `rava` |
| Maven / Gradle | `rava` |
| GraalVM native-image | `rava build` |
| JUnit + Maven Surefire | `rava test` |
| google-java-format | `rava fmt` |
| jlink / jpackage | `rava build --bundle` |

**Principle 3: HCL replaces XML/Groovy — human-readable project config.**

```hcl
# rava.hcl — project configuration (analogous to package.json / Cargo.toml)
project {
  name    = "my-api"
  version = "0.1.0"
  java    = "21"

  authors = ["[name] <[email]>"]
  license = "MIT"
}

dependencies {
  "org.springframework.boot:spring-boot-starter-web" = "3.2.0"
  "com.google.guava:guava"                           = "33.0.0-jre"
}

dev_dependencies {
  "org.junit.jupiter:junit-jupiter" = "5.10.1"
  "org.mockito:mockito-core"        = "5.8.0"
}

build {
  target   = "native"       # native | jar | jlink
  main     = "com.example.Main"
  optimize = "speed"        # speed | size | debug
}

run {
  args = ["--server.port=8080"]
  env  = {
    SPRING_PROFILES_ACTIVE = "dev"
  }
}
```

**Principle 4: AOT-first, JIT optional.**

```
Traditional Java:  .java → javac → .class → JVM (JIT) → machine code
Rava:              .java → rava  → native binary (AOT)  → direct execution

Startup time:  traditional ~2s   →  Rava ~10ms
Memory usage:  traditional ~200MB →  Rava ~20MB
```

### 1.4 Target Users

| User Group | Pain Point | Rava Value |
|------------|-----------|------------|
| Java backend developers | Complex toolchain, slow startup | One-command run, instant startup |
| Cloud-native / serverless developers | JVM cold-start unsuitable for Lambda | AOT compilation, 10 ms startup |
| CLI tool developers | Java is awkward for CLI | Compile to single-file native binary |
| Microservice developers | Bloated Docker images | Compile to scratch image (<20 MB) |
| Java beginners | Complex environment setup | Install rava and start coding immediately |

---

## 2. Core Capabilities

### 2.1 Capability Overview

| # | Capability | Command | Description |
|---|-----------|---------|-------------|
| 1 | Direct run | `rava run` | Run .java source directly, no pre-compilation |
| 2 | AOT compile | `rava build` | Compile to native binary or optimized JAR |
| 3 | Project management | `rava init` | Initialize project, generate rava.hcl |
| 4 | Dependency management | `rava add/remove/update` | Manage Maven dependencies, auto-resolve transitive deps |
| 5 | Test runner | `rava test` | Run JUnit tests |
| 6 | Code formatter | `rava fmt` | Format Java code |
| 7 | Linter | `rava lint` | Static analysis and code quality checks |
| 8 | REPL | `rava repl` | Interactive Java REPL |
| 9 | Script mode | `rava script` | Single-file script execution (auto-infers main) |
| 10 | Publish | `rava publish` | Publish to Maven Central or private registry |

### 2.2 Capability 1: Direct Run (`rava run`)

Run Java like a script, no manual compilation:

```bash
# Single-file run
rava run Main.java

# Project run (reads main class from rava.hcl)
rava run

# Run with arguments
rava run Main.java -- --port 8080

# Watch mode: restart on file changes (development)
rava run --watch
```

**Run strategies:**

| Scenario | Strategy | Notes |
|----------|----------|-------|
| Single file, no dependencies | Direct AOT compile-and-run | Fastest path, ~100 ms startup |
| Single file with imports | Auto-resolve stdlib + rava.hcl deps | Transparent dependency injection |
| Project directory (rava.hcl present) | Incremental compile + run | Recompiles only changed files |
| `--watch` mode | File watching + hot reload | Developer experience first |
| `--jit` flag | Fall back to JIT mode | Compatibility first (when specific JVM features are needed) |

**Script mode** (no `public static void main` required):

```java
// script.java — Rava auto-wraps this as an executable entry point
var name = "World";
System.out.println("Hello, " + name + "!");

// Top-level imports auto-available
import java.util.*;
import java.io.*;
var list = List.of(1, 2, 3);
list.forEach(System.out::println);
```

```bash
rava run script.java
# → Hello, World!
# → 1
# → 2
# → 3
```

### 2.3 Capability 2: AOT Compilation (`rava build`)

Compile Java source to a native binary that runs without a JVM:

```bash
# Compile to native binary (default)
rava build
# → target/my-api  (native executable, ~15 MB)

# Compile to optimized JAR
rava build --target jar
# → target/my-api.jar

# Compile to jlink slim runtime
rava build --target jlink
# → target/my-api/  (slim JRE included, ~40 MB)

# Cross-compilation
rava build --platform linux-amd64
rava build --platform linux-arm64
rava build --platform macos-amd64
rava build --platform windows-amd64

# Optimization options
rava build --optimize speed   # optimize for execution speed (default)
rava build --optimize size    # optimize for binary size
rava build --optimize debug   # retain debug information
```

**Output comparison:**

| Target | Output | Size | Startup | Requires JVM |
|--------|--------|------|---------|-------------|
| `native` (default) | Single-file binary | ~15 MB | ~10 ms | No |
| `jar` | Executable JAR | ~5 MB + deps | ~2 s | Yes |
| `jlink` | Slim JRE + JAR | ~40 MB | ~1 s | No (bundled) |
| `docker` | Scratch container image | ~20 MB | ~10 ms | No |

**AOT compilation pipeline:**

```
.java source
  → Rava frontend (parse → AST → type check → semantic analysis)
  → Rava Intermediate Representation (RIR)
  → Optimization passes (escape analysis, inlining, dead code elimination, constant folding)
  → Backend code generation (LLVM / Cranelift)
  → Linking (static-link stdlib + dependencies)
  → Native binary
```

### 2.4 Capability 3: Project Management (`rava init`)

```bash
# Initialize empty project
rava init my-project
# → my-project/
#   ├── rava.hcl
#   ├── src/
#   │   └── Main.java
#   └── test/
#       └── MainTest.java

# Initialize from template
rava init my-api --template spring-web
rava init my-cli --template cli
rava init my-lib --template library

# Migrate from existing Maven/Gradle project
rava init --from-maven     # reads pom.xml, generates rava.hcl
rava init --from-gradle    # reads build.gradle, generates rava.hcl
```

**Project layout:**

```
my-project/
├── rava.hcl                 # project config (the only config file)
├── rava.lock                # dependency lockfile (auto-generated)
├── src/                     # source directory
│   └── com/example/
│       └── Main.java
├── test/                    # test directory
│   └── com/example/
│       └── MainTest.java
├── native/                  # native library bindings (.a, .so, .h) — optional
│   ├── libsqlite3.a
│   └── jni_bridge.c
├── resources/               # resource files
│   └── application.properties
└── target/                  # build output (auto-generated)
    ├── cache/               # incremental compilation cache
    └── my-project           # compiled output
```

### 2.5 Capability 4: Dependency Management (`rava add/remove/update`)

```bash
# Add dependency
rava add spring-boot-starter-web
rava add com.google.guava:guava@33.0.0-jre
rava add lombok --dev

# Remove dependency
rava remove guava

# Update dependencies
rava update                  # update all deps to latest compatible version
rava update guava            # update specific dependency
rava update --latest         # update to absolute latest (may have breaking changes)

# View dependency tree
rava deps tree
# → com.example:my-api@0.1.0
#   ├── org.springframework.boot:spring-boot-starter-web@3.2.0
#   │   ├── org.springframework.boot:spring-boot-starter@3.2.0
#   │   ├── org.springframework.boot:spring-boot-starter-json@3.2.0
#   │   └── org.springframework.boot:spring-boot-starter-tomcat@3.2.0
#   └── com.google.guava:guava@33.0.0-jre

# Check outdated dependencies
rava deps outdated

# Audit for security vulnerabilities
rava deps audit
```

**Dependency resolution:**

| Feature | Description |
|---------|-------------|
| Registries | Maven Central (default), custom private registries |
| Version ranges | Semantic versioning (`^3.2.0`, `~3.2.0`, `>=3.2.0,<4.0.0`) |
| Transitive deps | Auto-resolved; highest compatible version wins on conflict |
| Lockfile | `rava.lock` guarantees reproducible builds |
| Local cache | `~/.rava/cache/` global cache, shared across projects |
| Offline mode | `rava add --offline` uses only local cache |
| Short names | Common deps have aliases (`spring-boot-web` → `org.springframework.boot:spring-boot-starter-web`) |

**rava.hcl dependency config:**

```hcl
dependencies {
  # Standard format
  "org.springframework.boot:spring-boot-starter-web" = "3.2.0"

  # Version range
  "com.google.guava:guava" = "^33.0.0-jre"

  # Detailed config
  "io.netty:netty-all" = {
    version  = "4.1.100.Final"
    exclude  = ["io.netty:netty-transport-native-epoll"]
  }

  # Local path dependency (monorepo)
  "my-common" = { path = "../common" }

  # Git dependency
  "my-lib" = {
    git    = "https://github.com/org/my-lib.git"
    branch = "main"
  }
}
```

**Built-in short name aliases:**

| Short name | Full coordinates |
|-----------|-----------------|
| `spring-boot-web` | `org.springframework.boot:spring-boot-starter-web` |
| `spring-boot-data-jpa` | `org.springframework.boot:spring-boot-starter-data-jpa` |
| `lombok` | `org.projectlombok:lombok` |
| `guava` | `com.google.guava:guava` |
| `jackson` | `com.fasterxml.jackson.core:jackson-databind` |
| `slf4j` | `org.slf4j:slf4j-api` |
| `logback` | `ch.qos.logback:logback-classic` |
| `junit` | `org.junit.jupiter:junit-jupiter` |
| `mockito` | `org.mockito:mockito-core` |
| `assertj` | `org.assertj:assertj-core` |

The short-name registry is viewable via `rava alias list` and supports user-defined extensions.

### 2.6 Capability 5: Test Runner (`rava test`)

```bash
# Run all tests
rava test

# Run a specific test class
rava test MainTest

# Run a specific test method
rava test MainTest::testHello

# Run tests matching a pattern
rava test --filter "*.service.*"

# Watch mode (auto re-run on file changes)
rava test --watch

# Generate coverage report
rava test --coverage
# → Coverage: 85.3% (lines), 78.2% (branches)
# → Report: target/coverage/index.html
```

**Supported test frameworks:**

| Framework | Support | Notes |
|-----------|---------|-------|
| JUnit 5 | Native | Default test framework |
| JUnit 4 | Compatible | Auto-detected and adapted |
| TestNG | Plugin | Configured via rava.hcl |
| Assertion libraries | Transparent | AssertJ, Hamcrest, etc. work directly |

### 2.7 Capabilities 6–7: Format & Lint (`rava fmt` / `rava lint`)

```bash
# Format
rava fmt                     # format all .java files
rava fmt --check             # check only, no changes (for CI)
rava fmt src/Main.java       # format a specific file

# Lint
rava lint                    # run all lint rules
rava lint --fix              # auto-fix fixable issues
```

### 2.8 Capability 8: Interactive REPL (`rava repl`)

```bash
rava repl
# → Rava REPL v0.1.0 (Java 21)
# → Type :help for help, :quit to exit
#
# rava> var x = 42;
# rava> System.out.println(x * 2);
# 84
# rava> import java.util.stream.*;
# rava> IntStream.range(0, 5).map(i -> i * i).toArray()
# [0, 1, 4, 9, 16]
# rava> :quit
```

### 2.9 Capabilities 9–10: Publish (`rava publish`)

```bash
# Publish to Maven Central
rava publish

# Publish to private registry
rava publish --registry company

# Dry run (no actual publish)
rava publish --dry-run
```

---

## 3. rava.hcl Reference

### 3.1 Top-Level Structure

```hcl
# rava.hcl — Rava project configuration file

project {
  # Project metadata
}

dependencies {
  # Runtime dependencies
}

dev_dependencies {
  # Development/test dependencies
}

repositories {
  # Maven repository sources
}

build {
  # Compilation config
}

run {
  # Run config
}

test {
  # Test config
}

publish {
  # Publish config
}
```

### 3.2 `project` Block

```hcl
project {
  name        = "my-api"                    # project name (required)
  group       = "com.example"               # group ID (required for publishing)
  version     = "0.1.0"                     # semantic version (required)
  java        = "21"                        # Java version (required)
  description = "My awesome API"            # project description
  license     = "MIT"                       # license
  homepage    = "https://github.com/..."    # project homepage
  repository  = "https://github.com/..."    # source repository

  authors = [
    "[name] <[email]>"
  ]
}
```

### 3.3 `dependencies` / `dev_dependencies` Blocks

```hcl
dependencies {
  # Short name + version
  "spring-boot-web" = "3.2.0"

  # Standard format: groupId:artifactId = version
  "com.google.guava:guava" = "33.0.0-jre"

  # Version ranges
  "org.slf4j:slf4j-api" = "^2.0.0"        # >=2.0.0, <3.0.0
  "io.netty:netty-all"  = "~4.1.100"      # >=4.1.100, <4.2.0

  # Detailed config
  "com.fasterxml.jackson.core:jackson-databind" = {
    version = "2.16.0"
    exclude = [
      "com.fasterxml.jackson.core:jackson-annotations"
    ]
  }

  # Local path dependency (monorepo)
  "my-common" = { path = "../common" }

  # Git dependency
  "my-lib" = {
    git    = "https://github.com/org/my-lib.git"
    branch = "main"
  }
}
```

### 3.4 `build` Block

```hcl
build {
  target   = "native"              # native | jar | jlink | docker
  main     = "com.example.Main"    # main class (auto-detected, overridable)
  optimize = "speed"               # speed | size | debug

  # AOT compilation options
  aot {
    reflection_config = "reflect-config.json"   # reflection config (optional)
    initialize_at_build_time = [                 # classes to initialize at build time
      "org.slf4j"
    ]
    enable_preview = true                        # enable preview features
  }

  # Docker build options (when target = "docker")
  docker {
    base_image = "scratch"          # base image
    tag        = "my-api:latest"
    expose     = [8080]
    labels     = {
      maintainer = "[name]"
    }
  }

  # JNI native library config
  jni {
    link      = ["sqlite3", "z", "crypto"]    # link these native libraries
    lib_paths = ["native/", "/usr/local/lib"] # search paths
    static    = ["sqlite3"]                   # force static link (rest are dynamic)
    # Dynamic libs are loaded at runtime via System.loadLibrary() / dlopen
  }

  # Cross-compilation targets
  platforms = ["linux-amd64", "linux-arm64", "macos-amd64"]
}
```

### 3.5 `run` Block

```hcl
run {
  main = "com.example.Main"        # main class (overrides build.main)
  args = ["--server.port=8080"]    # program arguments

  env = {
    SPRING_PROFILES_ACTIVE = "dev"
    DATABASE_URL           = "jdbc:postgresql://localhost:5432/mydb"
  }

  # Development watch config
  watch {
    paths   = ["src/", "resources/"]
    exclude = ["*.class", "target/"]
    delay   = "500ms"
  }

  # JIT fallback config (used with rava run --jit)
  jvm {
    heap_min = "256m"
    heap_max = "1g"
    options  = ["-XX:+UseG1GC"]
  }
}
```

### 3.6 `test` Block

```hcl
test {
  framework = "junit5"             # junit5 | junit4 | testng
  parallel  = true                 # run tests in parallel
  timeout   = "60s"                # per-test timeout

  coverage {
    enabled    = true
    min_line   = 80                # minimum line coverage (%)
    min_branch = 70                # minimum branch coverage (%)
    exclude    = ["**/generated/**"]
  }
}
```

### 3.7 `publish` Block

```hcl
publish {
  registry = "maven_central"       # target registry
  sign     = true                  # GPG signing

  pom {
    # Extra POM info (required for Maven Central)
    scm {
      url        = "https://github.com/org/repo"
      connection = "scm:git:git://github.com/org/repo.git"
    }
    developers = [
      {
        id    = "dev1"
        name  = "[name]"
        email = "[email]"
      }
    ]
  }
}
```

---

## 4. Java Compatibility

### 4.1 Language Version Support

| Java Version | Support Level | Notes |
|-------------|--------------|-------|
| Java 21 (LTS) | Full | Primary target — all features |
| Java 17 (LTS) | Full | All features |
| Java 11 (LTS) | Full | All features |
| Java 8+ | Full | Lambda, generics, annotation processing, all features |

### 4.2 Language Feature Support

| Feature | AOT Support | Notes |
|---------|-------------|-------|
| Records | ✅ | Java 16+ |
| Sealed Classes | ✅ | Java 17+ |
| Pattern Matching | ✅ | Java 21+ |
| Virtual Threads | ✅ | Java 21+, natively supported in AOT |
| Text Blocks | ✅ | Java 15+ |
| Switch Expressions | ✅ | Java 14+ |
| `var` local variables | ✅ | Java 10+ |
| Lambda / Stream | ✅ | Java 8+ |
| Generics | ✅ | Type erasure handled at compile time |
| Annotation processing | ✅ | Compile-time execution (Lombok, etc.) |
| Reflection (statically resolvable) | ✅ Full | AOT metadata table, zero config, faster than JVM reflection |
| Reflection (dynamically unresolvable) | ✅ Full | MicroRT metadata engine, auto-fallback, no reflect-config needed |
| Dynamic proxy (compile-time known interfaces) | ✅ Full | AOT pre-generated proxy classes, zero runtime overhead |
| Dynamic proxy (runtime interfaces) | ✅ Full | MicroRT runtime generation, auto-fallback |
| Dynamic class loading | ✅ Full | Embedded MicroRT bytecode runtime, no JVM required |
| JNI — Outbound (Java → C) | ✅ Full | Phase 1: AOT generates native method stubs, `System.loadLibrary()` via dlopen, `.a` static linking via `rava.hcl` |
| JNI — Inbound (C → JNIEnv\*) | ✅ Full | Phase 3: MicroRT provides full JNI function table (~230 functions), `JavaVM*` singleton, `JNI_OnLoad`/`JNI_OnUnload` lifecycle |

### 4.3 Framework Compatibility

| Framework | Compatibility | Notes |
|-----------|--------------|-------|
| Spring Boot 3.x | Tier 1 | Reflection/proxy config handled automatically |
| Quarkus | Tier 1 | AOT-friendly by design |
| Micronaut | Tier 1 | Compile-time DI, naturally suited to AOT |
| Vert.x | Tier 1 | No reflection dependency |
| Jakarta EE | Tier 2 | Some features require configuration |
| Hibernate | Tier 2 | Requires reflection config |
| MyBatis | Tier 2 | Requires reflection config |
| Lombok | Tier 1 | Compile-time annotation processing, fully compatible |

### 4.4 Reflection Auto-Detection

The biggest challenge with AOT compilation is reflection. Rava handles it automatically through multiple layers:

```
1. Static analysis:    scan source for Class.forName(), .getMethod(), etc.
2. Framework adapters: built-in reflection rules for Spring/Hibernate/Jackson
3. Annotation scan:    @Component, @Entity, @JsonProperty, etc. auto-registered
4. Runtime tracing:    rava run --trace-reflection records actual reflection calls
5. Manual override:    reflect-config.json as a last resort
```

### 4.5 Interoperability with Existing Projects

```bash
# Migrate from Maven
cd existing-maven-project
rava init --from-maven
# → reads pom.xml, generates rava.hcl
# → preserves src/main/java and src/test/java layout
# → optionally migrates to flat src/ and test/ layout

# Migrate from Gradle
rava init --from-gradle
# → reads build.gradle(.kts), generates rava.hcl

# Export pom.xml (reverse compatibility)
rava export maven
# → generates pom.xml for use in traditional Maven environments

# Mixed use: Rava manages deps, Maven/Gradle builds
rava export maven --sync
# → continuously syncs rava.hcl → pom.xml
```

---

## 5. CLI Reference

### 5.1 Command Overview

```
rava <command> [options] [args]

Project management:
  init [name]              initialize a new project
  run [file]               run Java source or project
  build                    compile project
  test [pattern]           run tests
  fmt [files...]           format code
  lint [files...]          lint code
  repl                     interactive REPL
  clean                    clean build artifacts

Dependency management:
  add <package>            add dependency
  remove <package>         remove dependency
  update [package]         update dependency
  deps tree                show dependency tree
  deps outdated            check for outdated deps
  deps audit               security vulnerability audit
  alias list               view short-name mappings

Publish:
  publish                  publish to registry
  export maven             export pom.xml

Tools:
  upgrade                  upgrade Rava itself
  doctor                   diagnose environment issues
  config                   manage global config
  completions <shell>      generate shell completion scripts
```

### 5.2 Global Options

| Option | Description |
|--------|-------------|
| `--verbose` / `-v` | Verbose output |
| `--quiet` / `-q` | Silent mode |
| `--color <auto\|always\|never>` | Color output |
| `--help` / `-h` | Help |
| `--version` / `-V` | Version |

### 5.3 Key Command Reference

**`rava run`:**

| Option | Description |
|--------|-------------|
| `--watch` / `-w` | Watch for file changes, auto-restart |
| `--jit` | Run in JIT mode (fall back to JVM) |
| `--release` | Run with release optimization level |
| `--env <KEY=VALUE>` | Set environment variables |
| `-- <args>` | Arguments passed to the program |

**`rava build`:**

| Option | Description |
|--------|-------------|
| `--target <native\|jar\|jlink\|docker>` | Compile target, default `native` |
| `--optimize <speed\|size\|debug>` | Optimization strategy, default `speed` |
| `--platform <target>` | Cross-compilation target platform |
| `--output` / `-o` | Output path |
| `--static` | Fully static linking (Linux musl) |

**`rava add`:**

| Option | Description |
|--------|-------------|
| `--dev` / `-D` | Add as dev dependency |
| `--exact` | Use exact version (no `^`) |
| `--offline` | Use local cache only |

---

## 6. Global Configuration

### 6.1 Config File Locations

```
~/.rava/
├── config.hcl              # global config
├── cache/                   # dependency cache
│   └── repository/          # Maven repository cache
├── toolchains/              # JDK toolchains (for JIT fallback)
└── aliases.hcl              # user-defined short names
```

### 6.2 Global Config (`~/.rava/config.hcl`)

```hcl
# Default Java version
default_java = "21"

# Default compile target
default_target = "native"

# Proxy config
proxy {
  http     = "http://proxy.company.internal:8080"
  https    = "http://proxy.company.internal:8080"
  no_proxy = ["localhost", "*.internal"]
}

# Mirror registry (for faster downloads)
mirror {
  maven_central = "https://maven.aliyun.com/repository/central"
}

# Telemetry (can be disabled)
telemetry = false
```

---

## 7. Non-Functional Requirements

### 7.1 Performance Targets

| Metric | Target | Comparison |
|--------|--------|------------|
| Single-file run startup | < 200 ms | javac + java: ~2 s |
| AOT-compiled startup | < 20 ms | JVM: ~2 s, GraalVM native: ~50 ms |
| Incremental build speed | < 1 s (single file change) | Maven: ~5 s |
| Full build (mid-size project) | < 30 s | Maven: ~60 s |
| Memory (AOT artifact) | < 50 MB (mid-size web app) | JVM: ~200 MB |
| Binary size | < 30 MB (mid-size web app) | GraalVM: ~60 MB |
| Dependency resolution | < 2 s (cached) | Maven: ~10 s |
| Rava binary size | < 50 MB | GraalVM: ~400 MB |

### 7.2 Reliability

| Metric | Target |
|--------|--------|
| Compilation correctness | Pass Java TCK core subset |
| Dependency resolution consistency | rava.lock guarantees 100% reproducible builds |
| Crash recovery | Interrupted builds do not corrupt cache |
| Error messages | Every error includes filename, line number, and fix suggestion |

### 7.3 Platform Support

| Platform | Support Level |
|----------|-------------|
| Linux x86_64 | Tier 1 (CI-tested) |
| Linux aarch64 | Tier 1 (CI-tested) |
| macOS x86_64 | Tier 1 (CI-tested) |
| macOS aarch64 (Apple Silicon) | Tier 1 (CI-tested) |
| Windows x86_64 | Tier 2 (community-tested) |

### 7.4 Installation

```bash
# macOS / Linux (recommended)
curl -fsSL https://rava.dev/install.sh | sh

# Homebrew
brew install a3s-lab/tap/rava

# Cargo (build from source)
cargo install rava

# Version management
rava upgrade              # upgrade to latest release
rava upgrade --canary     # upgrade to canary build
```

---

## 8. Comparison with Bun

| Dimension | Bun (TypeScript) | Rava (Java) |
|-----------|-----------------|-------------|
| Implementation language | Zig + C++ | Rust |
| Replaces | Node.js + npm + webpack | JDK + Maven/Gradle + GraalVM |
| Run | `bun run index.ts` | `rava run Main.java` |
| Build | `bun build` | `rava build` |
| Package management | `bun add express` | `rava add spring-boot-web` |
| Config | `package.json` | `rava.hcl` |
| Lockfile | `bun.lockb` | `rava.lock` |
| Test | `bun test` | `rava test` |
| REPL | `bun repl` | `rava repl` |
| Package registry | npm | Maven Central |
| Core advantage | Blazing-fast JS/TS runtime | Blazing-fast Java AOT compiler + runtime |

---

## 9. Glossary

| Term | Description |
|------|-------------|
| AOT | Ahead-of-Time compilation — compiling source to native machine code before execution |
| JIT | Just-in-Time compilation — compiling bytecode to machine code at runtime (traditional JVM approach) |
| RIR | Rava Intermediate Representation — Rava's internal IR |
| rava.hcl | Rava project config file, analogous to package.json / Cargo.toml |
| rava.lock | Dependency lockfile guaranteeing reproducible builds |
| Short name | Alias for a common Maven dependency (e.g. `guava` → `com.google.guava:guava`) |
| Reflection config | Reflection metadata for AOT compilation; Rava auto-detects with manual override available |
| TCK | Technology Compatibility Kit — Java compatibility test suite |
| LLVM | Compiler backend framework used for native machine code generation |
| Cranelift | Lightweight code-generation backend in the Rust ecosystem; faster compile times |
| MicroRT | Rava's embedded bytecode runtime — handles the 5% of code AOT cannot statically resolve |
| UnifiedHeap | Shared memory heap used by both AOT-compiled objects and MicroRT-interpreted objects |

---

# Part II — Technical Architecture

## 10. Approach: AOT + Embedded Bytecode Runtime

### 10.1 Executive Summary

**All dynamic Java features can be implemented — but not the GraalVM way.**

GraalVM's approach is "closed-world assumption + config fallback": every class must be known at compile time; anything unknown causes an error, and users must configure it manually. That is not solving the problem — it is avoiding it.

Rava's approach: **AOT compilation as the primary path + embedded lightweight bytecode runtime as the escape hatch**. Everything resolvable at compile time is AOT-compiled. Everything that cannot be resolved at compile time automatically falls back to the embedded runtime. Users never observe this transition.

```
GraalVM:  AOT compile → hit reflection → error → user writes reflect-config.json → recompile
Rava:     AOT compile → hit reflection → auto-mark → embedded runtime handles it → transparent to user
```

### 10.2 Why GraalVM's Approach Falls Short

GraalVM native-image is built on the [Closed-World Assumption](https://www.marcobehler.com/guides/graalvm-aot-jit):

> All classes, methods, and fields reachable by the program must be known at compile time. No class unseen at compile time may appear at runtime.

This assumption directly causes:

| Feature | GraalVM's handling | Problem |
|---------|-------------------|---------|
| Reflection | Requires [reflect-config.json](https://www.graalvm.org/22.1/reference-manual/native-image/Reflection/index.html) | Manual maintenance; framework upgrades can break it |
| Dynamic proxy | Requires [proxy-config.json](https://www.graalvm.org/latest/reference-manual/native-image/dynamic-features/DynamicProxy/) | Interface combinations must be declared upfront |
| Dynamic class loading | **Not supported** | `Class.forName()` can only find compile-time-known classes |
| Runtime bytecode generation | **Not supported** | [ByteBuddy/CGLIB require special adaptation](https://github.com/raphw/byte-buddy/issues/1588) |

**Why the config-file approach is a dead end:**

```
Typical reflection call chain in a Spring Boot project:

@RestController → Spring scan → reflection creates Bean
@Autowired      → reflection injects dependencies
@RequestBody    → Jackson reflection serialization/deserialization
@Transactional  → CGLIB dynamic proxy
JPA @Entity     → Hibernate reflection + dynamic proxy

A mid-size Spring Boot project's reflect-config.json can have 2,000+ entries.
Every dependency upgrade can invalidate the config.
This is not "limited support" — it is a maintenance nightmare.
```

**The Dart/Flutter lesson:**

[Dart removed dart:mirrors entirely](https://github.com/flutter/flutter/issues/1150) because reflection is incompatible with AOT + tree shaking. The Flutter ecosystem was forced to abandon dynamic capabilities and switch to compile-time code generation (`json_serializable`, `freezed`, etc.).

This is one "solution," but its cost is forcing the entire ecosystem to rewrite. Rava must not take this path — the core value of the Java ecosystem lies precisely in its dynamic capabilities. Removing reflection means removing Spring, Hibernate, and MyBatis.

---

## 11. First-Principles Analysis

### 11.1 What Do These Features Actually Do?

From the CPU's perspective, whether AOT or JIT, the end result is always machine code. The only difference is: **when is the machine code generated?**

| Feature | Essence | Required capability |
|---------|---------|-------------------|
| Reflection | Look up a class/method/field by string name at runtime and invoke it | Runtime metadata query + method dispatch |
| Dynamic proxy | Generate a new class at runtime that implements given interfaces and intercepts method calls | Runtime code generation + method interception |
| Dynamic class loading | Load and execute .class bytecode unknown at compile time | Runtime bytecode interpretation or compilation |

### 11.2 Three Levels of Complexity

```
Level 1 — Reflection (metadata query + call dispatch)
  → No runtime code generation needed
  → Only requires preserved metadata + function pointer table
  → Solvable with pure AOT

Level 2 — Dynamic proxy (generate a new class at runtime)
  → Requires runtime code generation
  → But generated code follows a fixed pattern: interface method → InvocationHandler.invoke()
  → Solvable with template AOT pre-generation + runtime assembly

Level 3 — Dynamic class loading (load arbitrary bytecode at runtime)
  → Requires runtime interpretation or compilation of arbitrary bytecode
  → Must embed a bytecode runtime
  → The hardest — and the one GraalVM abandoned entirely
```

---

## 12. Hybrid Runtime Architecture

### 12.1 Core Design: AOT + MicroRT

```
┌─────────────────────────────────────────────────────┐
│                  Rava Native Binary                   │
│                                                       │
│  ┌──────────────────────┐  ┌──────────────────────┐  │
│  │   AOT-compiled code   │  │ Embedded bytecode RT  │  │
│  │   (95%+ of code)      │  │   (Rava MicroRT)      │  │
│  │                       │  │                       │  │
│  │  • All statically     │  │  • Bytecode interpreter│  │
│  │    analyzable code    │  │  • Lightweight JIT     │  │
│  │  • Resolved reflection│  │    (optional)         │  │
│  │  • Pre-generated      │  │  • Class loader       │  │
│  │    proxy classes      │  │  • Reflection metadata │  │
│  │  • Direct machine     │  │    engine             │  │
│  │    code execution     │  │  • GC (shared)        │  │
│  └──────────┬────────────┘  └──────────┬────────────┘  │
│             │                          │               │
│             └─────────┬────────────────┘               │
│                       │                                │
│             ┌─────────▼──────────┐                     │
│             │   Unified Object    │                     │
│             │   Model             │                     │
│             │ (AOT objects and    │                     │
│             │  interpreter objects│                     │
│             │  share one heap)    │                     │
│             └─────────────────────┘                    │
└─────────────────────────────────────────────────────┘
```

**Key design decision: AOT code and interpreter code share a single object model and memory heap.**

This means:
- AOT-compiled methods can call objects running in the interpreter, and vice versa
- A `Method` found via reflection can point to either AOT code or bytecode in the interpreter
- Instances of dynamically loaded classes can be passed to AOT-compiled code

### 12.2 Rava MicroRT: Embedded Bytecode Runtime

MicroRT is not a full JVM. It is a lean runtime purpose-built as the escape hatch:

| Component | Description | Size estimate |
|-----------|-------------|--------------|
| Bytecode interpreter | Interprets Java bytecode (~200 instructions) | ~500 KB |
| Lightweight JIT | Compiles hot interpreter code (optional, uses Cranelift) | ~2 MB |
| Class loader | Loads bytecode from .class / .jar files | ~200 KB |
| Reflection metadata engine | Queries class/method/field metadata | ~100 KB |
| Bytecode verifier | Validates loaded bytecode for safety | ~150 KB |
| JNI environment layer | Provides `JNIEnv*` function table (~230 functions) and `JavaVM*` singleton for native library callbacks into Java | ~300 KB |
| **Total** | | **~3.3 MB** |

Final binary size: AOT code (~15 MB) + MicroRT (~3.3 MB) = **~18 MB** — still far smaller than a JVM (~200 MB).

### 12.3 Compile-Time Decision: AOT vs MicroRT

```
Rava compiler analysis pipeline:

1. Parse all source → AST → type check → semantic analysis

2. Reflection analysis pass:
   ├── Statically resolvable reflection calls → mark AOT (compile to function pointer calls)
   │   e.g. Class.forName("com.example.User")  ← string constant, resolvable at compile time
   │
   └── Dynamically unresolvable reflection calls → mark MicroRT
       e.g. Class.forName(config.get("className"))  ← only known at runtime

3. Proxy analysis pass:
   ├── Interface combination known at compile time → pre-generate proxy class, AOT compile
   └── Interface combination only known at runtime → MicroRT runtime generation

4. Class loading analysis pass:
   ├── Classes known at compile time → AOT
   └── Classes unknown at compile time (plugins, SPI) → MicroRT

5. Code generation:
   ├── AOT regions → LLVM/Cranelift → native machine code
   └── MicroRT regions → preserve bytecode + generate bridging code
```

---

## 13. Dynamic Features: Implementation

### 13.1 Reflection: Metadata Table + Dual-Path Dispatch

**Principle:** Reflection is fundamentally "find a function pointer by name and call it." After AOT compilation the function pointer is already known — we only need to retain a lookup table.

```
Compile-time metadata table (embedded in binary):

ClassMetadata {
  "com.example.User" → {
    fields: [
      { name: "id",   type: "long",   offset: 0,  getter: 0x7f001000 },
      { name: "name", type: "String", offset: 8,  getter: 0x7f001040 },
    ],
    methods: [
      { name: "getId",   signature: "()J",          ptr: 0x7f001000 },
      { name: "setName", signature: "(Ljava/lang/String;)V", ptr: 0x7f001080 },
    ],
    constructors: [
      { signature: "()V", ptr: 0x7f001100 },
    ]
  }
}
```

**Dual-path dispatch:**

```
Class.forName("com.example.User").getMethod("getId").invoke(obj)

Path A (AOT fast path):
  1. Query metadata table → find "com.example.User"
  2. Query method table  → find "getId" → function pointer 0x7f001000
  3. Call the function pointer directly (same speed as a normal method call)

Path B (MicroRT slow path):
  1. Query metadata table → not found (class unknown at compile time)
  2. Fall back to MicroRT → load .class file from classpath
  3. Parse bytecode → interpret or JIT compile
  4. Cache result → subsequent calls hit the cache
```

**Size impact:** The metadata table adds roughly 5–10% to binary size. Can be fully stripped with `rava build --strip-metadata` to trade reflection support for minimum size.

### 13.2 Dynamic Proxy: Template Pre-generation + Runtime Assembly

**Principle:** The code generated for a Java dynamic proxy follows a fixed pattern — every method is `handler.invoke(proxy, method, args)`. The only variation is the interface list and method signatures.

```java
// Java dynamic proxy in essence
Object proxy = Proxy.newProxyInstance(
    classLoader,
    new Class<?>[] { UserService.class, Cacheable.class },
    (proxy, method, args) -> {
        // interception logic
        return method.invoke(target, args);
    }
);

// The generated proxy class is essentially:
class $Proxy0 implements UserService, Cacheable {
    InvocationHandler handler;

    public User getUser(long id) {
        Method m = UserService.class.getMethod("getUser", long.class);
        return (User) handler.invoke(this, m, new Object[]{id});
    }
    // ... every interface method follows the same template
}
```

**Rava's three-layer strategy:**

```
Layer 1 — Compile-time pre-generation (covers 90%+ of cases)
  Compiler scans all Proxy.newProxyInstance() calls
  If the interface list is a compile-time constant → generate proxy class → AOT compile
  Spring @Transactional, MyBatis Mapper, etc. all fall here

Layer 2 — Template instantiation (covers ~9% of cases)
  Compiler generates a generic proxy template (AOT-compiled machine code)
  Runtime only needs to fill in: interface method table + InvocationHandler
  No new bytecode generation needed — only assembly of existing machine code fragments

Layer 3 — MicroRT fallback (covers ~1% of extreme cases)
  Runtime interface combination is completely unpredictable
  Fall back to MicroRT to generate bytecode → interpret
  Slow on first call; subsequent calls hit the cache
```

### 13.3 Dynamic Class Loading: Embedded Bytecode Runtime

**Principle:** Dynamic class loading is fundamentally "introducing code unknown at compile time." This is the root tension with AOT — but it is not an unsolvable one.

**Key insight: dynamically loaded classes do not need to be AOT-compiled. They can be interpreted.**

```
Scenario: SPI plugin loading

// Implementations are unknown at compile time
ServiceLoader<Plugin> plugins = ServiceLoader.load(Plugin.class);
for (Plugin p : plugins) {
    p.execute();  // calling code unknown at compile time
}

Rava's handling:

1. Plugin interface → AOT compile (known at compile time)
2. ServiceLoader.load() → scan META-INF/services/ at runtime
3. Discover com.third.MyPlugin → unknown at compile time
4. MicroRT loads MyPlugin.class → interprets bytecode
5. p.execute() → dispatched via interface; AOT code calls interpreter method
6. If MyPlugin.execute() becomes hot → MicroRT JIT-compiles it to machine code
```

**AOT ↔ MicroRT interop: the Unified Object Model**

```
┌─────────────────────────────────────────┐
│              Unified Object Header        │
│  ┌─────────┬──────────┬───────────────┐  │
│  │ Mark    │ Type ptr │ Origin tag    │  │
│  │ (GC)    │ (vtable) │ AOT/MicroRT   │  │
│  └─────────┴──────────┴───────────────┘  │
│                                          │
│  AOT object:                             │
│    Type ptr → AOT-compiled vtable        │
│               (array of function ptrs)   │
│                                          │
│  MicroRT object:                         │
│    Type ptr → Interpreter vtable         │
│               (bytecode method table)    │
│                                          │
│  Both object types allocated on the same │
│  heap and managed by the same GC         │
└─────────────────────────────────────────┘
```

When AOT code calls a method on a MicroRT object:
1. Read the type pointer from the object header
2. Detect it is a MicroRT vtable → jump to the interpreter entry point
3. Interpreter executes bytecode → returns result to AOT code

When MicroRT code calls a method on an AOT object:
1. Read the type pointer from the object header
2. Detect it is an AOT vtable → call the function pointer directly
3. Identical speed to a normal AOT call

---

## 14. JNI Subsystem

### 14.1 Two Directions, Two Levels of Complexity

JNI has two directions with very different complexity:

**Outbound (Java → C) — Phase 1**

Java declares `native` methods; C/C++ implements them:

```java
public class Database {
    static { System.loadLibrary("sqlite3"); }  // triggers dlopen
    public native long open(String path);      // AOT generates C call stub
    public native void exec(long db, String sql);
}
```

AOT compiler handling:
- `native` keyword → generate a C function call stub (a direct `call` instruction, no JNI overhead)
- `System.loadLibrary("sqlite3")` → runtime `dlopen` (Linux/macOS) or `LoadLibrary` (Windows)
- `System.load("/path/to/lib.so")` → load by explicit absolute path
- Static linking: merge `.a` archives at link time via `jni { static = ["sqlite3"] }` in `rava.hcl`

This is just ordinary C function calling — Phase 1 can implement it without MicroRT.

**Inbound (C → Java via JNIEnv\*) — Phase 3**

Native code calls back into Java via the JNI API:

```c
jclass    cls = (*env)->FindClass(env, "com/example/Callback");
jmethodID mid = (*env)->GetMethodID(env, cls, "onEvent", "(Ljava/lang/String;)V");
(*env)->CallVoidMethod(env, obj, mid, str);
```

This requires MicroRT to implement the full JNI function table.

### 14.2 MicroRT JNI Function Table (~230 Functions)

| Function group | MicroRT mapping |
|---------------|----------------|
| `FindClass`, `GetSuperclass` | ClassLoader (reflection metadata engine) |
| `GetMethodID`, `GetStaticMethodID` | Reflection metadata query |
| `Call*Method`, `CallStatic*Method` | Virtual dispatch (vtable/itable) |
| `NewObject`, `AllocObject` | UnifiedHeap allocation |
| `Get/SetField`, `Get/SetStaticField` | Field offset table |
| `NewStringUTF`, `GetStringUTFChars` | String interning |
| `New*Array`, `Get/Set*ArrayRegion` | Array operations |
| `NewGlobalRef`, `DeleteGlobalRef` | Reference counting |
| `ExceptionOccurred`, `ThrowNew` | Exception model |
| `AttachCurrentThread` | Thread registry |
| `GetPrimitiveArrayCritical` | Zero-copy array access |
| `JNI_OnLoad` / `JNI_OnUnload` | Library lifecycle hooks |

`JNIEnv*` is a per-thread pointer; `JavaVM*` is a global singleton. Both are provided by MicroRT.

### 14.3 JNI Type Mapping

| JNI type | MicroRT internal representation |
|----------|--------------------------------|
| `jobject` | `HeapRef` (UnifiedHeap reference) |
| `jstring` | `HeapRef` → Java `String` object |
| `jclass` | `ClassRef` (class metadata pointer) |
| `jarray` | `HeapRef` → Java array object |
| `jint`, `jlong`, `jdouble` | Rust native types (`i32`, `i64`, `f64`) |
| `jboolean` | `u8` (0 = false, 1 = true) |
| `jbyteArray` | `HeapRef` → `byte[]` (zero-copy eligible) |

### 14.4 Reference Management

JNI defines three reference types; MicroRT supports all three:

- **Local Ref**: valid within a single JNI call; auto-released when the function returns (Local Frame stack)
- **Global Ref**: created via `NewGlobalRef`; must be freed explicitly via `DeleteGlobalRef`
- **Weak Global Ref**: weak reference; GC may collect the referent; created via `NewWeakGlobalRef`

### 14.5 Library Lifecycle

```
System.loadLibrary("foo")
  → dlopen("libfoo.so")
  → look up symbol JNI_OnLoad
  → call JNI_OnLoad(JavaVM*, void*)   ← MicroRT provides JavaVM*
  → library init (register native methods, cache jclass/jmethodID)
  → return JNI version (e.g. JNI_VERSION_1_8)

dlclose("libfoo.so")  (on program exit)
  → call JNI_OnUnload(JavaVM*, void*)
  → library cleanup
```

### 14.6 GraalVM Comparison

| Capability | GraalVM native-image | Rava |
|-----------|---------------------|------|
| Outbound JNI (Java → C) | ✅ Supported (requires config) | ✅ Zero config, Phase 1 |
| `System.loadLibrary()` | ✅ Supported | ✅ Zero config |
| Inbound JNI (C → JNIEnv\*) | ⚠️ Limited, requires `@CEntryPoint` | ✅ Full function table, Phase 3 |
| `AttachCurrentThread` | ❌ Not supported | ✅ Thread registry, Phase 3 |
| `GetPrimitiveArrayCritical` | ✅ Supported | ✅ Zero-copy, Phase 3 |

---

## 15. Prior Art

The hybrid runtime approach is not new; it has well-established precedents:

### 15.1 GraalVM Truffle (Closest Precedent)

GraalVM's [Truffle framework](https://www.graalvm.org/jdk21/graalvm-as-a-platform/language-implementation-framework/HostOptimization/) is the production-grade incarnation of this idea:

- The Truffle interpreter itself is AOT-compiled into the native image
- The interpreter can dynamically interpret any guest-language code at runtime
- Hot guest code is JIT-compiled to machine code via Partial Evaluation
- Host (AOT) code and guest (interpreter) code share the same heap

Rava's MicroRT is essentially a Truffle-like embedded interpreter specialized for Java bytecode.

### 15.2 LuaJIT (Embedded Interpreter + JIT)

LuaJIT packages a Lua interpreter and JIT compiler into a ~500 KB single-file library. Any program that links against LuaJIT gains full Lua dynamic execution. Rava MicroRT does the same for Java bytecode.

### 15.3 Android ART (AOT + JIT Hybrid)

Android ART achieves:
- Frequently used code: AOT-compiled (at install time)
- Infrequently used or first-run code: JIT-executed
- Both share the same object model and GC

This proves that AOT + interpreter/JIT hybrid runtimes are production-viable.

### 15.4 .NET NativeAOT + Partial Interpreter

.NET NativeAOT faced the same reflection problem. .NET 9's solution:
- Statically resolvable reflection → AOT
- Unresolvable reflection → preserve metadata, handle at runtime via a built-in interpreter layer
- Unlike GraalVM, it does not error out

---

## 16. Trade-offs

This approach is not free. An honest accounting of costs:

### 16.1 Implementation Complexity

| Component | Complexity | Notes |
|-----------|-----------|-------|
| AOT compiler | High | Rava's core — required regardless |
| Metadata table generation | Medium | Additional compiler pass |
| MicroRT bytecode interpreter | High | ~100–200 K lines of Rust |
| AOT↔MicroRT interop layer | High | Unified object model is the hardest part |
| MicroRT JIT (optional) | Very high | Use Cranelift rather than building from scratch |
| Unified GC | High | GC must manage both object types simultaneously |

**This is a 2–3 year engineering project, not a 6-month deliverable.**

### 16.2 Performance Impact

| Scenario | Performance impact |
|----------|------------------|
| Pure AOT code (no reflection/proxy/dynamic loading) | Zero impact |
| Reflection call to an AOT-known class | Near-zero (metadata table lookup, faster than JVM reflection) |
| Reflection call to an unknown class | Slow on first call (MicroRT load); fast on cache hits thereafter |
| Dynamic proxy (compile-time known interfaces) | Zero impact (AOT pre-generated) |
| Dynamic proxy (runtime interfaces) | Small overhead on first call; normal thereafter |
| Dynamic class loading | Interpreted execution 2–5× slower than AOT; approaches AOT speed after JIT |

### 16.3 Binary Size

```
Final binary composition:
  AOT-compiled application code   ~15 MB
  AOT-compiled dependency code    ~10 MB
  MicroRT (interpreter)           ~3.3 MB
  Metadata table (for reflection) ~2 MB
  ──────────────────────────────────────
  Total                           ~30 MB   ← still far smaller than JVM 200 MB
```

### 16.4 Startup Time

```
Pure AOT code path:            ~10 ms   (no MicroRT initialization)
MicroRT present but not triggered: ~12 ms   (MicroRT init is lightweight)
Dynamic class loading triggered:   ~50 ms   (first bytecode load)
Traditional JVM:               ~2,000 ms
```

---

## 17. Phased Implementation Roadmap

### Phase 1 — Basic AOT (no MicroRT)

```
Goal:    make 80% of Java code AOT-compilable and runnable
Covers:  static code, Lambda, generics, stdlib, JNI Outbound
Excludes: reflection, dynamic proxy, dynamic class loading, JNI Inbound

Value delivered at this phase:
  - A friendlier toolchain than GraalVM native-image
  - HCL config, dependency management, rava run developer experience
  - Pure static code (algorithm libraries, CLI tools) runs perfectly
  - JNI Outbound: SQLite, OpenSSL, RocksDB, and any native library work out of the box
```

### Phase 2 — Reflection Support

```
Goal:    support reflection
Impl:    compile-time metadata table generation + dual-path dispatch
         (no MicroRT needed — reflection requires metadata, not an interpreter)

Frameworks unlocked: Jackson, Lombok (most cases)
Not yet: dynamic proxy, dynamic class loading, JNI Inbound
```

### Phase 3 — MicroRT v1 (Bytecode Interpreter)

```
Goal:    implement a Java bytecode interpreter
Impl:    Java bytecode interpreter (Rust implementation)
         Unified object model (AOT↔MicroRT interop)
         Full JNI function table (JNIEnv*, JavaVM*)

Unlocks: dynamic class loading, SPI, plugin systems, JNI Inbound
Frameworks: MyBatis, Hibernate (via interpreter), any JNI-heavy library
```

### Phase 4 — Dynamic Proxy AOT Promotion

```
Goal:    lift dynamic proxy from interpreter to AOT
Impl:    proxy template pre-generation + runtime assembly

Frameworks unlocked: Spring @Transactional (fully AOT), JDK Proxy (common combos AOT)
```

### Phase 5 — MicroRT v2 (Hot JIT)

```
Goal:    JIT-compile hot code paths inside MicroRT
Impl:    Cranelift as the JIT backend

Result:  dynamic class hot paths reach near-AOT performance
         Spring Boot + Hibernate runs fully, performance ~90% of JVM
```

---

## 18. Technical Challenges

An honest list of the three hardest problems:

### 18.1 Unified Object Model (Hardest)

AOT-compiled objects and MicroRT-interpreted objects must share the same memory representation and GC. This means Rava needs a custom GC capable of managing objects from both sources. Reference: Android ART. Estimated effort: 6–12 months.

### 18.2 Java Standard Library Coverage in the Bytecode Interpreter

The Java standard library contains thousands of classes. MicroRT cannot re-implement all of them. Solution: the vast majority of the standard library is already AOT-compiled into the binary; MicroRT only needs to be able to call those AOT-compiled stdlib methods (reverse interop). This is feasible but requires careful design.

### 18.3 Security

Dynamic class loading means arbitrary code can be loaded at runtime. A bytecode verifier is required to prevent malicious code. This is a mandatory component of MicroRT.

---

## 19. Feature Feasibility Summary

| Feature | Feasible? | Approach | Cost |
|---------|----------|----------|------|
| Reflection | ✅ Fully | AOT metadata table + dual-path dispatch | Phase 2 |
| Dynamic proxy | ✅ Fully | AOT pre-generation + MicroRT fallback | Phase 3–4 |
| Dynamic class loading | ✅ Fully | Embedded MicroRT bytecode runtime | Phase 3, most complex |
| JNI Outbound (Java → C) | ✅ Fully | AOT native method stubs + dlopen | Phase 1, low complexity |
| JNI Inbound (C → JNIEnv\*) | ✅ Fully | MicroRT JNI function table (~230 functions) | Phase 3, parallel with dynamic class loading |

**Rava's differentiation:** where GraalVM says "I can't do it — you configure it," Rava says "I handle it — you don't need to know." The cost is high engineering complexity and a long development timeline, but that is precisely Rava's core technical moat.

The Rava that achieves this will mean:
- Any Spring Boot / Hibernate / MyBatis / SQLite JNI project compiles to a native binary with zero code changes
- 10 ms startup, 20 MB memory, single-file deployment
- Something GraalVM cannot do — and the most urgent need in the entire Java ecosystem

---

*References:*
- [GraalVM Reachability Metadata](https://docs.oracle.com/en/graalvm/jdk/21/docs/reference-manual/native-image/metadata/)
- [GraalVM Dynamic Proxy](https://www.graalvm.org/latest/reference-manual/native-image/dynamic-features/DynamicProxy/)
- [GraalVM Truffle Host Optimization](https://www.graalvm.org/jdk21/graalvm-as-a-platform/language-implementation-framework/HostOptimization/)
- [ByteBuddy GraalVM Issue #1588](https://github.com/raphw/byte-buddy/issues/1588)
- [Flutter dart:mirrors Issue #1150](https://github.com/flutter/flutter/issues/1150)
- [OpenJDK JEP 8335368 — Ahead-of-Time Code Compilation](https://openjdk.org/jeps/8335368)
- [Java 25 AOT Cache Deep Dive](https://andrewbaker.ninja/2025/12/23/java-25-aot-cache-a-deep-dive-into-ahead-of-time-compilation-and-training/)
