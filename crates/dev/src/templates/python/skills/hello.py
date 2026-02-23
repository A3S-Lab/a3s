# skills/hello.py — example a3s-code skill
from a3s_code import skill, ToolResult

@skill(
    name="hello",
    description="Say hello to someone",
    parameters={
        "name": {"type": "string", "description": "Name to greet"},
    },
)
async def hello(name: str) -> ToolResult:
    return ToolResult(content=f"Hello, {name}!")
