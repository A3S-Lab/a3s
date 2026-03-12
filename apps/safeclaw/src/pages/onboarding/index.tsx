import { AvatarUploader } from "@/components/custom/avatar-uploader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { fileToDataUrl } from "@/lib/image";
import globalModel from "@/models/global.model";
import { ArrowRight } from "lucide-react";
import { motion } from "motion/react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

export default function OnboardingPage() {
	const nav = useNavigate();
	const [nickname, setNickname] = useState("");
	const [avatar, setAvatar] = useState("/safeclaw.jpg");

	const handleStart = () => {
		globalModel.setProfile(nickname.trim() || "用户", avatar);
		nav("/", { replace: true });
	};

	return (
		<div className="flex h-screen w-screen items-center justify-center bg-background">
			<motion.div
				initial={{ opacity: 0, y: 20 }}
				animate={{ opacity: 1, y: 0 }}
				transition={{ duration: 0.4, ease: "easeOut" }}
				className="w-full max-w-sm rounded-2xl border bg-card p-8 shadow-xl"
			>
				<div className="flex flex-col items-center gap-1 mb-8">
					<h1 className="text-xl font-bold">欢迎使用 SafeClaw</h1>
					<p className="text-sm text-muted-foreground text-center">
						设置你的昵称和头像，开始使用吧
					</p>
				</div>

				<div className="flex flex-col items-center gap-6">
					<AvatarUploader
						className="size-24"
						value={avatar}
						onChange={setAvatar}
						onUpload={fileToDataUrl}
						cropProps={{}}
					/>

					<div className="w-full space-y-1.5">
						<label
							htmlFor="onboarding-nickname"
							className="text-sm font-medium"
						>
							昵称
						</label>
						<Input
							id="onboarding-nickname"
							placeholder="请输入你的昵称（可选）"
							value={nickname}
							onChange={(e) => setNickname(e.target.value)}
							onKeyDown={(e) => e.key === "Enter" && handleStart()}
							maxLength={20}
						/>
					</div>

					<Button className="w-full" onClick={handleStart}>
						开始使用
						<ArrowRight className="ml-2 size-4" />
					</Button>
				</div>
			</motion.div>
		</div>
	);
}
