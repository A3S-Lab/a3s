import { useNavigate } from "react-router-dom";
import type { ReactNode } from "react";

export interface NavLink {
	name: string;
	url: string;
	icon?: ReactNode;
	items?: NavLink[];
}

export function useNavbar() {
	const navigateTo = useNavigate();

	return {
		navigate: (item: Pick<NavLink, "url">) => {
			navigateTo(item.url);
		},
	};
}
