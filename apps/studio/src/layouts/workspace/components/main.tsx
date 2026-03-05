import { ReactNode } from "react";

export default function Main({ children }: { children: ReactNode }) {
	return (
		<main className="flex flex-col h-full w-full overflow-hidden">
			{children}
		</main>
	);
}
