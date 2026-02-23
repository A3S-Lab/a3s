// skills/hello.ts — example a3s-code skill
import { skill, ToolResult } from "@a3s-lab/code";

export const hello = skill({
  name: "hello",
  description: "Say hello to someone",
  parameters: {
    name: { type: "string", description: "Name to greet" },
  },
  async execute({ name }: { name: string }): Promise<ToolResult> {
    return { content: `Hello, ${name}!` };
  },
});
