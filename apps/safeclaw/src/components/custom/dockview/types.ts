import { Parameters } from "@/components/custom/dockview-core";

export interface PanelParameters<T extends {} = Parameters> {
	params: T;
}
