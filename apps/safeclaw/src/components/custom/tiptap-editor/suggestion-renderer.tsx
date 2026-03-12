/**
 * Renders the suggestion popup using React portal.
 * Used by both @mention and /slash-command extensions.
 */
import type React from "react";
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
	onFolderClick?: (item: SuggestionItem) => void,
	onOpen?: () => void,
	suggestionOpenRef?: React.MutableRefObject<boolean>,
	enableSearch?: boolean,
): Pick<SuggestionOptions<SuggestionItem>, "items" | "render"> {
	return {
		items: ({ query }) => getItems(query),
		render: () => {
			let component: MentionListRef | null = null;
			let container: HTMLDivElement | null = null;
			let root: Root | null = null;
			let currentQuery = "";
			let updateScheduled = false;
			let currentCommandFn: ((item: SuggestionItem) => void) | null = null;

			// Wrapped command that calls the current command function
			const wrappedCommand = (item: SuggestionItem) => {
				onSelect?.();
				if (currentCommandFn) {
					currentCommandFn(item);
				}
			};

			// Wrapped folder click handler that triggers re-render
			const wrappedFolderClick = (item: SuggestionItem) => {
				if (onFolderClick) {
					console.log(
						"[SuggestionRenderer] Folder clicked, calling onFolderClick",
					);
					onFolderClick(item);

					// Schedule update with multiple requestAnimationFrame for better timing
					// This ensures React state updates and DOM updates are complete
					if (!updateScheduled) {
						updateScheduled = true;
						requestAnimationFrame(() => {
							requestAnimationFrame(() => {
								requestAnimationFrame(() => {
									if (root && container) {
										const items = getItems(currentQuery);
										console.log(
											"[SuggestionRenderer] Re-rendering after folder click, items:",
											items.length,
										);
										root.render(
											<MentionList
												ref={(ref) => {
													component = ref;
												}}
												items={items}
												command={wrappedCommand}
												onFolderClick={wrappedFolderClick}
												enableSearch={enableSearch}
											/>,
										);
										updateScheduled = false;
									}
								});
							});
						});
					}
				}
			};

			return {
				onStart: (props: SuggestionProps<SuggestionItem>) => {
					// Mark suggestion menu as open
					if (suggestionOpenRef) {
						suggestionOpenRef.current = true;
					}

					// Trigger refresh callback when panel opens
					console.log("[SuggestionRenderer] onStart called, triggering onOpen");

					currentQuery = props.query || "";
					currentCommandFn = props.command;

					container = document.createElement("div");
					container.style.position = "fixed";
					container.style.zIndex = "9999";

					positionContainer(container, props.clientRect?.() ?? null);
					document.body.appendChild(container);

					root = createRoot(container);

					// Initial render with current items
					root.render(
						<MentionList
							ref={(ref) => {
								component = ref;
							}}
							items={props.items}
							command={wrappedCommand}
							onFolderClick={wrappedFolderClick}
							enableSearch={enableSearch}
						/>,
					);

					// Call onOpen and wait for it to complete, then re-render
					if (onOpen) {
						Promise.resolve(onOpen()).then(() => {
							// Wait for state updates to complete
							requestAnimationFrame(() => {
								requestAnimationFrame(() => {
									if (root && container) {
										const updatedItems = getItems(currentQuery);
										console.log(
											"[SuggestionRenderer] Re-rendering after onOpen, items:",
											updatedItems.length,
										);
										root.render(
											<MentionList
												ref={(ref) => {
													component = ref;
												}}
												items={updatedItems}
												command={wrappedCommand}
												onFolderClick={wrappedFolderClick}
												enableSearch={enableSearch}
											/>,
										);
									}
								});
							});
						});
					}
				},

				onUpdate: (props: SuggestionProps<SuggestionItem>) => {
					if (!root || !container) return;

					currentQuery = props.query || "";
					currentCommandFn = props.command;

					positionContainer(container, props.clientRect?.() ?? null);

					root.render(
						<MentionList
							ref={(ref) => {
								component = ref;
							}}
							items={props.items}
							command={wrappedCommand}
							onFolderClick={wrappedFolderClick}
							enableSearch={enableSearch}
						/>,
					);
				},

				onKeyDown: (props: { event: KeyboardEvent }) => {
					if (props.event.key === "Escape") {
						destroy();
						return true;
					}
					// If component is ready, delegate to it
					if (component) {
						return component.onKeyDown(props);
					}
					// If component is not ready yet but we have items, intercept Enter/Tab
					// to prevent submitting the form before the suggestion is selected
					if (props.event.key === "Enter" || props.event.key === "Tab") {
						return true;
					}
					return false;
				},

				onExit: () => {
					// Mark suggestion menu as closed
					if (suggestionOpenRef) {
						suggestionOpenRef.current = false;
					}
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
