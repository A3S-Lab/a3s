// agents/demo.ts — example a3s-code agent runner
import { Agent } from "@a3s-lab/code";

async function main() {
  const agent = await Agent.fromConfig("config.hcl", { agent: "demo" });
  const session = await agent.session();
  const result = await session.run("Say hello to the world");
  console.log(result);
  await session.close();
}

main().catch(console.error);
