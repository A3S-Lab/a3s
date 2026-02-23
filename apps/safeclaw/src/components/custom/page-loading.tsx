import { getAssetsUrl } from "@/lib/url";
import { Player } from "@lottiefiles/react-lottie-player";
import { tv, VariantProps } from "tailwind-variants";

const pageLoadingVariants = tv({
	slots: {
		base: "flex flex-col justify-center items-center",
		player: "size-48",
		text: "text-center text-muted-foreground/50",
	},
	variants: {
		size: {
			default: {
				player: "size-48",
				text: "text-sm",
			},
			sm: {
				player: "size-32",
				text: "text-sm",
			},
			lg: {
				player: "size-64",
				text: "text-lg",
			},
		},
	},
	defaultVariants: {
		size: "default",
	},
});

export interface PageLoadingProps
	extends React.HTMLAttributes<HTMLDivElement>,
		VariantProps<typeof pageLoadingVariants> {
	tip?: string;
}

const PageLoading = ({
	className,
	size,
	tip = "正在加载中...",
}: PageLoadingProps) => {
	const { base, player, text } = pageLoadingVariants({ className, size });
	return (
		<div className={base()}>
			<Player
				className={player()}
				autoplay
				loop
				src={getAssetsUrl("lotties/loading.json")}
			/>
			<div className={text()}>{tip}</div>
		</div>
	);
};

PageLoading.displayName = "PageLoading";

export { PageLoading, pageLoadingVariants };
