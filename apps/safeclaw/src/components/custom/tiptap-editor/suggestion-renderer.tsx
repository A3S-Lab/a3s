/**
 * Renders the suggestion popup using React portal.
 * Used by both @mention and /slash-command extensions.
 */
import type { SuggestionOptions, SuggestionProps } from "@tiptap/suggestion";
import { createRoot, type Root } from "react-dom/client";
import MentionList, {
	type MentionListRef,
	type SuggestionItem,
} from "./mention-list";

/** Max popup height — must match max-h-64 (256px) in MentionList */
const POPUP_MAX_HEIGHT = 260;
const GAP = 4;

function positionContainer(container: HTMLDivElement, rect: DOMRect | null) {
	if (!rect) return;

	const spaceBelow = window.innerHeight - rect.bottom;
	const spaceAbove = rect.top;

	// Flip above if not enough room below
	if (spaceBelow < POPUP_MAX_HEIGHT + GAP && spaceAbove > spaceBelow) {
		container.style.top = "";
		container.style.bottom = `${window.innerHeight - rect.top + GAP}px`;
	} else {
		container.style.bottom = "";
		container.style.top = `${rect.bottom + GAP}px`;
	}

	// Clamp left so popup doesn't overflow right edge
	const popupWidth = 288; // w-72 = 18rem = 288px
	const left = Math.min(rect.left, window.innerWidth - popupWidth - 8);
	container.style.left = `${Math.max(8, left)}px`;
}

export function createSuggestionRenderer(
	getItems: (query: string) => SuggestionItem[],
	onSelect?: () => void,
): Pick<SuggestionOptions<SuggestionItem>, "items" | "render"> {
	return {
		items: ({ query }) => getItems(query),
		render: () => {
			let component: MentionListRef | null = null;
			let container: HTMLDivElement | null = null;
			let root: Root | null = null;

			return {
				onStart: (props: SuggestionProps<SuggestionItem>) => {
					container = document.createElement("div");
					container.style.position = "fixed";
					container.style.zIndex = "9999";

					positionContainer(container, props.clientRect?.() ?? null);
					document.body.appendChild(container);

					const wrappedCommand = (item: SuggestionItem) => {
						onSelect?.();
						props.command(item);
					};

					root = createRoot(container);
					root.render(
						<MentionList
							ref={(ref) => {
								component = ref;
							}}
							items={props.items}
							command={wrappedCommand}
						/>,
					);
				},

				onUpdate: (props: SuggestionProps<SuggestionItem>) => {
					if (!root || !container) return;

					positionContainer(container, props.clientRect?.() ?? null);

					const wrappedCommand = (item: SuggestionItem) => {
						onSelect?.();
						props.command(item);
					};

					root.render(
						<MentionList
							ref={(ref) => {
								component = ref;
							}}
							items={props.items}
							command={wrappedCommand}
						/>,
					);
				},

				onKeyDown: (props: { event: KeyboardEvent }) => {
					if (props.event.key === "Escape") {
						destroy();
						return true;
					}
					return component?.onKeyDown(props) ?? false;
				},

				onExit: () => {
					destroy();
				},
			};

			function destroy() {
				if (root) {
					// Defer unmount to avoid React warnings
					const r = root;
					const c = container;
					root = null;
					container = null;
					setTimeout(() => {
						r.unmount();
						c?.remove();
					}, 0);
				}
			}
		},
	};
}
