# agents/demo.py — example a3s-code agent runner
import asyncio
from a3s_code import Agent

async def main():
    agent = Agent.from_config("config.hcl", agent="demo")
    async with agent.session() as session:
        result = await session.run("Say hello to the world")
        print(result)

if __name__ == "__main__":
    asyncio.run(main())
