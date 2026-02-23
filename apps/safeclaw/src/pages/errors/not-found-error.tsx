import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";

export default function NotFoundError() {
	const navigate = useNavigate();
	return (
		<div className="h-svh">
			<div className="m-auto flex h-full w-full flex-col items-center justify-center gap-2">
				<h1 className="text-[7rem] font-bold leading-tight">404</h1>
				<span className="font-medium">页面未找到</span>
				<p className="text-center text-muted-foreground">
					您访问的页面不存在
					<br />
					或已被移除。
				</p>
				<div className="mt-6 flex gap-4">
					<Button variant="outline" onClick={() => navigate(-1)}>
						返回
					</Button>
					<Button onClick={() => navigate("/")}>回到首页</Button>
				</div>
			</div>
		</div>
	);
}
