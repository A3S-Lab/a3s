/**
 * A3S Box Management — MicroVM runtime dashboard.
 * Uses shared SidebarLayout for consistent navigation.
 */
import {
	SidebarLayout,
	type SidebarSection,
} from "@/components/layout/sidebar-layout";
import {
	Box,
	Container,
	Image,
	Network,
	HardDrive,
	Camera,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useSnapshot } from "valtio";
import boxModel from "@/models/box.model";

import { OverviewSection } from "./components/overview-section";
import { BoxesSection } from "./components/boxes-section";
import { ImagesSection } from "./components/images-section";
import { NetworksSection } from "./components/networks-section";
import { VolumesSection } from "./components/volumes-section";
import { SnapshotsSection } from "./components/snapshots-section";

type SectionId =
	| "overview"
	| "boxes"
	| "images"
	| "networks"
	| "volumes"
	| "snapshots";

const sections: SidebarSection<SectionId>[] = [
	{
		id: "overview",
		label: "系统概览",
		icon: Box,
		description: "运行状态与资源",
	},
	{
		id: "boxes",
		label: "容器管理",
		icon: Container,
		description: "MicroVM 实例",
	},
	{
		id: "images",
		label: "镜像管理",
		icon: Image,
		description: "OCI 镜像",
	},
	{
		id: "networks",
		label: "网络",
		icon: Network,
		description: "虚拟网络",
	},
	{
		id: "volumes",
		label: "存储卷",
		icon: HardDrive,
		description: "数据持久化",
	},
	{
		id: "snapshots",
		label: "快照",
		icon: Camera,
		description: "VM 快照",
	},
];

export default function BoxPage() {
	const [section, setSection] = useState<SectionId>("overview");
	const snap = useSnapshot(boxModel.state);

	useEffect(() => {
		boxModel.fetchSystemInfo();
		boxModel.fetchBoxes();
	}, []);

	return (
		<SidebarLayout
			title="Box 管理"
			subtitle="MicroVM 运行时"
			sections={sections}
			current={section}
			onChange={setSection}
			contentMaxWidth="max-w-4xl"
			footer="A3S Box · MicroVM Runtime"
			badge={(id) =>
				id === "boxes" && snap.boxes.length > 0 ? (
					<span className="text-[10px] rounded-full px-1.5 py-0.5 min-w-[20px] text-center bg-primary/15 text-primary font-medium">
						{snap.boxes.filter((b) => b.status === "running").length}
					</span>
				) : null
			}
		>
			{section === "overview" && <OverviewSection />}
			{section === "boxes" && <BoxesSection />}
			{section === "images" && <ImagesSection />}
			{section === "networks" && <NetworksSection />}
			{section === "volumes" && <VolumesSection />}
			{section === "snapshots" && <SnapshotsSection />}
		</SidebarLayout>
	);
}
