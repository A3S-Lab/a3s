import constants from "@/constants";
import { Button } from "@/components/ui/button";
import { useState } from "react";
import { toast } from "sonner";

export default function Home() {
	const [testing, setTesting] = useState(false);

	const testBackendConnection = async () => {
		setTesting(true);
		try {
			// 测试创建订单
			const response = await fetch("http://localhost:3000/api/orders", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({
					customerId: "test-customer",
					items: [
						{ productId: "product-001", quantity: 2, unitPrice: 99.99 },
					],
				}),
			});

			if (!response.ok) throw new Error("API 调用失败");

			const data = await response.json();
			toast.success(`✅ 后端连接成功！订单已创建: ${data.orderId.slice(0, 8)}...`);
		} catch (error) {
			toast.error("❌ 后端连接失败: " + (error as Error).message);
		} finally {
			setTesting(false);
		}
	};

	return (
		<div className="flex flex-col h-full w-full items-center justify-center gap-4 bg-background">
			<div className="flex flex-col items-center gap-2">
				<h1 className="text-3xl font-bold text-foreground">
					{constants.name}
				</h1>
				<p className="text-muted-foreground text-sm">
					{constants.description}
				</p>
			</div>
			<div className="mt-8 flex flex-col items-center gap-4">
				<Button onClick={testBackendConnection} disabled={testing}>
					{testing ? "测试中..." : "🔗 测试后端连接"}
				</Button>
				<p className="text-muted-foreground/50 text-xs">
					点击按钮测试 NestJS 后端 API 调用
				</p>
			</div>
		</div>
	);
}
