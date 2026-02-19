# Rava — 产品需求文档

> 版本: 1.0 | 日期: 2026-02-19 | 作者: A3S Team

---

## 1. 概述

### 1.1 问题陈述

Java 生态系统强大但笨重。开发者面临：

- **启动慢**：JVM 冷启动需要数秒，不适合 CLI 工具、Serverless、边缘计算
- **内存大**：即使一个 Hello World 也要占用 100MB+ 内存
- **工具链碎片化**：Maven / Gradle / Ant 各自为政，配置冗长（XML / Groovy / Kotlin DSL）
- **依赖管理痛苦**：`pom.xml` 动辄数百行，依赖冲突是日常噩梦
- **部署复杂**：需要预装 JRE/JDK，Docker 镜像臃肿（200MB+）
- **开发体验落后**：相比 Bun/Deno 对 TypeScript 的体验，Java 的开发工具链停留在上一个时代

### 1.2 产品愿景

**Rava** 是一个用 Rust 编写的 Java AOT 编译器和一体化工具链。它之于 Java，就像 Bun 之于 TypeScript。

```
Bun : TypeScript = Rava : Java

一个二进制文件，搞定一切：
  rava run Main.java          → 直接运行 Java 源码
  rava build                  → AOT 编译为原生二进制
  rava init                   → 初始化项目（生成 rava.hcl）
  rava add spring-boot-web    → 添加依赖
  rava test                   → 运行测试
  rava fmt                    → 格式化代码
```

### 1.3 核心理念

**理念一：零配置即可运行。**

```bash
# 不需要安装 JDK，不需要 pom.xml，不需要 build.gradle
# 一个文件，直接跑
echo 'class Main { public static void main(String[] args) { System.out.println("Hello"); } }' > Main.java
rava run Main.java
# → Hello
```

**理念二：一个二进制，替代整个工具链。**

| 传统 Java | Rava |
|-----------|------|
| JDK (javac) | `rava` |
| Maven / Gradle | `rava` |
| GraalVM native-image | `rava build` |
| JUnit + Maven Surefire | `rava test` |
| google-java-format | `rava fmt` |
| jlink / jpackage | `rava build --bundle` |

**理念三：HCL 替代 XML/Groovy，人类可读的项目配置。**

```hcl
# rava.hcl — 项目配置（类比 package.json / Cargo.toml）
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
  target  = "native"       # native | jar | jlink
  main    = "com.example.Main"
  optimize = "speed"       # speed | size | debug
}

run {
  args = ["--server.port=8080"]
  env  = {
    SPRING_PROFILES_ACTIVE = "dev"
  }
}
```

**理念四：AOT 优先，JIT 可选。**

```
传统 Java:  .java → javac → .class → JVM (JIT) → 机器码
Rava:       .java → rava  → 原生二进制 (AOT)     → 直接执行

启动时间:   传统 ~2s  →  Rava ~10ms
内存占用:   传统 ~200MB → Rava ~20MB
```

### 1.4 目标用户

| 用户群 | 痛点 | Rava 价值 |
|--------|------|-----------|
| Java 后端开发者 | 工具链复杂、启动慢 | 一键运行、秒级启动 |
| 云原生 / Serverless 开发者 | JVM 冷启动不适合 Lambda | AOT 编译，10ms 启动 |
| CLI 工具开发者 | Java 不适合写 CLI | 编译为单文件原生二进制 |
| 微服务开发者 | Docker 镜像臃肿 | 编译为 scratch 镜像（<20MB） |
| Java 初学者 | 环境配置复杂 | 安装 rava 即可开始写代码 |

---

## 2. 核心能力

### 2.1 能力总览

| # | 能力 | 命令 | 说明 |
|---|------|------|------|
| 1 | 直接运行 | `rava run` | 直接运行 .java 源码，无需预编译 |
| 2 | AOT 编译 | `rava build` | 编译为原生二进制或优化 JAR |
| 3 | 项目管理 | `rava init` | 初始化项目，生成 rava.hcl |
| 4 | 依赖管理 | `rava add/remove/update` | 管理 Maven 依赖，自动解析传递依赖 |
| 5 | 测试运行 | `rava test` | 运行 JUnit 测试 |
| 6 | 代码格式化 | `rava fmt` | 格式化 Java 代码 |
| 7 | 代码检查 | `rava lint` | 静态分析和代码质量检查 |
| 8 | REPL | `rava repl` | 交互式 Java REPL |
| 9 | 脚本模式 | `rava script` | 单文件脚本运行（自动推断 main） |
| 10 | 打包发布 | `rava publish` | 发布到 Maven Central 或私有仓库 |

### 2.2 能力 1：直接运行 (`rava run`)

像运行脚本一样运行 Java，无需手动编译：

```bash
# 单文件运行
rava run Main.java

# 项目运行（读取 rava.hcl 中的 main 配置）
rava run

# 带参数运行
rava run Main.java -- --port 8080

# 监听文件变化，自动重启（开发模式）
rava run --watch
```

**运行策略**：

| 场景 | 策略 | 说明 |
|------|------|------|
| 单文件，无依赖 | 直接 AOT 编译运行 | 最快路径，~100ms 启动 |
| 单文件，有 import | 自动解析标准库 + rava.hcl 依赖 | 透明依赖注入 |
| 项目目录（有 rava.hcl） | 增量编译 + 运行 | 只重编译变更文件 |
| `--watch` 模式 | 文件监听 + 热重载 | 开发体验优先 |
| `--jit` 标志 | 回退到 JIT 模式 | 兼容性优先（需要特定 JVM 特性时） |

**脚本模式**（无需 `public static void main`）：

```java
// script.java — Rava 自动包装为可执行入口
var name = "World";
System.out.println("Hello, " + name + "!");

// 自动可用的顶层 import
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

### 2.3 能力 2：AOT 编译 (`rava build`)

将 Java 源码编译为原生二进制，无需 JVM 即可运行：

```bash
# 编译为原生二进制（默认）
rava build
# → target/my-api (原生可执行文件, ~15MB)

# 编译为优化 JAR
rava build --target jar
# → target/my-api.jar

# 编译为 jlink 精简运行时
rava build --target jlink
# → target/my-api/ (含精简 JRE, ~40MB)

# 交叉编译
rava build --platform linux-amd64
rava build --platform linux-arm64
rava build --platform macos-amd64
rava build --platform windows-amd64

# 优化选项
rava build --optimize speed    # 优化执行速度（默认）
rava build --optimize size     # 优化二进制体积
rava build --optimize debug    # 保留调试信息
```

**编译产物对比**：

| 目标 | 产物 | 体积 | 启动时间 | 需要 JVM |
|------|------|------|----------|----------|
| `native`（默认） | 单文件二进制 | ~15MB | ~10ms | 否 |
| `jar` | 可执行 JAR | ~5MB + 依赖 | ~2s | 是 |
| `jlink` | 精简 JRE + JAR | ~40MB | ~1s | 否（自带） |
| `docker` | scratch 容器镜像 | ~20MB | ~10ms | 否 |

**AOT 编译流水线**：

```
.java 源码
  → Rava 前端（解析 → AST → 类型检查 → 语义分析）
  → Rava 中间表示（RIR — Rava Intermediate Representation）
  → 优化 Pass（逃逸分析、内联、死代码消除、常量折叠）
  → 后端代码生成（LLVM / Cranelift）
  → 链接（静态链接标准库 + 依赖）
  → 原生二进制
```

### 2.4 能力 3：项目管理 (`rava init`)

```bash
# 初始化空项目
rava init my-project
# → my-project/
#   ├── rava.hcl
#   ├── src/
#   │   └── Main.java
#   └── test/
#       └── MainTest.java

# 从模板初始化
rava init my-api --template spring-web
rava init my-cli --template cli
rava init my-lib --template library

# 从现有 Maven/Gradle 项目迁移
rava init --from-maven     # 读取 pom.xml，生成 rava.hcl
rava init --from-gradle    # 读取 build.gradle，生成 rava.hcl
```

**项目结构**：

```
my-project/
├── rava.hcl                 # 项目配置（唯一配置文件）
├── rava.lock                # 依赖锁定文件（自动生成）
├── src/                     # 源码目录
│   └── com/example/
│       └── Main.java
├── test/                    # 测试目录
│   └── com/example/
│       └── MainTest.java
├── resources/               # 资源文件
│   └── application.properties
└── target/                  # 构建产物（自动生成）
    ├── cache/               # 增量编译缓存
    └── my-project           # 编译产物
```

### 2.5 能力 4：依赖管理 (`rava add/remove/update`)

```bash
# 添加依赖
rava add spring-boot-starter-web
rava add com.google.guava:guava@33.0.0-jre
rava add lombok --dev

# 移除依赖
rava remove guava

# 更新依赖
rava update                  # 更新所有依赖到兼容最新版
rava update guava            # 更新指定依赖
rava update --latest         # 更新到绝对最新版（可能有 breaking change）

# 查看依赖树
rava deps tree
# → com.example:my-api@0.1.0
#   ├── org.springframework.boot:spring-boot-starter-web@3.2.0
#   │   ├── org.springframework.boot:spring-boot-starter@3.2.0
#   │   ├── org.springframework.boot:spring-boot-starter-json@3.2.0
#   │   └── org.springframework.boot:spring-boot-starter-tomcat@3.2.0
#   └── com.google.guava:guava@33.0.0-jre

# 检查过时依赖
rava deps outdated

# 审计安全漏洞
rava deps audit
```

**依赖解析**：

| 特性 | 说明 |
|------|------|
| 仓库源 | Maven Central（默认）、自定义私有仓库 |
| 版本解析 | 语义化版本范围（`^3.2.0`、`~3.2.0`、`>=3.2.0,<4.0.0`） |
| 传递依赖 | 自动解析，冲突时选择最高兼容版本 |
| 锁定文件 | `rava.lock` 确保可重复构建 |
| 本地缓存 | `~/.rava/cache/` 全局缓存，跨项目共享 |
| 离线模式 | `rava add --offline` 仅使用本地缓存 |
| 短名称 | 常用依赖支持短名称（`spring-boot-web` → `org.springframework.boot:spring-boot-starter-web`） |

**rava.hcl 依赖配置**：

```hcl
dependencies {
  # 标准格式
  "org.springframework.boot:spring-boot-starter-web" = "3.2.0"

  # 版本范围
  "com.google.guava:guava" = "^33.0.0-jre"

  # 详细配置
  "io.netty:netty-all" = {
    version  = "4.1.100.Final"
    exclude  = ["io.netty:netty-transport-native-epoll"]
  }
}

dev_dependencies {
  "org.junit.jupiter:junit-jupiter" = "5.10.1"
  "org.mockito:mockito-core"        = "5.8.0"
}

repositories {
  maven_central = { url = "https://repo1.maven.org/maven2" }
  company       = {
    url      = "https://maven.company.internal/releases"
    username = env("MAVEN_USER")
    password = env("MAVEN_PASS")
  }
}
```

### 2.6 能力 5：测试运行 (`rava test`)

```bash
# 运行所有测试
rava test

# 运行指定测试类
rava test MainTest

# 运行指定测试方法
rava test MainTest::testHello

# 运行匹配模式的测试
rava test --filter "*.service.*"

# 监听模式（文件变化自动重跑）
rava test --watch

# 生成覆盖率报告
rava test --coverage
# → Coverage: 85.3% (lines), 78.2% (branches)
# → Report: target/coverage/index.html
```

**测试框架支持**：

| 框架 | 支持 | 说明 |
|------|------|------|
| JUnit 5 | 原生支持 | 默认测试框架 |
| JUnit 4 | 兼容 | 自动检测并适配 |
| TestNG | 插件 | 通过 rava.hcl 配置 |
| 断言库 | 透明 | AssertJ、Hamcrest 等直接使用 |

### 2.7 能力 6-7：代码格式化与检查 (`rava fmt` / `rava lint`)

```bash
# 格式化
rava fmt                     # 格式化所有 .java 文件
rava fmt --check             # 仅检查，不修改（CI 用）
rava fmt src/Main.java       # 格式化指定文件

# 代码检查
rava lint                    # 运行所有检查规则
rava lint --fix              # 自动修复可修复的问题
```

### 2.8 能力 8：交互式 REPL (`rava repl`)

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

### 2.9 能力 9-10：发布 (`rava publish`)

```bash
# 发布到 Maven Central
rava publish

# 发布到私有仓库
rava publish --registry company

# 干跑（不实际发布）
rava publish --dry-run
```

---

## 3. rava.hcl 完整规范

### 3.1 顶层结构

```hcl
# rava.hcl — Rava 项目配置文件

project {
  # 项目基本信息
}

dependencies {
  # 运行时依赖
}

dev_dependencies {
  # 开发/测试依赖
}

repositories {
  # Maven 仓库源
}

build {
  # 编译配置
}

run {
  # 运行配置
}

test {
  # 测试配置
}

publish {
  # 发布配置
}
```

### 3.2 project 块

```hcl
project {
  name        = "my-api"                    # 项目名称（必填）
  group       = "com.example"               # 组织 ID（发布时必填）
  version     = "0.1.0"                     # 语义化版本（必填）
  java        = "21"                        # Java 版本（必填）
  description = "My awesome API"            # 项目描述
  license     = "MIT"                       # 许可证
  homepage    = "https://github.com/..."    # 项目主页
  repository  = "https://github.com/..."    # 源码仓库

  authors = [
    "[name] <[email]>"
  ]
}
```

### 3.3 dependencies / dev_dependencies 块

```hcl
dependencies {
  # 简写：短名称 + 版本
  "spring-boot-web" = "3.2.0"

  # 标准格式：groupId:artifactId = version
  "com.google.guava:guava" = "33.0.0-jre"

  # 版本范围
  "org.slf4j:slf4j-api" = "^2.0.0"        # >=2.0.0, <3.0.0
  "io.netty:netty-all"  = "~4.1.100"      # >=4.1.100, <4.2.0

  # 详细配置
  "com.fasterxml.jackson.core:jackson-databind" = {
    version = "2.16.0"
    exclude = [
      "com.fasterxml.jackson.core:jackson-annotations"
    ]
  }

  # 本地路径依赖（monorepo）
  "my-common" = { path = "../common" }

  # Git 依赖
  "my-lib" = {
    git    = "https://github.com/org/my-lib.git"
    branch = "main"
  }
}
```

**短名称映射**（内置常用依赖别名）：

| 短名称 | 完整坐标 |
|--------|----------|
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

短名称注册表可通过 `rava alias list` 查看，支持用户自定义扩展。

### 3.4 build 块

```hcl
build {
  target   = "native"              # native | jar | jlink | docker
  main     = "com.example.Main"    # 主类（自动检测，可覆盖）
  optimize = "speed"               # speed | size | debug

  # AOT 编译选项
  aot {
    reflection_config = "reflect-config.json"   # 反射配置（可选）
    initialize_at_build_time = [                 # 构建时初始化的类
      "org.slf4j"
    ]
    enable_preview = true                        # 启用预览特性
  }

  # Docker 构建选项（target = "docker" 时）
  docker {
    base_image = "scratch"          # 基础镜像
    tag        = "my-api:latest"
    expose     = [8080]
    labels     = {
      maintainer = "[name]"
    }
  }

  # 交叉编译目标
  platforms = ["linux-amd64", "linux-arm64", "macos-amd64"]
}
```

### 3.5 run 块

```hcl
run {
  main = "com.example.Main"        # 主类（覆盖 build.main）
  args = ["--server.port=8080"]    # 运行参数

  env = {
    SPRING_PROFILES_ACTIVE = "dev"
    DATABASE_URL           = "jdbc:postgresql://localhost:5432/mydb"
  }

  # 开发模式配置
  watch {
    paths   = ["src/", "resources/"]
    exclude = ["*.class", "target/"]
    delay   = "500ms"
  }

  # JIT 回退配置（rava run --jit 时使用）
  jvm {
    heap_min = "256m"
    heap_max = "1g"
    options  = ["-XX:+UseG1GC"]
  }
}
```

### 3.6 test 块

```hcl
test {
  framework = "junit5"             # junit5 | junit4 | testng
  parallel  = true                 # 并行执行测试
  timeout   = "60s"                # 单个测试超时

  coverage {
    enabled    = true
    min_line   = 80                # 最低行覆盖率（%）
    min_branch = 70                # 最低分支覆盖率（%）
    exclude    = ["**/generated/**"]
  }
}
```

### 3.7 publish 块

```hcl
publish {
  registry = "maven_central"       # 发布目标仓库
  sign     = true                  # GPG 签名

  pom {
    # 额外 POM 信息（发布到 Maven Central 需要）
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

## 4. Java 兼容性

### 4.1 语言版本支持

| Java 版本 | 支持级别 | 说明 |
|-----------|----------|------|
| Java 21 (LTS) | 完整支持 | 首要目标，所有特性 |
| Java 17 (LTS) | 完整支持 | 广泛使用的 LTS |
| Java 11 (LTS) | 基本支持 | 旧项目迁移 |
| Java 8 | 有限支持 | 仅编译，不保证 AOT 全特性 |

### 4.2 语言特性支持

| 特性 | AOT 支持 | 说明 |
|------|----------|------|
| Records | ✅ | Java 16+ |
| Sealed Classes | ✅ | Java 17+ |
| Pattern Matching | ✅ | Java 21+ |
| Virtual Threads | ✅ | Java 21+，AOT 原生支持 |
| Text Blocks | ✅ | Java 15+ |
| Switch Expressions | ✅ | Java 14+ |
| var 局部变量 | ✅ | Java 10+ |
| Lambda / Stream | ✅ | Java 8+ |
| 泛型 | ✅ | 类型擦除在编译期处理 |
| 注解处理 | ✅ | 编译期执行（Lombok 等） |
| 反射（静态可解析） | ✅ 完整 | AOT 元数据表，零配置，比 JVM 反射更快 |
| 反射（动态不可解析） | ✅ 完整 | MicroRT 元数据引擎，自动降级，无需 reflect-config |
| 动态代理（编译时已知接口） | ✅ 完整 | AOT 预生成代理类，零运行时开销 |
| 动态代理（运行时接口） | ✅ 完整 | MicroRT 运行时生成，自动降级 |
| 动态类加载 | ✅ 完整 | 嵌入式 MicroRT 字节码运行时，无需 JVM |
| JNI | ⚠️ 有限 | 支持静态链接的 native 库 |

### 4.3 框架兼容性

| 框架 | 兼容级别 | 说明 |
|------|----------|------|
| Spring Boot 3.x | 一级支持 | 自动处理反射/代理配置 |
| Quarkus | 一级支持 | 本身就是 AOT 友好的 |
| Micronaut | 一级支持 | 编译时 DI，天然适合 AOT |
| Vert.x | 一级支持 | 无反射依赖 |
| Jakarta EE | 二级支持 | 部分特性需要配置 |
| Hibernate | 二级支持 | 需要反射配置 |
| MyBatis | 二级支持 | 需要反射配置 |
| Lombok | 一级支持 | 编译期注解处理，完全兼容 |

### 4.4 反射自动检测

AOT 编译的最大挑战是反射。Rava 通过多层策略自动处理：

```
1. 静态分析：扫描源码中的 Class.forName()、.getMethod() 等调用
2. 框架适配：内置 Spring/Hibernate/Jackson 等框架的反射规则
3. 注解扫描：@Component、@Entity、@JsonProperty 等自动注册
4. 运行时追踪：rava run --trace-reflection 记录实际反射调用
5. 手动配置：reflect-config.json 兜底
```

### 4.5 与现有项目互操作

```bash
# 从 Maven 项目迁移
cd existing-maven-project
rava init --from-maven
# → 读取 pom.xml，生成 rava.hcl
# → 保留 src/main/java 和 src/test/java 结构
# → 或迁移到 src/ 和 test/ 扁平结构（可选）

# 从 Gradle 项目迁移
rava init --from-gradle
# → 读取 build.gradle(.kts)，生成 rava.hcl

# 生成 pom.xml（反向兼容）
rava export maven
# → 生成 pom.xml，可在传统 Maven 环境中使用

# 混合使用：Rava 管理依赖，Maven/Gradle 构建
rava export maven --sync
# → 持续同步 rava.hcl → pom.xml
```

---

## 5. CLI 命令参考

### 5.1 命令总览

```
rava <command> [options] [args]

项目管理:
  init [name]              初始化新项目
  run [file]               运行 Java 源码或项目
  build                    编译项目
  test [pattern]           运行测试
  fmt [files...]           格式化代码
  lint [files...]          代码检查
  repl                     交互式 REPL
  clean                    清理构建产物

依赖管理:
  add <package>            添加依赖
  remove <package>         移除依赖
  update [package]         更新依赖
  deps tree                显示依赖树
  deps outdated            检查过时依赖
  deps audit               安全漏洞审计
  alias list               查看短名称映射

发布:
  publish                  发布到仓库
  export maven             导出 pom.xml

工具:
  upgrade                  升级 Rava 自身
  doctor                   诊断环境问题
  config                   管理全局配置
  completions <shell>      生成 Shell 补全脚本
```

### 5.2 全局选项

| 选项 | 说明 |
|------|------|
| `--verbose` / `-v` | 详细输出 |
| `--quiet` / `-q` | 静默模式 |
| `--color <auto\|always\|never>` | 颜色输出 |
| `--help` / `-h` | 帮助信息 |
| `--version` / `-V` | 版本信息 |

### 5.3 关键命令详解

**`rava run`**：

| 选项 | 说明 |
|------|------|
| `--watch` / `-w` | 监听文件变化，自动重启 |
| `--jit` | 使用 JIT 模式运行（回退到 JVM） |
| `--release` | 以 release 优化级别运行 |
| `--env <KEY=VALUE>` | 设置环境变量 |
| `-- <args>` | 传递给程序的参数 |

**`rava build`**：

| 选项 | 说明 |
|------|------|
| `--target <native\|jar\|jlink\|docker>` | 编译目标，默认 `native` |
| `--optimize <speed\|size\|debug>` | 优化策略，默认 `speed` |
| `--platform <target>` | 交叉编译目标平台 |
| `--output` / `-o` | 输出路径 |
| `--static` | 完全静态链接（Linux musl） |

**`rava add`**：

| 选项 | 说明 |
|------|------|
| `--dev` / `-D` | 添加为开发依赖 |
| `--exact` | 使用精确版本（不加 `^`） |
| `--offline` | 仅使用本地缓存 |

---

## 6. 全局配置

### 6.1 配置文件位置

```
~/.rava/
├── config.hcl              # 全局配置
├── cache/                   # 依赖缓存
│   └── repository/          # Maven 仓库缓存
├── toolchains/              # JDK 工具链（JIT 回退用）
└── aliases.hcl              # 用户自定义短名称
```

### 6.2 全局配置 (`~/.rava/config.hcl`)

```hcl
# 默认 Java 版本
default_java = "21"

# 默认编译目标
default_target = "native"

# 代理配置
proxy {
  http  = "http://proxy.company.internal:8080"
  https = "http://proxy.company.internal:8080"
  no_proxy = ["localhost", "*.internal"]
}

# 镜像仓库（加速国内下载）
mirror {
  maven_central = "https://maven.aliyun.com/repository/central"
}

# 遥测（可关闭）
telemetry = false
```

---

## 7. 非功能需求

### 7.1 性能目标

| 指标 | 目标 | 对比 |
|------|------|------|
| 单文件运行启动 | < 200ms | javac + java: ~2s |
| AOT 编译后启动 | < 20ms | JVM: ~2s, GraalVM native: ~50ms |
| 增量编译速度 | < 1s（单文件变更） | Maven: ~5s |
| 全量编译（中型项目） | < 30s | Maven: ~60s |
| 内存占用（AOT 产物） | < 50MB（中型 Web 应用） | JVM: ~200MB |
| 二进制体积 | < 30MB（中型 Web 应用） | GraalVM: ~60MB |
| 依赖解析 | < 2s（有缓存） | Maven: ~10s |
| Rava 自身二进制体积 | < 50MB | GraalVM: ~400MB |

### 7.2 可靠性

| 指标 | 目标 |
|------|------|
| 编译正确性 | 通过 Java TCK 核心子集 |
| 依赖解析一致性 | rava.lock 保证 100% 可重复构建 |
| 崩溃恢复 | 编译中断不损坏缓存 |
| 错误信息 | 每个错误包含文件名、行号、修复建议 |

### 7.3 平台支持

| 平台 | 支持级别 |
|------|----------|
| Linux x86_64 | 一级（CI 测试） |
| Linux aarch64 | 一级（CI 测试） |
| macOS x86_64 | 一级（CI 测试） |
| macOS aarch64 (Apple Silicon) | 一级（CI 测试） |
| Windows x86_64 | 二级（社区测试） |

### 7.4 安装方式

```bash
# macOS / Linux（推荐）
curl -fsSL https://rava.dev/install.sh | sh

# Homebrew
brew install a3s-lab/tap/rava

# Cargo（从源码编译）
cargo install rava

# 版本管理
rava upgrade              # 自动升级到最新版
rava upgrade --canary     # 升级到 canary 版本
```

---

## 8. 与 Bun 的类比对照

| 维度 | Bun (TypeScript) | Rava (Java) |
|------|-------------------|-------------|
| 实现语言 | Zig + C++ | Rust |
| 替代 | Node.js + npm + webpack | JDK + Maven/Gradle + GraalVM |
| 运行 | `bun run index.ts` | `rava run Main.java` |
| 编译 | `bun build` | `rava build` |
| 包管理 | `bun add express` | `rava add spring-boot-web` |
| 配置 | `package.json` | `rava.hcl` |
| 锁文件 | `bun.lockb` | `rava.lock` |
| 测试 | `bun test` | `rava test` |
| REPL | `bun repl` (Node) | `rava repl` |
| 包仓库 | npm | Maven Central |
| 核心优势 | 极速 JS/TS 运行时 | 极速 Java AOT 编译 + 运行时 |

---

## 9. 术语表

| 术语 | 说明 |
|------|------|
| AOT | Ahead-of-Time 编译，在运行前将源码编译为原生机器码 |
| JIT | Just-in-Time 编译，运行时将字节码编译为机器码（传统 JVM 方式） |
| RIR | Rava Intermediate Representation，Rava 中间表示 |
| rava.hcl | Rava 项目配置文件，类比 package.json / Cargo.toml |
| rava.lock | 依赖锁定文件，确保可重复构建 |
| 短名称 | 常用 Maven 依赖的别名（如 `guava` → `com.google.guava:guava`） |
| 反射配置 | AOT 编译时需要的反射元数据，Rava 自动检测 + 手动兜底 |
| TCK | Technology Compatibility Kit，Java 技术兼容性测试套件 |
| LLVM | 编译器后端框架，用于生成原生机器码 |
| Cranelift | Rust 生态的轻量级代码生成后端，编译速度快 |

---

> Rava 的目标是让 Java 开发者拥有与 Bun/Deno 用户同等的开发体验：一个二进制文件，零配置运行，秒级编译，人类可读的项目配置。通过 Rust 实现的 AOT 编译器，Java 程序可以编译为 10ms 启动、20MB 内存的原生二进制，真正适合云原生、Serverless 和 CLI 场景。
