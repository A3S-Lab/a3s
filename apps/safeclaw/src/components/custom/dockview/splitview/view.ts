import {
	SplitviewApi,
	PanelViewInitParameters,
	SplitviewPanel,
} from "@/components/custom/dockview-core";
import { ReactPart, ReactPortalStore } from "../react";
import { ISplitviewPanelProps } from "./splitview";

export class ReactPanelView extends SplitviewPanel {
	constructor(
		id: string,
		component: string,
		private readonly reactComponent: React.FunctionComponent<ISplitviewPanelProps>,
		private readonly reactPortalStore: ReactPortalStore,
	) {
		super(id, component);
	}

	getComponent(): ReactPart<ISplitviewPanelProps> {
		return new ReactPart<any>(
			this.element,
			this.reactPortalStore,
			this.reactComponent as any,
			{
				params: this._params?.params ?? {},
				api: this.api,
				containerApi: new SplitviewApi(
					(this._params as PanelViewInitParameters).accessor,
				),
			} as any,
		);
	}
}
