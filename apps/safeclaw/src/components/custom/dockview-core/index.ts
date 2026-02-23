import "./svg.scss";
import "./theme.scss";

export {
	getPaneData,
	getPanelData,
	PaneTransfer,
	PanelTransfer,
} from "./dnd/dataTransfer";

/**
 * Events, Emitters and Disposables are very common concepts that many codebases will contain, however we need
 * to export them for dockview framework packages to use.
 * To be a good citizen these are exported with a `Dockview` prefix to prevent accidental use by others.
 */
export { Emitter as DockviewEmitter, Event as DockviewEvent } from "./events";
export {
	MutableDisposable as DockviewMutableDisposable,
	CompositeDisposable as DockviewCompositeDisposable,
	Disposable as DockviewDisposable,
} from "./lifecycle";
export type { IDisposable as DockviewIDisposable } from "./lifecycle";

export * from "./panel/types";

export * from "./splitview/splitview";
export type {
	SplitviewComponentOptions,
	PanelViewInitParameters,
} from "./splitview/options";

export * from "./paneview/paneview";
export * from "./gridview/gridview";
export type { GridviewComponentOptions } from "./gridview/options";
export * from "./gridview/baseComponentGridview";

export { DraggablePaneviewPanel } from "./paneview/draggablePaneviewPanel";
export type { PaneviewDropEvent } from "./paneview/draggablePaneviewPanel";

export * from "./dockview/components/panel/content";
export * from "./dockview/components/tab/tab";
export * from "./dockview/dockviewGroupPanelModel";
export type {
	TabDragEvent,
	GroupDragEvent,
} from "./dockview/components/titlebar/tabsContainer";
export * from "./dockview/types";
export * from "./dockview/dockviewGroupPanel";
export type {
	IGroupPanelBaseProps,
	IDockviewPanelHeaderProps,
	IDockviewPanelProps,
	IDockviewHeaderActionsProps,
	IGroupHeaderProps,
	IWatermarkPanelProps,
	DockviewReadyEvent,
} from "./dockview/framework";

export * from "./dockview/options";
export * from "./dockview/dockviewPanel";
export { DefaultTab } from "./dockview/components/tab/defaultTab";
export { DefaultDockviewDeserialzier } from "./dockview/deserializer";
export type { IPanelDeserializer } from "./dockview/deserializer";

export * from "./dockview/dockviewComponent";
export * from "./gridview/gridviewComponent";
export * from "./splitview/splitviewComponent";
export * from "./paneview/paneviewComponent";
export type { PaneviewComponentOptions } from "./paneview/options";

export * from "./gridview/gridviewPanel";
export { SplitviewPanel } from "./splitview/splitviewPanel";
export type { ISplitviewPanel } from "./splitview/splitviewPanel";
export * from "./paneview/paneviewPanel";
export * from "./dockview/types";

export type { DockviewPanelRenderer } from "./overlay/overlayRenderContainer";

export {
	positionToDirection,
	directionToPosition,
} from "./dnd/droptarget";
export type {
	Position,
	MeasuredValue,
	DroptargetOverlayModel,
} from "./dnd/droptarget";

export type {
	FocusEvent,
	PanelDimensionChangeEvent,
	VisibilityEvent,
	ActiveEvent,
	PanelApi,
} from "./api/panelApi";
export type {
	SizeEvent,
	GridviewPanelApi,
	GridConstraintChangeEvent,
} from "./api/gridviewPanelApi";
export type {
	TitleEvent,
	RendererChangedEvent,
	DockviewPanelApi,
	DockviewPanelMoveParams,
} from "./api/dockviewPanelApi";
export type {
	PanelSizeEvent,
	PanelConstraintChangeEvent,
	SplitviewPanelApi,
} from "./api/splitviewPanelApi";
export type { ExpansionEvent, PaneviewPanelApi } from "./api/paneviewPanelApi";
export type {
	DockviewGroupPanelApi,
	DockviewGroupPanelFloatingChangeEvent,
	DockviewGroupMoveParams,
} from "./api/dockviewGroupPanelApi";
export {
	SplitviewApi,
	PaneviewApi,
	GridviewApi,
	DockviewApi,
} from "./api/component.api";
export type { CommonApi } from "./api/component.api";
export {
	createDockview,
	createGridview,
	createPaneview,
	createSplitview,
} from "./api/entryPoints";
