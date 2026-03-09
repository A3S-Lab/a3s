# A3S Code 文档修复计划

## 问题总结

1. **Rule 17 违规** - 文档中大量使用布尔值配置（`defaultSecurity: true`, `builtinSkills: true`），而实际 SDK 已实现类型化对象
2. **中间件系统缺失** - 文档完全没有展示中间件的使用和配置
3. **Rust 示例残留** - 部分文档仍有 Rust 代码示例
4. **API 不一致** - 文档示例与实际 SDK API 不匹配

## 实际 SDK API（Node.js）

### SessionOptions 正确用法

```typescript
import { Agent, FileMemoryStore, FileSessionStore, DefaultSecurityProvider } from '@a3s-lab/code';

const agent = await Agent.create('agent.hcl');

const session = agent.session('.', {
  // ✅ 类型化对象（Rule 17 compliant）
  memoryStore: new FileMemoryStore('./memory'),
  sessionStore: new FileSessionStore('./sessions'),
  securityProvider: new DefaultSecurityProvider(),

  // ✅ 功能开关（布尔值 OK - 不是扩展选择）
  builtinSkills: true,
  planning: true,
  permissive: false,
  autoSave: true,

  // ✅ 配置参数
  model: 'openai/gpt-4o',
  maxParseRetries: 3,
  toolTimeoutMs: 30000,
  circuitBreakerThreshold: 5,

  // ✅ 提示词插槽
  role: 'You are a senior Rust developer.',
  guidelines: 'Use clippy. No unwrap().',
  responseStyle: 'Be concise.',
  extra: 'This project uses tokio.',
});
```

### 中间件系统

```typescript
import { MiddlewarePipeline, MiddlewareContext } from '@a3s-lab/code';

// 同步中间件
const pipeline = new MiddlewarePipeline();
pipeline.useMiddleware((ctx: MiddlewareContext) => {
  console.log(`Session: ${ctx.sessionId}`);
  return { resultType: 'continue' };
});

// 异步中间件（需要 AsyncMiddlewarePipeline wrapper）
class AsyncMiddlewarePipeline {
  private pipeline: MiddlewarePipeline;
  private pendingPromises: Promise<any>[] = [];

  use(middleware: (ctx) => any | Promise<any>): this {
    const isAsync = middleware.constructor.name === 'AsyncFunction';
    if (isAsync) {
      const wrapped = (ctx) => {
        this.pendingPromises.push(middleware(ctx));
        return { resultType: 'continue' };
      };
      this.pipeline.useMiddleware(wrapped);
    } else {
      this.pipeline.useMiddleware(middleware);
    }
    return this;
  }

  async execute(ctx): Promise<any> {
    const result = await this.pipeline.execute(ctx);
    await Promise.all(this.pendingPromises);
    return result;
  }
}
```

## 需要修复的文档文件

### 核心文档
- [ ] `apps/docs/content/docs/{cn,en}/code/index.mdx` - 移除 `defaultSecurity: true`
- [ ] `apps/docs/content/docs/{cn,en}/code/sessions.mdx` - 添加中间件章节，修复 SessionOptions
- [ ] `apps/docs/content/docs/{cn,en}/code/security.mdx` - 展示 `securityProvider: new DefaultSecurityProvider()`
- [ ] `apps/docs/content/docs/{cn,en}/code/memory.mdx` - 展示 `memoryStore: new FileMemoryStore()`
- [ ] `apps/docs/content/docs/{cn,en}/code/persistence.mdx` - 展示 `sessionStore: new FileSessionStore()`

### 示例文档
- [ ] `apps/docs/content/docs/{cn,en}/code/examples/*.mdx` - 检查所有示例

### 新增文档
- [ ] `apps/docs/content/docs/{cn,en}/code/middleware.mdx` - 中间件系统完整文档

## 修复优先级

1. **P0 - 核心 API 错误**：index.mdx, sessions.mdx（移除 defaultSecurity 等错误用法）
2. **P1 - 扩展点文档**：security.mdx, memory.mdx, persistence.mdx（展示正确的类型化对象）
3. **P2 - 新增功能**：middleware.mdx（补充缺失的中间件文档）
4. **P3 - 示例修复**：examples/*.mdx（确保所有示例正确）
