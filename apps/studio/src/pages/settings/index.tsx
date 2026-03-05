import { useTheme } from "@/components/custom/theme-provider";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import constants from "@/constants";

export default function Settings() {
	const { theme, setTheme } = useTheme();

	return (
		<div className="flex flex-col h-full w-full bg-background overflow-y-auto">
			<div className="max-w-2xl mx-auto w-full p-8 space-y-6">
				<h1 className="text-2xl font-bold">设置</h1>

				<Card>
					<CardHeader>
						<CardTitle>外观</CardTitle>
					</CardHeader>
					<CardContent className="space-y-4">
						<div className="flex items-center justify-between">
							<Label>主题</Label>
							<Select value={theme} onValueChange={setTheme}>
								<SelectTrigger className="w-[180px]">
									<SelectValue placeholder="选择主题" />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="light">浅色</SelectItem>
									<SelectItem value="dark">深色</SelectItem>
									<SelectItem value="system">跟随系统</SelectItem>
								</SelectContent>
							</Select>
						</div>
					</CardContent>
				</Card>

				<Card>
					<CardHeader>
						<CardTitle>Sidecar</CardTitle>
					</CardHeader>
					<CardContent className="space-y-4">
						<div className="flex items-center justify-between">
							<Label>后端地址</Label>
							<span className="text-sm text-muted-foreground font-mono">
								{constants.sidecarUrl}
							</span>
						</div>
					</CardContent>
				</Card>

				<Card>
					<CardHeader>
						<CardTitle>关于</CardTitle>
					</CardHeader>
					<CardContent className="space-y-2">
						<div className="flex items-center justify-between">
							<Label>应用名称</Label>
							<span className="text-sm text-muted-foreground">
								{constants.name}
							</span>
						</div>
						<div className="flex items-center justify-between">
							<Label>版本</Label>
							<span className="text-sm text-muted-foreground">0.1.0</span>
						</div>
					</CardContent>
				</Card>
			</div>
		</div>
	);
}
