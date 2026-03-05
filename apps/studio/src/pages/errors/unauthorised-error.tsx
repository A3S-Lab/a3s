import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";

export default function UnauthorisedError() {
	const navigate = useNavigate();
	return (
		<div className="h-svh">
			<div className="m-auto flex h-full w-full flex-col items-center justify-center gap-2">
				<h1 className="text-[7rem] font-bold leading-tight">401</h1>
				<span className="font-medium">无权访问此页面</span>
				<p className="text-center text-muted-foreground">
					您尝试访问的资源需要身份验证，
					<br />
					请使用正确的凭据登录。
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
